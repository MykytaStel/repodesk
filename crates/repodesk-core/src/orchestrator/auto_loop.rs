//! Bounded autonomous orchestration loop (N8-C — the "Hermes" autonomy).
//!
//! A single [`run_plan`] is one attempt; this drives **attempt → evaluate →
//! re-plan → retry** until the task succeeds or a stop condition fires. The loop
//! is deliberately *bounded and explainable*, not open-ended: every iteration is
//! capped by a max-iteration count and an optional total-cost ceiling, and the
//! loop pauses (never spends) before a paid plan unless the human approved it —
//! the same human-in-the-loop discipline used elsewhere.
//!
//! **The learning between retries is free.** Each real [`run_plan`] records its
//! outcomes to the ledger ([`crate::outcomes`]), and the next iteration's
//! [`build_plan`] re-reads the learned routing bias — so a provider that just
//! failed a step is routed away from on the retry, with no extra machinery here.
//!
//! Re-planning is currently the same deterministic template re-routed under the
//! updated bias. LLM-assisted decomposition (a dynamic plan per attempt) is the
//! natural next step and is intentionally left out: it is non-deterministic and
//! needs a live model to verify, so it does not belong in this bounded core.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::api_clients::ProviderSettings;
use crate::errors::RepoDeskResult;

use super::plan::{build_plan, plan_has_coding_agent_step, plan_has_paid_provider_step};
use super::runner::{AgentWorkspacePolicy, ExecutionAuthorization, RunOptions, run_plan};
use super::types::{RunStatus, SubAgentStatus};

/// Default attempt cap when the caller does not specify one.
pub const DEFAULT_MAX_ITERATIONS: usize = 3;

/// How the autonomous loop is allowed to run.
#[derive(Debug, Clone)]
pub struct LoopOptions {
    /// Hard cap on attempts (clamped to at least 1).
    pub max_iterations: usize,
    /// Optional ceiling on total cost across all attempts (cost units).
    pub max_total_cost: Option<f64>,
    /// Preview only — no provider calls, no spend, a single pass.
    pub dry_run: bool,
    /// Human-in-the-loop: when false, the loop refuses to execute a plan that
    /// includes paid provider steps and stops with [`LoopStatus::NeedsApproval`].
    pub approve_paid: bool,
    /// Human-in-the-loop: when false, coding-agent CLI steps are previewed but
    /// not launched.
    pub approve_coding_agents: bool,
    /// Timeout for one coding-agent process.
    pub coding_agent_timeout_secs: u64,
    /// Target provider bypass for the adaptive router.
    pub override_provider: Option<String>,
    /// Target model bypass for the adaptive router.
    pub override_model: Option<String>,
    /// Provider credentials/endpoints.
    pub settings: ProviderSettings,
    /// Workspace policy for coding-agent CLIs. Defaults to requiring isolation:
    /// an autonomous loop must never launch a CLI in the active checkout.
    pub agent_workspace_policy: AgentWorkspacePolicy,
}

impl Default for LoopOptions {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            max_total_cost: None,
            dry_run: false,
            approve_paid: false,
            approve_coding_agents: false,
            coding_agent_timeout_secs: 600,
            override_provider: None,
            override_model: None,
            settings: ProviderSettings::default(),
            agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
        }
    }
}

/// Why the loop stopped — every outcome is terminal and explained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStatus {
    /// An attempt completed every step successfully.
    Succeeded,
    /// A gated plan needs human approval before spend or CLI launch.
    NeedsApproval,
    /// A safety/budget guardrail blocked a step — needs human intervention, not
    /// a retry (retrying would hit the same wall).
    GuardrailBlocked,
    /// Out of attempts or out of cost budget before succeeding.
    Exhausted,
    /// A preview pass (dry run) finished; nothing was executed.
    DryRun,
}

impl LoopStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, LoopStatus::Succeeded)
    }
}

/// One attempt within the loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIteration {
    /// Zero-based attempt index.
    pub index: usize,
    pub run_id: String,
    pub run_status: RunStatus,
    pub cost_units: f64,
    /// Why this attempt ended the way it did.
    pub note: String,
}

/// The aggregated result of an autonomous loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopRun {
    pub project: String,
    pub task_id: String,
    pub goal: String,
    pub status: LoopStatus,
    pub iterations: Vec<LoopIteration>,
    pub total_cost_units: f64,
    pub started_at: String,
    pub finished_at: String,
}

// Stable substrings of the runner's block notes, used to tell a guardrail block
// (safety/budget — a human must fix it) from a cost-ceiling block (just out of
// budget for this attempt). Kept in sync with `runner.rs`.
const SAFETY_BLOCK_MARKER: &str = "safety scan";
const BUDGET_BLOCK_MARKER: &str = "above the block limit";

/// Run the autonomous loop for the active task. `goal` overrides the task title.
pub async fn run_loop(goal: Option<String>, opts: &LoopOptions) -> RepoDeskResult<LoopRun> {
    let started_at = Utc::now().to_rfc3339();
    let max_iterations = opts.max_iterations.max(1);

    let mut iterations: Vec<LoopIteration> = Vec::new();
    let mut total_cost = 0.0f64;
    let mut status = LoopStatus::Exhausted;
    let mut project = String::new();
    let mut task_id = String::new();
    let mut resolved_goal = goal.clone().unwrap_or_default();

    for index in 0..max_iterations {
        // Re-plan each attempt: build_plan re-reads the learned bias updated by
        // the previous attempt's recorded outcomes, so routing adapts to failure.
        let plan = build_plan(
            goal.clone(),
            &opts.settings,
            opts.override_provider.clone(),
            opts.override_model.clone(),
        )?;
        project = plan.project.clone();
        task_id = plan.task_id.clone();
        resolved_goal = plan.goal.clone();

        // Human-in-the-loop: never spend or launch a CLI agent without the
        // matching approval.
        let needs_paid_approval = plan_has_paid_provider_step(&plan) && !opts.approve_paid;
        let needs_coding_agent_approval =
            plan_has_coding_agent_step(&plan) && !opts.approve_coding_agents;
        if !opts.dry_run && (needs_paid_approval || needs_coding_agent_approval) {
            status = LoopStatus::NeedsApproval;
            let note = match (needs_paid_approval, needs_coding_agent_approval) {
                (true, true) => {
                    "plan includes paid provider and coding-agent steps; re-run with approvals to execute"
                }
                (true, false) => {
                    "plan includes paid provider steps; re-run with approval to execute"
                }
                (false, true) => {
                    "plan includes coding-agent CLI steps; re-run with approval to execute"
                }
                (false, false) => unreachable!(),
            };
            iterations.push(LoopIteration {
                index,
                run_id: String::new(),
                run_status: RunStatus::Failed,
                cost_units: 0.0,
                note: note.to_string(),
            });
            break;
        }

        // Cost ceiling for this attempt = whatever total budget remains.
        let remaining = opts.max_total_cost.map(|max| (max - total_cost).max(0.0));
        let run = run_plan(
            &plan,
            &RunOptions {
                dry_run: opts.dry_run,
                max_cost: remaining,
                settings: opts.settings.clone(),
                authorization: ExecutionAuthorization {
                    allow_paid_providers: opts.approve_paid,
                    allow_coding_agents: opts.approve_coding_agents,
                    // An approved coding-agent step in the loop writes only inside
                    // its isolated worktree, so agent approval authorizes that.
                    allow_workspace_writes: opts.approve_coding_agents,
                },
                coding_agent_timeout_secs: opts.coding_agent_timeout_secs,
                agent_workspace_policy: opts.agent_workspace_policy,
            },
        )
        .await?;

        total_cost += run.total_cost_units;

        let guardrail_hit = run.results.iter().any(|result| {
            result.status == SubAgentStatus::Blocked
                && (result.notes.iter().any(|n| n.contains(SAFETY_BLOCK_MARKER))
                    || result.notes.iter().any(|n| n.contains(BUDGET_BLOCK_MARKER)))
        });

        let (note, terminal) = classify(&run.status, guardrail_hit, opts.dry_run);
        iterations.push(LoopIteration {
            index,
            run_id: run.run_id.clone(),
            run_status: run.status,
            cost_units: run.total_cost_units,
            note: note.to_string(),
        });

        if let Some(terminal_status) = terminal {
            status = terminal_status;
            break;
        }

        // Not terminal — a retry is warranted. Stop early if the cost ceiling is
        // already spent, otherwise loop again with the freshly-learned bias.
        if let Some(max) = opts.max_total_cost
            && total_cost >= max
        {
            status = LoopStatus::Exhausted;
            break;
        }
        // The for-loop bound handles the max-iteration exhaustion case; `status`
        // stays Exhausted unless a later attempt resolves it.
    }

    Ok(LoopRun {
        project,
        task_id,
        goal: resolved_goal,
        status,
        iterations,
        total_cost_units: total_cost,
        started_at,
        finished_at: Utc::now().to_rfc3339(),
    })
}

/// Classify one attempt: its human-readable note and, if terminal, the loop
/// status to stop on. `None` means "retry warranted".
fn classify(
    run_status: &RunStatus,
    guardrail_hit: bool,
    dry_run: bool,
) -> (&'static str, Option<LoopStatus>) {
    if dry_run {
        return (
            "dry run preview — nothing executed",
            Some(LoopStatus::DryRun),
        );
    }
    if guardrail_hit {
        return (
            "a safety/budget guardrail blocked a step — needs human intervention",
            Some(LoopStatus::GuardrailBlocked),
        );
    }
    match run_status {
        RunStatus::Completed => ("all steps completed", Some(LoopStatus::Succeeded)),
        RunStatus::DryRun => (
            "dry run preview — nothing executed",
            Some(LoopStatus::DryRun),
        ),
        // Partial/Failed without a guardrail block: a retry may route around the
        // failure thanks to the updated learned bias.
        RunStatus::Partial => ("some steps failed — retrying with updated routing", None),
        RunStatus::Failed => ("attempt failed — retrying with updated routing", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_is_terminal_in_a_single_pass() {
        let (_, terminal) = classify(&RunStatus::DryRun, false, true);
        assert_eq!(terminal, Some(LoopStatus::DryRun));
    }

    #[test]
    fn completed_run_succeeds() {
        let (_, terminal) = classify(&RunStatus::Completed, false, false);
        assert_eq!(terminal, Some(LoopStatus::Succeeded));
    }

    #[test]
    fn guardrail_block_stops_without_retry() {
        let (_, terminal) = classify(&RunStatus::Partial, true, false);
        assert_eq!(terminal, Some(LoopStatus::GuardrailBlocked));
    }

    #[test]
    fn partial_run_without_guardrail_retries() {
        let (_, terminal) = classify(&RunStatus::Partial, false, false);
        assert_eq!(terminal, None);
    }
}
