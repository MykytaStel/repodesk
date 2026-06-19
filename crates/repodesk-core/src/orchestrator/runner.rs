//! Execute an [`OrchestrationPlan`]: run sub-agents in dependency order, each
//! with its own bounded context pack and routed model. Every step passes the
//! same gates the rest of RepoDesk uses (safety scan, budget verdict, cost
//! ceiling), every call is logged to the token ledger, and every output is fed
//! to the Memory Brain as human-reviewable capture proposals (the durable brain
//! still requires propose→accept; only the in-run handoff is automatic).
//!
//! Execution proceeds in dependency *waves* (see [`dependency_waves`]): steps
//! whose dependencies are all satisfied form a wave and run concurrently, while
//! waves themselves run in order. Gating (dependency, safety, budget, cost
//! ceiling) happens in a deterministic ascending-index decision pass before a
//! wave's calls are launched, and results are recorded in index order, so a run
//! is identical regardless of which concurrent call finishes first.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use tokio::fs;
use tokio::task::JoinSet;

use crate::api_clients::{LlmRequest, LlmResponse, ProviderSettings, provider_for};
use crate::errors::RepoDeskResult;
use crate::persistence::event_journal::{LogEventInput, log_event};
use crate::routing::types::ExecutorKind;
use crate::safety::{self, SafetyLevel};
use crate::tasks::show_active_task;
use crate::tokens::estimate_text;
use crate::usage::budget::{BudgetLevel, evaluate_context, load_budget_config};
use crate::usage::cost::{estimate_agent_cost, load_cost_config};
use crate::usage::token_ledger::{LogTokenInput, log_token_event};

use super::context::{build_base_context, compose_step_prompt, step_system_prompt};
use super::types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, RunSummary, SubAgentResult, SubAgentStatus,
    SubAgentTask, dependency_waves,
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

    // Steps grouped into dependency waves: every step in a wave is independent
    // of the others, so their provider calls can run concurrently.
    let waves = dependency_waves(&plan.steps)?;
    let budget = load_budget_config()?;
    let cost_config = load_cost_config()?;

    // Shared base context (local; no network) — built once, reused per step.
    let base = build_base_context().await?;

    let mut state = RunState::default();

    for wave in &waves {
        // ── Decision pass: gate every step in ascending index order. Each step
        // is either finalized here (skipped/blocked/manual/dry-run) or has its
        // provider call scheduled. The cost ceiling reserves projected cost in
        // order, so the gating decision is deterministic regardless of the
        // concurrent execution that follows.
        let mut set: JoinSet<(usize, RepoDeskResult<LlmResponse>)> = JoinSet::new();
        let mut scheduled: Vec<ExecMeta> = Vec::new();

        for &idx in wave {
            let step = &plan.steps[idx];

            // A dependency from an earlier wave that didn't complete blocks this step.
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
            if step.executor_kind == ExecutorKind::Manual
                || step.provider.eq_ignore_ascii_case("manual")
                || step.resolved_executor_id().eq_ignore_ascii_case("manual")
            {
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

            if step.executor_kind == ExecutorKind::CodingAgent {
                state.push(SubAgentResult {
                    status: if opts.dry_run {
                        SubAgentStatus::Ok
                    } else {
                        SubAgentStatus::Skipped
                    },
                    input_tokens,
                    notes: vec![format!(
                        "{} is a coding-agent executor; automatic CLI execution is not implemented in this PR",
                        step.resolved_executor_id()
                    )],
                    ..base_result(step)
                });
                continue;
            }

            let projected_cost = estimate_agent_cost(
                &cost_config,
                step.resolved_executor_id(),
                input_tokens,
                step.budget_tokens,
            )
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
            // Reserve the projection so later steps in this wave gate against it;
            // real runs reconcile it to the actual cost in the finalize pass.
            state.running_cost += projected_cost;

            if opts.dry_run {
                // Marked Ok so dependents still preview; the run status says DryRun.
                state.push(SubAgentResult {
                    status: SubAgentStatus::Ok,
                    input_tokens,
                    cost_units: projected_cost,
                    notes: vec![format!(
                        "[dry-run] would call {} via {}{} (projected only)",
                        step.resolved_executor_id(),
                        step.resolved_provider_id().unwrap_or(&step.provider),
                        step.model
                            .as_deref()
                            .map(|m| format!("/{m}"))
                            .unwrap_or_default()
                    )],
                    ..base_result(step)
                });
                continue;
            }

            // Real execution — schedule the provider call to run concurrently.
            let Some(provider_id) = step.resolved_provider_id() else {
                state.running_cost -= projected_cost; // refund: nothing spent
                state.push(failed(
                    step,
                    format!(
                        "provider unavailable: {} has no completion provider id",
                        step.resolved_executor_id()
                    ),
                ));
                continue;
            };
            let provider = match provider_for(provider_id, &opts.settings) {
                Ok(provider) => provider,
                Err(error) => {
                    state.running_cost -= projected_cost; // refund: nothing spent
                    state.push(failed(step, format!("provider unavailable: {error}")));
                    continue;
                }
            };
            let request = LlmRequest::new(step.model.clone().unwrap_or_default(), prompt)
                .with_system(step_system_prompt(step))
                .with_max_tokens(step.budget_tokens as u32)
                .with_thinking(step.thinking);
            let fut = provider.complete(request);
            set.spawn(async move { (idx, fut.await) });
            scheduled.push(ExecMeta {
                idx,
                input_tokens,
                projected_cost,
            });
        }

        // ── Execute pass: await every scheduled call (they run concurrently).
        let mut responses: HashMap<usize, RepoDeskResult<LlmResponse>> = HashMap::new();
        while let Some(joined) = set.join_next().await {
            let (idx, response) = joined.map_err(|error| {
                crate::errors::RepoDeskError::Api(format!("orchestration task failed: {error}"))
            })?;
            responses.insert(idx, response);
        }

        // ── Finalize pass: record results in ascending index order so the run
        // is identical regardless of which call finished first.
        scheduled.sort_by_key(|meta| meta.idx);
        for meta in scheduled {
            let step = &plan.steps[meta.idx];
            match responses.remove(&meta.idx) {
                Some(Ok(response)) => {
                    let _ = log_token_event(LogTokenInput {
                        agent: step.resolved_executor_id().to_string(),
                        model: Some(response.model.clone()),
                        input_tokens: response.input_tokens,
                        output_tokens: response.output_tokens,
                        category: "orchestrate".to_string(),
                        notes: Some(step.id.clone()),
                    });
                    let cost = estimate_agent_cost(
                        &cost_config,
                        step.resolved_executor_id(),
                        response.input_tokens,
                        response.output_tokens,
                    )
                    .estimated_cost_units;
                    // Reconcile the reservation to the actual cost.
                    state.running_cost += cost - meta.projected_cost;

                    // Coordination: durable brain stays human-approved (proposals only).
                    let captured = crate::memory::capture_from_text(
                        &plan.project,
                        &plan.task_id,
                        step.resolved_executor_id(),
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
                Some(Err(error)) => {
                    state.running_cost -= meta.projected_cost; // refund: call failed
                    state.push(SubAgentResult {
                        input_tokens: meta.input_tokens,
                        ..failed(step, format!("provider call failed: {error}"))
                    });
                }
                None => {
                    state.running_cost -= meta.projected_cost;
                    state.push(failed(step, "provider call produced no result".to_string()));
                }
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
    // Record the outcome ledger (the N8 learning signal). Dry runs carry no
    // signal and are skipped inside `record_run`; a ledger error never fails a
    // run that already completed.
    let _ = crate::outcomes::record_run(plan, &run);
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

/// Every persisted run for the active task, newest-first, as lightweight
/// summaries (no per-step outputs). The rolling `latest.json` pointer is
/// skipped so a run is never listed twice.
pub fn list_runs() -> RepoDeskResult<Vec<RunSummary>> {
    let dir = orchestrate_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_run_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("run-") && name.ends_with(".json"));
        if !is_run_file {
            continue;
        }
        // Skip a file that fails to parse rather than failing the whole list.
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(run) = serde_json::from_str::<OrchestrationRun>(&content)
        {
            summaries.push(run.summary());
        }
    }

    // Run ids embed a sortable timestamp (`run-YYYYMMDD-HHMMSS`), so a reverse
    // lexical sort on the id is newest-first.
    summaries.sort_by(|a, b| b.run_id.cmp(&a.run_id));
    Ok(summaries)
}

// ── internals ───────────────────────────────────────────────────────────────

/// Bookkeeping for a step whose provider call was scheduled in a wave, so the
/// finalize pass can reconcile its reserved cost and attribute the result.
struct ExecMeta {
    idx: usize,
    input_tokens: usize,
    projected_cost: f64,
}

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
        agent: step.resolved_executor_id().to_string(),
        provider: step
            .resolved_provider_id()
            .unwrap_or(&step.provider)
            .to_string(),
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
