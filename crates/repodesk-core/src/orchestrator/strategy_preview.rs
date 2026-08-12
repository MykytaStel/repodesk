//! Strategy-aware execution preparation shared by preview and launch.
//!
//! A single helper builds the strategy-shaped plan, attaches the currently
//! prepared Context Pipeline, compares it with the stable three-step baseline,
//! and fingerprints the exact plan + strategy + context boundary. Desktop launch
//! can therefore reject a stale preview instead of silently executing a newly
//! routed plan.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api_clients::ProviderSettings;
use crate::context_pipeline::ContextSelectionState;
use crate::engineering::{AiStrategyMode, AiStrategyRecommendation, load_context_inspector};
use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;
use crate::usage::cost::{estimate_agent_cost, load_cost_config};

use super::plan::build_plan;
use super::preview::{ExecutionContextPreview, ExecutionPreview, preview_plan};
use super::strategy::build_strategy_plan;
use super::types::OrchestrationPlan;

const FALLBACK_INPUT_TOKENS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyBaselineComparison {
    pub baseline_steps: usize,
    pub planned_steps: usize,
    pub baseline_estimated_tokens: usize,
    pub planned_estimated_tokens: usize,
    pub estimated_saved_tokens: usize,
    /// Positive means the selected strategy is estimated to cost more than the
    /// baseline; negative means it is cheaper.
    pub estimated_cost_delta_units: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyExecutionPreview {
    pub execution: ExecutionPreview,
    pub strategy: AiStrategyRecommendation,
    pub comparison: StrategyBaselineComparison,
    pub plan_fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct PreparedStrategyExecution {
    pub plan: OrchestrationPlan,
    pub preview: StrategyExecutionPreview,
}

pub fn prepare_strategy_execution(
    goal: Option<String>,
    settings: &ProviderSettings,
    override_provider: Option<String>,
    override_model: Option<String>,
    requested_mode: AiStrategyMode,
) -> RepoDeskResult<PreparedStrategyExecution> {
    let (plan, strategy) = build_strategy_plan(
        goal.clone(),
        settings,
        override_provider.clone(),
        override_model.clone(),
        requested_mode,
    )?;
    let execution = preview_plan_with_active_context(&plan)?;

    let baseline_plan = build_plan(goal, settings, override_provider, override_model)?;
    let baseline = preview_plan_with_active_context(&baseline_plan)?;
    let comparison = StrategyBaselineComparison {
        baseline_steps: baseline.steps.len(),
        planned_steps: execution.steps.len(),
        baseline_estimated_tokens: baseline.total_estimated_tokens,
        planned_estimated_tokens: execution.total_estimated_tokens,
        estimated_saved_tokens: baseline
            .total_estimated_tokens
            .saturating_sub(execution.total_estimated_tokens),
        estimated_cost_delta_units: execution.total_estimated_cost_units
            - baseline.total_estimated_cost_units,
    };
    let plan_fingerprint = execution_fingerprint(&plan, &strategy, &execution.context)?;

    Ok(PreparedStrategyExecution {
        plan,
        preview: StrategyExecutionPreview {
            execution,
            strategy,
            comparison,
            plan_fingerprint,
        },
    })
}

fn preview_plan_with_active_context(plan: &OrchestrationPlan) -> RepoDeskResult<ExecutionPreview> {
    let mut preview = preview_plan(plan)?;
    let context = active_context_preview();
    let estimated_input_tokens = if context.prepared && context.context_tokens > 0 {
        context.context_tokens
    } else {
        FALLBACK_INPUT_TOKENS
    };
    let cost = load_cost_config()?;

    for (preview_step, plan_step) in preview.steps.iter_mut().zip(&plan.steps) {
        preview_step.estimated_input_tokens = estimated_input_tokens;
        preview_step.estimated_cost_units = estimate_agent_cost(
            &cost,
            plan_step.resolved_executor_id(),
            estimated_input_tokens,
            preview_step.estimated_output_tokens,
        )
        .estimated_cost_units;
    }
    preview.context = context;
    preview.total_estimated_tokens = preview
        .steps
        .iter()
        .map(|step| step.estimated_input_tokens + step.estimated_output_tokens)
        .sum();
    preview.total_estimated_cost_units = preview
        .steps
        .iter()
        .map(|step| step.estimated_cost_units)
        .sum();
    Ok(preview)
}

fn active_context_preview() -> ExecutionContextPreview {
    let Ok(task) = show_active_task() else {
        return ExecutionContextPreview::default();
    };
    let report = match load_context_inspector(&task.config.run_dir) {
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
        .filter(|selection| selection.state == ContextSelectionState::Included)
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

fn execution_fingerprint(
    plan: &OrchestrationPlan,
    strategy: &AiStrategyRecommendation,
    context: &ExecutionContextPreview,
) -> RepoDeskResult<String> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "plan": plan,
        "strategy": strategy,
        "context_fingerprint": context.context_fingerprint,
    }))?;
    Ok(hex::encode(Sha256::digest(payload)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_clients::ThinkingLevel;
    use crate::engineering::{AiPlanShape, AiStrategyProfile, AiStrategyReason};
    use crate::routing::types::{ExecutorKind, TaskKind};
    use crate::orchestrator::types::SubAgentTask;

    fn plan() -> OrchestrationPlan {
        OrchestrationPlan {
            project: "demo".into(),
            task_id: "task".into(),
            goal: "goal".into(),
            steps: vec![SubAgentTask {
                id: "implement".into(),
                title: "Implement".into(),
                kind: TaskKind::Patch,
                agent: "codex_cli".into(),
                provider: "codex_cli".into(),
                executor_kind: ExecutorKind::CodingAgent,
                executor_id: "codex_cli".into(),
                provider_id: None,
                model: Some("default".into()),
                thinking: ThinkingLevel::None,
                instruction: "implement".into(),
                depends_on: Vec::new(),
                budget_tokens: 1_500,
                allow_write: true,
                verify_command: None,
            }],
        }
    }

    fn strategy() -> AiStrategyRecommendation {
        AiStrategyRecommendation {
            requested_mode: AiStrategyMode::Lean,
            profile: AiStrategyProfile::Lean,
            plan_shape: AiPlanShape::SingleWriter,
            economy_mode: "economy".into(),
            reuse_prepared_context: true,
            max_agent_steps: 1,
            independent_ai_review: false,
            reasons: Vec::<AiStrategyReason>::new(),
        }
    }

    #[test]
    fn fingerprint_changes_when_context_boundary_changes() {
        let first = ExecutionContextPreview {
            prepared: true,
            context_fingerprint: Some("one".into()),
            ..ExecutionContextPreview::default()
        };
        let second = ExecutionContextPreview {
            prepared: true,
            context_fingerprint: Some("two".into()),
            ..ExecutionContextPreview::default()
        };
        let a = execution_fingerprint(&plan(), &strategy(), &first).unwrap();
        let b = execution_fingerprint(&plan(), &strategy(), &second).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn baseline_comparison_can_represent_extra_quality_cost() {
        let comparison = StrategyBaselineComparison {
            baseline_steps: 3,
            planned_steps: 3,
            baseline_estimated_tokens: 10_000,
            planned_estimated_tokens: 10_000,
            estimated_saved_tokens: 0,
            estimated_cost_delta_units: 0.2,
        };
        assert!(comparison.estimated_cost_delta_units > 0.0);
    }
}
