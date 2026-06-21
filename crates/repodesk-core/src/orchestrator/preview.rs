//! A human-readable preview of what an agent run *would* do, computed from the
//! routed plan **before** anything launches. It answers the questions a user
//! needs before approving spend or workspace writes: which executor + model,
//! whether it writes, in an isolated workspace, the token/cost estimate, and
//! exactly which approvals the run requires.

use serde::{Deserialize, Serialize};

use crate::api_clients::ProviderSettings;
use crate::errors::RepoDeskResult;
use crate::routing::types::ExecutorKind;
use crate::usage::cost::{estimate_agent_cost, load_cost_config};

use super::plan::{
    build_plan, plan_has_coding_agent_step, plan_has_paid_provider_step, step_uses_paid_provider,
};
use super::types::{OrchestrationPlan, SubAgentTask};

/// Nominal per-step input estimate — mirrors the routing request the planner
/// uses, so the preview's numbers match what the run would actually route on.
const NOMINAL_INPUT_TOKENS: usize = 4_000;

/// One routed step, as the user should see it before launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreviewStep {
    pub step_id: String,
    pub title: String,
    /// Human label for the executor (e.g. "Codex CLI", "Ollama").
    pub executor_label: String,
    pub executor_kind: ExecutorKind,
    pub model: String,
    pub allow_write: bool,
    /// Coding-agent steps always run inside an isolated git worktree.
    pub isolated_workspace: bool,
    /// Whether this step routes to a paid completion provider.
    pub paid: bool,
    pub estimated_input_tokens: usize,
    pub estimated_output_tokens: usize,
    pub estimated_cost_units: f64,
}

/// The whole-run preview: per-step detail plus the aggregate facts and the
/// exact approvals the run requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreview {
    pub goal: String,
    pub steps: Vec<ExecutionPreviewStep>,
    pub total_estimated_tokens: usize,
    pub total_estimated_cost_units: f64,
    pub currency_label: String,
    /// Any step writes to the workspace.
    pub expected_writes: bool,
    /// The run uses an isolated worktree (any coding-agent step).
    pub isolated_workspace: bool,
    /// The run needs the coding-agent + workspace-write approval.
    pub requires_coding_agent_approval: bool,
    /// The run needs the paid-provider approval (may spend).
    pub requires_paid_approval: bool,
}

/// A stable, human label for an executor, preferring the concrete CLI/runtime id.
pub fn executor_label(kind: ExecutorKind, executor_id: &str) -> String {
    match executor_id.trim().to_ascii_lowercase().as_str() {
        "codex" | "codex_cli" => "Codex CLI".to_string(),
        "claude" | "claude_code" | "claude_code_cli" => "Claude Code CLI".to_string(),
        "ollama" => "Ollama".to_string(),
        "lm_studio" => "LM Studio".to_string(),
        "local_checks" => "Check runner".to_string(),
        "manual" => "Manual".to_string(),
        _ => match kind {
            ExecutorKind::CodingAgent => "Coding agent".to_string(),
            ExecutorKind::LocalRuntime => "Local model".to_string(),
            ExecutorKind::CompletionProvider => "API model".to_string(),
            ExecutorKind::CheckRunner => "Check runner".to_string(),
            ExecutorKind::Manual => "Manual".to_string(),
        },
    }
}

fn preview_step(
    step: &SubAgentTask,
    cost: &crate::usage::cost::CostConfig,
) -> ExecutionPreviewStep {
    let kind = step.resolved_executor_kind();
    let input = NOMINAL_INPUT_TOKENS;
    let output = step.budget_tokens;
    let estimate = estimate_agent_cost(cost, step.resolved_executor_id(), input, output);
    ExecutionPreviewStep {
        step_id: step.id.clone(),
        title: step.title.clone(),
        executor_label: executor_label(kind, step.resolved_executor_id()),
        executor_kind: kind,
        model: step
            .model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "provider default".to_string()),
        allow_write: step.allow_write,
        isolated_workspace: kind == ExecutorKind::CodingAgent,
        paid: step_uses_paid_provider(step),
        estimated_input_tokens: input,
        estimated_output_tokens: output,
        estimated_cost_units: estimate.estimated_cost_units,
    }
}

/// Build the preview for an already-routed plan.
pub fn preview_plan(plan: &OrchestrationPlan) -> RepoDeskResult<ExecutionPreview> {
    let cost = load_cost_config()?;
    let steps: Vec<ExecutionPreviewStep> = plan
        .steps
        .iter()
        .map(|step| preview_step(step, &cost))
        .collect();

    let total_estimated_tokens = steps
        .iter()
        .map(|s| s.estimated_input_tokens + s.estimated_output_tokens)
        .sum();
    let total_estimated_cost_units = steps.iter().map(|s| s.estimated_cost_units).sum();

    Ok(ExecutionPreview {
        goal: plan.goal.clone(),
        steps,
        total_estimated_tokens,
        total_estimated_cost_units,
        currency_label: cost.currency_label.clone(),
        expected_writes: plan.steps.iter().any(|s| s.allow_write),
        isolated_workspace: plan_has_coding_agent_step(plan),
        requires_coding_agent_approval: plan_has_coding_agent_step(plan),
        requires_paid_approval: plan_has_paid_provider_step(plan),
    })
}

/// Route the active task into a plan and preview it — the same `build_plan` the
/// run uses, so the preview reflects exactly what `orchestrate_run` would launch.
pub fn execution_preview(
    goal: Option<String>,
    settings: &ProviderSettings,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> RepoDeskResult<ExecutionPreview> {
    let plan = build_plan(goal, settings, override_provider, override_model)?;
    preview_plan(&plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_clients::ThinkingLevel;
    use crate::routing::types::TaskKind;

    fn step(id: &str, executor_id: &str, kind: ExecutorKind, allow_write: bool) -> SubAgentTask {
        SubAgentTask {
            id: id.to_string(),
            title: id.to_string(),
            kind: TaskKind::Patch,
            agent: executor_id.to_string(),
            provider: executor_id.to_string(),
            executor_kind: kind,
            executor_id: executor_id.to_string(),
            provider_id: Some(executor_id.to_string()),
            model: Some("codex".to_string()),
            thinking: ThinkingLevel::None,
            instruction: String::new(),
            depends_on: vec![],
            budget_tokens: 1_500,
            allow_write,
            verify_command: None,
        }
    }

    #[test]
    fn labels_map_known_executors() {
        assert_eq!(
            executor_label(ExecutorKind::CodingAgent, "codex_cli"),
            "Codex CLI"
        );
        assert_eq!(
            executor_label(ExecutorKind::LocalRuntime, "ollama"),
            "Ollama"
        );
        assert_eq!(
            executor_label(ExecutorKind::CompletionProvider, "weird"),
            "API model"
        );
    }

    #[test]
    fn preview_aggregates_writes_workspace_and_approvals() {
        let plan = OrchestrationPlan {
            project: "demo".into(),
            task_id: "t".into(),
            goal: "do the thing".into(),
            steps: vec![
                step("analyze", "ollama", ExecutorKind::LocalRuntime, false),
                step("implement", "codex_cli", ExecutorKind::CodingAgent, true),
            ],
        };
        let preview = preview_plan(&plan).expect("preview");
        assert_eq!(preview.steps.len(), 2);
        assert!(preview.expected_writes);
        assert!(preview.isolated_workspace);
        assert!(preview.requires_coding_agent_approval);
        // The implementation step writes inside an isolated worktree.
        let implement = preview
            .steps
            .iter()
            .find(|s| s.step_id == "implement")
            .unwrap();
        assert!(implement.allow_write);
        assert!(implement.isolated_workspace);
        assert_eq!(implement.executor_label, "Codex CLI");
        // Tokens are summed across steps (input + output per step).
        assert_eq!(preview.total_estimated_tokens, (4_000 + 1_500) * 2);
    }
}
