//! A human-readable preview of what an agent run *would* do, computed from the
//! routed plan **before** anything launches. It answers the questions a user
//! needs before approving spend or workspace writes: which executor + model,
//! which prepared context packet, whether it writes, where it writes, the
//! token/cost estimate, and exactly which approvals the run requires.

use serde::{Deserialize, Serialize};

use crate::api_clients::ProviderSettings;
use crate::errors::RepoDeskResult;
use crate::routing::types::ExecutorKind;
use crate::tasks::show_active_task;
use crate::usage::cost::{estimate_agent_cost, load_cost_config};

use super::plan::{
    build_plan, plan_has_coding_agent_step, plan_has_paid_provider_step, step_uses_paid_provider,
};
use super::types::{OrchestrationPlan, SubAgentTask};

/// Fallback when there is no prepared Context Pipeline yet. Normal Work flow
/// reaches Execute only after Prepare, but direct CLI/orchestrator callers may
/// still preview before building task context.
const FALLBACK_INPUT_TOKENS: usize = 4_000;

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

/// Prepared canonical context facts shown before launch. This contains only
/// structural evidence; raw context content stays in `context.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionContextPreview {
    pub prepared: bool,
    pub context_tokens: usize,
    pub candidate_tokens: usize,
    pub token_budget: Option<usize>,
    pub included_sources: usize,
    pub excluded_sources: usize,
    pub context_fingerprint: Option<String>,
    pub generated_at: Option<String>,
    pub warning: Option<String>,
}

impl Default for ExecutionContextPreview {
    fn default() -> Self {
        Self {
            prepared: false,
            context_tokens: 0,
            candidate_tokens: 0,
            token_budget: None,
            included_sources: 0,
            excluded_sources: 0,
            context_fingerprint: None,
            generated_at: None,
            warning: None,
        }
    }
}

/// The whole-run preview: per-step detail plus the aggregate facts and the
/// exact approvals the run requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreview {
    pub goal: String,
    pub steps: Vec<ExecutionPreviewStep>,
    pub context: ExecutionContextPreview,
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
    estimated_input_tokens: usize,
) -> ExecutionPreviewStep {
    let kind = step.resolved_executor_kind();
    let output = step.budget_tokens;
    let estimate = estimate_agent_cost(
        cost,
        step.resolved_executor_id(),
        estimated_input_tokens,
        output,
    );
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
        estimated_input_tokens,
        estimated_output_tokens: output,
        estimated_cost_units: estimate.estimated_cost_units,
    }
}

fn active_context_preview() -> ExecutionContextPreview {
    let Ok(task) = show_active_task() else {
        return ExecutionContextPreview::default();
    };

    let report = match crate::engineering::load_context_inspector(&task.config.run_dir) {
        Ok(report) => report,
        Err(error) => {
            return ExecutionContextPreview {
                warning: Some(format!("Context evidence could not be read: {error}")),
                ..ExecutionContextPreview::default()
            };
        }
    };

    if let Some(error) = report.pipeline_error {
        return ExecutionContextPreview {
            warning: Some(format!("Prepared Context Pipeline is damaged: {error}")),
            ..ExecutionContextPreview::default()
        };
    }

    let Some(pipeline) = report.pipeline else {
        return ExecutionContextPreview {
            warning: Some("No prepared Context Pipeline yet.".to_string()),
            ..ExecutionContextPreview::default()
        };
    };

    let included_sources = pipeline
        .selections
        .iter()
        .filter(|selection| {
            selection.state == crate::context_pipeline::ContextSelectionState::Included
        })
        .count();
    let excluded_sources = pipeline.selections.len().saturating_sub(included_sources);
    let context_tokens = report
        .compactness
        .latest
        .as_ref()
        .map(|latest| latest.included_tokens)
        .unwrap_or(pipeline.included_tokens);

    ExecutionContextPreview {
        prepared: true,
        context_tokens,
        candidate_tokens: pipeline.candidate_tokens,
        token_budget: pipeline.token_budget,
        included_sources,
        excluded_sources,
        context_fingerprint: Some(pipeline.context_fingerprint),
        generated_at: Some(pipeline.generated_at.to_rfc3339()),
        warning: None,
    }
}

fn preview_plan_with_context(
    plan: &OrchestrationPlan,
    context: ExecutionContextPreview,
) -> RepoDeskResult<ExecutionPreview> {
    let cost = load_cost_config()?;
    let estimated_input_tokens = if context.prepared && context.context_tokens > 0 {
        context.context_tokens
    } else {
        FALLBACK_INPUT_TOKENS
    };
    let steps: Vec<ExecutionPreviewStep> = plan
        .steps
        .iter()
        .map(|step| preview_step(step, &cost, estimated_input_tokens))
        .collect();

    let total_estimated_tokens = steps
        .iter()
        .map(|step| step.estimated_input_tokens + step.estimated_output_tokens)
        .sum();
    let total_estimated_cost_units = steps.iter().map(|step| step.estimated_cost_units).sum();

    Ok(ExecutionPreview {
        goal: plan.goal.clone(),
        steps,
        context,
        total_estimated_tokens,
        total_estimated_cost_units,
        currency_label: cost.currency_label.clone(),
        expected_writes: plan.steps.iter().any(|step| step.allow_write),
        isolated_workspace: plan_has_coding_agent_step(plan),
        requires_coding_agent_approval: plan_has_coding_agent_step(plan),
        requires_paid_approval: plan_has_paid_provider_step(plan),
    })
}

/// Build the preview for an already-routed plan. This compatibility entry point
/// uses the old fallback input estimate because it may be called without an
/// active Work Item; desktop execution preview uses prepared context evidence.
pub fn preview_plan(plan: &OrchestrationPlan) -> RepoDeskResult<ExecutionPreview> {
    preview_plan_with_context(plan, ExecutionContextPreview::default())
}

/// Route the active task into a plan and preview it — the same `build_plan` the
/// run uses — while attaching the already-prepared canonical packet facts from
/// Context Evidence. No provider is called and no context is rebuilt here.
pub fn execution_preview(
    goal: Option<String>,
    settings: &ProviderSettings,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> RepoDeskResult<ExecutionPreview> {
    let plan = build_plan(goal, settings, override_provider, override_model)?;
    preview_plan_with_context(&plan, active_context_preview())
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

    fn plan() -> OrchestrationPlan {
        OrchestrationPlan {
            project: "demo".into(),
            task_id: "t".into(),
            goal: "do the thing".into(),
            steps: vec![
                step("analyze", "ollama", ExecutorKind::LocalRuntime, false),
                step("implement", "codex_cli", ExecutorKind::CodingAgent, true),
            ],
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
        let preview = preview_plan(&plan()).expect("preview");
        assert_eq!(preview.steps.len(), 2);
        assert!(preview.expected_writes);
        assert!(preview.isolated_workspace);
        assert!(preview.requires_coding_agent_approval);
        assert!(!preview.context.prepared);
        let implement = preview
            .steps
            .iter()
            .find(|step| step.step_id == "implement")
            .unwrap();
        assert!(implement.allow_write);
        assert!(implement.isolated_workspace);
        assert_eq!(implement.executor_label, "Codex CLI");
        assert_eq!(preview.total_estimated_tokens, (4_000 + 1_500) * 2);
    }

    #[test]
    fn prepared_packet_drives_input_and_cost_estimate() {
        let context = ExecutionContextPreview {
            prepared: true,
            context_tokens: 2_400,
            candidate_tokens: 8_000,
            token_budget: Some(7_200),
            included_sources: 7,
            excluded_sources: 4,
            context_fingerprint: Some("abc123".into()),
            generated_at: Some("2026-08-12T12:00:00Z".into()),
            warning: None,
        };
        let preview = preview_plan_with_context(&plan(), context).expect("preview");

        assert_eq!(preview.steps[0].estimated_input_tokens, 2_400);
        assert_eq!(preview.steps[1].estimated_input_tokens, 2_400);
        assert_eq!(preview.context.included_sources, 7);
        assert_eq!(preview.context.excluded_sources, 4);
        assert_eq!(preview.total_estimated_tokens, (2_400 + 1_500) * 2);
    }
}
