//! Execute an [`OrchestrationPlan`]: run sub-agents in dependency order, each
//! with its own bounded context pack and routed model. Every step passes the
//! same gates the rest of RepoDesk uses (safety scan, budget verdict, cost
//! ceiling), every call is logged to the token ledger, and every output is fed
//! to the Memory Brain as human-reviewable capture proposals (the durable brain
//! still requires propose→accept; only the in-run handoff is automatic).
//!
//! Execution is sequential in topological order. Concurrent execution of
//! independent steps is a deliberate follow-up.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use tokio::fs;

use crate::api_clients::{LlmRequest, ProviderSettings, provider_for};
use crate::errors::RepoDeskResult;
use crate::persistence::event_journal::{LogEventInput, log_event};
use crate::safety::{self, SafetyLevel};
use crate::tasks::show_active_task;
use crate::tokens::estimate_text;
use crate::usage::budget::{BudgetLevel, evaluate_context, load_budget_config};
use crate::usage::cost::{estimate_agent_cost, load_cost_config};
use crate::usage::token_ledger::{LogTokenInput, log_token_event};

use super::context::{build_base_context, compose_step_prompt, step_system_prompt};
use super::types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, SubAgentResult, SubAgentStatus, SubAgentTask,
    topological_order,
};

/// Options that govern a run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Preview only — route and gate every step, but make no provider calls.
    pub dry_run: bool,
    /// Optional cost ceiling (cost units). Halts before a step that would exceed it.
    pub max_cost: Option<f64>,
    /// Provider credentials/endpoints.
    pub settings: ProviderSettings,
}

/// Execute `plan` and return the aggregated run (also persisted to the run dir).
pub async fn run_plan(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
) -> RepoDeskResult<OrchestrationRun> {
    let started_at = Utc::now().to_rfc3339();
    let run_id = format!("run-{}", Utc::now().format("%Y%m%d-%H%M%S"));

    let order = topological_order(&plan.steps)?;
    let budget = load_budget_config()?;
    let cost_config = load_cost_config()?;

    // Shared base context (local; no network) — built once, reused per step.
    let base = build_base_context().await?;

    let mut state = RunState::default();

    for &idx in &order {
        let step = &plan.steps[idx];

        // A dependency that didn't complete blocks this step.
        if let Some(dep) = step
            .depends_on
            .iter()
            .find(|d| state.unsuccessful.contains(*d))
        {
            state.push(skipped(
                step,
                format!("skipped: dependency '{dep}' did not complete"),
            ));
            continue;
        }
        if state.ceiling_hit {
            state.push(skipped(step, "skipped: cost ceiling reached".to_string()));
            continue;
        }

        let upstream: Vec<&SubAgentResult> = step
            .depends_on
            .iter()
            .filter_map(|d| state.index.get(d).map(|&i| &state.results[i]))
            .collect();
        let prompt = compose_step_prompt(&base, &plan.goal, step, &upstream);

        // Safety gate — never send detected secrets to any provider.
        if safety::scan_text(&step.id, &prompt).level == SafetyLevel::Block {
            state.push(blocked(
                step,
                "blocked: safety scan found secret-like content in the context".to_string(),
            ));
            continue;
        }

        // Budget gate on the outgoing context.
        let input_estimate = estimate_text(&prompt);
        if evaluate_context(&input_estimate, &budget).level == BudgetLevel::Block {
            state.push(blocked(
                step,
                format!(
                    "blocked: context is {} tokens, above the block limit",
                    input_estimate.estimated_tokens
                ),
            ));
            continue;
        }

        let input_tokens = input_estimate.estimated_tokens;

        // A "manual" route means no automatic provider fits this step (e.g. an
        // unreviewed patch with no paid key configured). It is human work, not
        // an API spend: zero cost, and clearly flagged in both run modes.
        if step.provider.eq_ignore_ascii_case("manual") {
            state.push(SubAgentResult {
                status: if opts.dry_run {
                    SubAgentStatus::Ok
                } else {
                    SubAgentStatus::Skipped
                },
                input_tokens,
                notes: vec![
                    "manual step — no automatic provider fits this task kind; configure a \
                     provider key or perform this step yourself"
                        .to_string(),
                ],
                ..base_result(step)
            });
            continue;
        }

        let projected_cost =
            estimate_agent_cost(&cost_config, &step.agent, input_tokens, step.budget_tokens)
                .estimated_cost_units;

        // Cost ceiling — check before any spend.
        if let Some(max) = opts.max_cost
            && state.running_cost + projected_cost > max
        {
            state.ceiling_hit = true;
            state.push(blocked(
                step,
                format!("blocked: would exceed --max-cost ({max:.3} units)"),
            ));
            continue;
        }

        if opts.dry_run {
            state.running_cost += projected_cost;
            // Marked Ok so dependents still preview; the run status says DryRun.
            state.push(SubAgentResult {
                status: SubAgentStatus::Ok,
                input_tokens,
                cost_units: projected_cost,
                notes: vec![format!(
                    "[dry-run] would call {}{} (projected only)",
                    step.provider,
                    step.model
                        .as_deref()
                        .map(|m| format!("/{m}"))
                        .unwrap_or_default()
                )],
                ..base_result(step)
            });
            continue;
        }

        // Real execution.
        let provider = match provider_for(&step.provider, &opts.settings) {
            Ok(provider) => provider,
            Err(error) => {
                state.push(failed(step, format!("provider unavailable: {error}")));
                continue;
            }
        };
        let request = LlmRequest::new(step.model.clone().unwrap_or_default(), prompt)
            .with_system(step_system_prompt(step))
            .with_max_tokens(step.budget_tokens as u32)
            .with_thinking(step.thinking);

        match provider.complete(request).await {
            Ok(response) => {
                let _ = log_token_event(LogTokenInput {
                    agent: step.agent.clone(),
                    model: Some(response.model.clone()),
                    input_tokens: response.input_tokens,
                    output_tokens: response.output_tokens,
                    category: "orchestrate".to_string(),
                    notes: Some(step.id.clone()),
                });
                let cost = estimate_agent_cost(
                    &cost_config,
                    &step.agent,
                    response.input_tokens,
                    response.output_tokens,
                )
                .estimated_cost_units;
                state.running_cost += cost;

                // Coordination: durable brain stays human-approved (proposals only).
                let captured = crate::memory::capture_from_text(
                    &plan.project,
                    &plan.task_id,
                    &step.agent,
                    &response.text,
                )
                .map(|proposals| proposals.len())
                .unwrap_or(0);

                state.push(SubAgentResult {
                    status: SubAgentStatus::Ok,
                    model: response.model,
                    output: response.text,
                    input_tokens: response.input_tokens,
                    output_tokens: response.output_tokens,
                    cost_units: cost,
                    captured_proposals: captured,
                    ..base_result(step)
                });
            }
            Err(error) => {
                state.push(failed(step, format!("provider call failed: {error}")));
            }
        }
    }

    let status = if opts.dry_run {
        RunStatus::DryRun
    } else if state.results.iter().all(|r| r.status == SubAgentStatus::Ok) {
        RunStatus::Completed
    } else if state.results.iter().any(|r| r.status == SubAgentStatus::Ok) {
        RunStatus::Partial
    } else {
        RunStatus::Failed
    };

    let run = OrchestrationRun {
        run_id,
        project: plan.project.clone(),
        task_id: plan.task_id.clone(),
        goal: plan.goal.clone(),
        status,
        dry_run: opts.dry_run,
        started_at,
        finished_at: Utc::now().to_rfc3339(),
        total_input_tokens: state.results.iter().map(|r| r.input_tokens).sum(),
        total_output_tokens: state.results.iter().map(|r| r.output_tokens).sum(),
        total_cost_units: state.results.iter().map(|r| r.cost_units).sum(),
        results: state.results,
    };

    persist_run(&run).await?;
    let _ = log_event(LogEventInput {
        module_name: "orchestrator".to_string(),
        level: "info".to_string(),
        message: format!("orchestration {} finished: {:?}", run.run_id, run.status),
        metadata: vec![
            ("run_id".to_string(), run.run_id.clone()),
            (
                "cost_units".to_string(),
                format!("{:.3}", run.total_cost_units),
            ),
        ],
    });

    Ok(run)
}

/// Most recent run for the active task, if any.
pub fn load_latest_run() -> RepoDeskResult<Option<OrchestrationRun>> {
    read_run_file("latest.json")
}

/// A specific run by id for the active task, if present.
pub fn load_run(run_id: &str) -> RepoDeskResult<Option<OrchestrationRun>> {
    read_run_file(&format!("{run_id}.json"))
}

// ── internals ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct RunState {
    results: Vec<SubAgentResult>,
    index: HashMap<String, usize>,
    unsuccessful: HashSet<String>,
    running_cost: f64,
    ceiling_hit: bool,
}

impl RunState {
    fn push(&mut self, result: SubAgentResult) {
        if !result.status.is_success() {
            self.unsuccessful.insert(result.task_id.clone());
        }
        self.index
            .insert(result.task_id.clone(), self.results.len());
        self.results.push(result);
    }
}

/// An empty result carrying the step's identity, for `..` struct update.
fn base_result(step: &SubAgentTask) -> SubAgentResult {
    SubAgentResult {
        task_id: step.id.clone(),
        agent: step.agent.clone(),
        provider: step.provider.clone(),
        model: step.model.clone().unwrap_or_default(),
        status: SubAgentStatus::Ok,
        output: String::new(),
        input_tokens: 0,
        output_tokens: 0,
        cost_units: 0.0,
        captured_proposals: 0,
        notes: Vec::new(),
    }
}

fn skipped(step: &SubAgentTask, note: String) -> SubAgentResult {
    SubAgentResult {
        status: SubAgentStatus::Skipped,
        notes: vec![note],
        ..base_result(step)
    }
}

fn blocked(step: &SubAgentTask, note: String) -> SubAgentResult {
    SubAgentResult {
        status: SubAgentStatus::Blocked,
        notes: vec![note],
        ..base_result(step)
    }
}

fn failed(step: &SubAgentTask, note: String) -> SubAgentResult {
    SubAgentResult {
        status: SubAgentStatus::Failed,
        notes: vec![note],
        ..base_result(step)
    }
}

async fn persist_run(run: &OrchestrationRun) -> RepoDeskResult<PathBuf> {
    let dir = orchestrate_dir()?;
    fs::create_dir_all(&dir).await?;
    let json = serde_json::to_string_pretty(run)?;
    let path = dir.join(format!("{}.json", run.run_id));
    fs::write(&path, &json).await?;
    fs::write(dir.join("latest.json"), &json).await?;
    Ok(path)
}

fn read_run_file(name: &str) -> RepoDeskResult<Option<OrchestrationRun>> {
    let path = orchestrate_dir()?.join(name);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

fn orchestrate_dir() -> RepoDeskResult<PathBuf> {
    Ok(show_active_task()?.config.run_dir.join("orchestrate"))
}
