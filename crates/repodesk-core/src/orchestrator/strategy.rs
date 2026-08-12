//! Strategy-aware planning layered over the stable orchestrator planner.
//!
//! The legacy/default planner remains unchanged. This adapter derives an
//! explainable strategy from the Work Item's engineering ledger, then narrows
//! the AI plan shape and/or re-routes read-only reasoning through the existing
//! routing engine. Human review, verification and commit gates are untouched.

use crate::api_clients::ProviderSettings;
use crate::engineering::{
    AiPlanShape, AiStrategyInputs, AiStrategyMode, AiStrategyProfile, AiStrategyRecommendation,
    derive_ai_strategy, derive_ai_usage_report, derive_engineering_intelligence,
    load_context_inspector, read_events, read_work_item_contract,
};
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::routing::engine::route_request_with_bias;
use crate::routing::types::{RouteRequest, TaskKind};
use crate::tasks::show_active_task;
use crate::usage::budget::load_budget_config;

use super::plan::{available_capacities, build_plan};
use super::types::OrchestrationPlan;

const STRATEGY_ROUTE_INPUT_TOKENS: usize = 4_000;

pub fn derive_active_ai_strategy(
    requested_mode: AiStrategyMode,
) -> RepoDeskResult<AiStrategyRecommendation> {
    let task = show_active_task()?;
    let events = read_events(&task.config.run_dir)?;
    let intelligence = derive_engineering_intelligence(&events);
    let usage = derive_ai_usage_report(&events, &intelligence);
    let contract = read_work_item_contract(&task.config.run_dir)?;
    let context_prepared = load_context_inspector(&task.config.run_dir)
        .ok()
        .is_some_and(|report| report.pipeline_error.is_none() && report.pipeline.is_some());
    let inputs = AiStrategyInputs {
        scope_path_count: contract
            .as_ref()
            .map(|contract| contract.allowed_paths.len())
            .unwrap_or(0),
        protected_path_count: contract
            .as_ref()
            .map(|contract| contract.protected_paths.len())
            .unwrap_or(0),
        context_prepared,
    };

    Ok(derive_ai_strategy(&usage, inputs, requested_mode))
}

/// Build a plan using the normal planner/router first, then apply the selected
/// strategy. Explicit provider/model overrides always win over strategy routing.
pub fn build_strategy_plan(
    goal: Option<String>,
    settings: &ProviderSettings,
    override_provider: Option<String>,
    override_model: Option<String>,
    requested_mode: AiStrategyMode,
) -> RepoDeskResult<(OrchestrationPlan, AiStrategyRecommendation)> {
    let strategy = derive_active_ai_strategy(requested_mode)?;
    let mut plan = build_plan(
        goal,
        settings,
        override_provider.clone(),
        override_model,
    )?;

    apply_plan_shape(&mut plan, strategy.plan_shape);
    if override_provider
        .as_deref()
        .is_none_or(|provider| provider.trim().is_empty())
    {
        reroute_read_only_steps(&mut plan, settings, &strategy)?;
    }

    Ok((plan, strategy))
}

fn apply_plan_shape(plan: &mut OrchestrationPlan, shape: AiPlanShape) {
    match shape {
        AiPlanShape::AnalyzeWriterReview => {}
        AiPlanShape::WriterWithReview => {
            plan.steps
                .retain(|step| matches!(step.id.as_str(), "implement" | "review"));
            for step in &mut plan.steps {
                match step.id.as_str() {
                    "implement" => {
                        step.depends_on.clear();
                        step.instruction = direct_implementation_instruction();
                    }
                    "review" => step.depends_on = vec!["implement".to_string()],
                    _ => {}
                }
            }
        }
        AiPlanShape::SingleWriter => {
            plan.steps.retain(|step| step.id == "implement");
            if let Some(step) = plan.steps.first_mut() {
                step.depends_on.clear();
                step.instruction = direct_implementation_instruction();
            }
        }
    }
}

fn direct_implementation_instruction() -> String {
    "Implement the bounded Work Item directly from the prepared task, contract and repository context. Prefer the smallest safe diff, stay inside allowed paths, and do not touch secrets or unrelated files. The returned ChangeSet still requires human review and verification."
        .to_string()
}

fn reroute_read_only_steps(
    plan: &mut OrchestrationPlan,
    settings: &ProviderSettings,
    strategy: &AiStrategyRecommendation,
) -> RepoDeskResult<()> {
    if strategy.profile == AiStrategyProfile::Balanced {
        return Ok(());
    }

    let project = get_active_project()?;
    let budget = load_budget_config()?;
    let capacities = available_capacities(settings, &budget);
    let bias = crate::outcomes::routing_bias(&project.name).unwrap_or_default();

    for step in plan.steps.iter_mut().filter(|step| !step.allow_write) {
        let request = RouteRequest {
            task_kind: step.kind,
            estimated_input_tokens: STRATEGY_ROUTE_INPUT_TOKENS,
            estimated_output_tokens: step.budget_tokens,
            risk_level: "medium".to_string(),
            changed_file_count: 0,
            requires_write: false,
            context_safe: Some(true),
            checks_ok: None,
            guard_allowed: Some(true),
            git_dirty: None,
            max_cost_units: None,
            economy_mode: Some(strategy.economy_mode.clone()),
        };
        let decision = route_request_with_bias(&request, &capacities, &budget, &bias);

        step.agent = decision.recommended_executor_id.clone();
        step.provider = decision
            .recommended_provider_id
            .clone()
            .unwrap_or_else(|| decision.recommended_provider.clone());
        step.executor_kind = decision.recommended_executor_kind;
        step.executor_id = decision.recommended_executor_id;
        step.provider_id = decision.recommended_provider_id;
        step.model = decision.recommended_model;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_clients::ThinkingLevel;
    use crate::routing::types::ExecutorKind;
    use crate::orchestrator::types::SubAgentTask;

    fn step(id: &str, allow_write: bool, depends_on: &[&str]) -> SubAgentTask {
        SubAgentTask {
            id: id.into(),
            title: id.into(),
            kind: if allow_write { TaskKind::Patch } else { TaskKind::Review },
            agent: "agent".into(),
            provider: "provider".into(),
            executor_kind: if allow_write {
                ExecutorKind::CodingAgent
            } else {
                ExecutorKind::LocalRuntime
            },
            executor_id: "agent".into(),
            provider_id: Some("provider".into()),
            model: Some("model".into()),
            thinking: ThinkingLevel::None,
            instruction: "legacy instruction".into(),
            depends_on: depends_on.iter().map(|value| value.to_string()).collect(),
            budget_tokens: 1_500,
            allow_write,
            verify_command: None,
        }
    }

    fn plan() -> OrchestrationPlan {
        OrchestrationPlan {
            project: "demo".into(),
            task_id: "task".into(),
            goal: "goal".into(),
            steps: vec![
                step("analyze", false, &[]),
                step("implement", true, &["analyze"]),
                step("review", false, &["implement"]),
            ],
        }
    }

    #[test]
    fn single_writer_removes_ai_overhead_but_keeps_bounded_writer() {
        let mut value = plan();
        apply_plan_shape(&mut value, AiPlanShape::SingleWriter);

        assert_eq!(value.steps.len(), 1);
        assert_eq!(value.steps[0].id, "implement");
        assert!(value.steps[0].depends_on.is_empty());
        assert!(value.steps[0].instruction.contains("human review"));
    }

    #[test]
    fn writer_with_review_relinks_dependencies_after_analysis_is_removed() {
        let mut value = plan();
        apply_plan_shape(&mut value, AiPlanShape::WriterWithReview);

        assert_eq!(value.steps.len(), 2);
        assert_eq!(value.steps[0].id, "implement");
        assert!(value.steps[0].depends_on.is_empty());
        assert_eq!(value.steps[1].depends_on, vec!["implement".to_string()]);
    }

    #[test]
    fn full_shape_preserves_original_pipeline() {
        let mut value = plan();
        apply_plan_shape(&mut value, AiPlanShape::AnalyzeWriterReview);
        assert_eq!(value.steps.len(), 3);
        assert_eq!(value.steps[1].depends_on, vec!["analyze".to_string()]);
    }
}
