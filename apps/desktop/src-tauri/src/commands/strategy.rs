//! Desktop execution boundary for evidence-backed AI strategies.
//!
//! This intentionally lives beside the legacy Orchestrate commands so existing
//! advanced/manual flows keep their stable contract. Work uses these commands to
//! preview and launch the same fingerprinted strategy-shaped plan.

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::engineering::{AiPlanShape, AiStrategyMode};
use repodesk_core::orchestrator::{
    self, AgentWorkspacePolicy, ExecutionAuthorization, OrchestrationRun, RunOptions,
    StrategyExecutionPreview,
};

use super::ErrorPayload;

const MAX_GOAL_LEN: usize = 2_000;

fn strategy_settings() -> ProviderSettings {
    let mut settings = ProviderSettings::from_env();
    if let Ok(saved) = crate::store::read_provider_settings() {
        settings.ollama.base_url = Some(saved.ollama_url);
        settings.ollama.default_model = Some(saved.ollama_model);
        settings.lm_studio.base_url = Some(saved.lm_studio_url);
        if !saved.anthropic_api_key.trim().is_empty() {
            settings.anthropic.api_key = Some(saved.anthropic_api_key);
        }
        if !saved.openai_api_key.trim().is_empty() {
            settings.openai.api_key = Some(saved.openai_api_key);
        }
        if !saved.gemini_api_key.trim().is_empty() {
            settings.gemini.api_key = Some(saved.gemini_api_key);
        }
    }
    settings
}

fn clean_goal(goal: Option<String>) -> Result<Option<String>, ErrorPayload> {
    if let Some(value) = &goal
        && value.len() > MAX_GOAL_LEN
    {
        return Err(ErrorPayload::resource_limit(format!(
            "goal is too long ({} > {MAX_GOAL_LEN} chars)",
            value.len()
        )));
    }
    Ok(goal.filter(|value| !value.trim().is_empty()))
}

fn parse_strategy_mode(value: Option<String>) -> Result<AiStrategyMode, ErrorPayload> {
    let value = value.unwrap_or_else(|| "auto".to_string());
    AiStrategyMode::from_label(&value).ok_or_else(|| {
        ErrorPayload::configuration(format!(
            "unknown AI strategy '{value}'; expected auto, lean, balanced, local_first, or quality"
        ))
    })
}

fn plan_shape_label(shape: AiPlanShape) -> &'static str {
    match shape {
        AiPlanShape::SingleWriter => "single_writer",
        AiPlanShape::WriterWithReview => "writer_with_review",
        AiPlanShape::AnalyzeWriterReview => "analyze_writer_review",
    }
}

#[tauri::command]
pub async fn work_strategy_execution_preview(
    goal: Option<String>,
    override_provider: Option<String>,
    override_model: Option<String>,
    strategy_mode: Option<String>,
) -> Result<StrategyExecutionPreview, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let mode = parse_strategy_mode(strategy_mode)?;
    Ok(orchestrator::prepare_strategy_execution(
        goal,
        &strategy_settings(),
        override_provider,
        override_model,
        mode,
    )?
    .preview)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn orchestrate_strategy_run(
    goal: Option<String>,
    dry_run: bool,
    max_cost: Option<f64>,
    approve_paid: bool,
    approve_coding_agents: bool,
    override_provider: Option<String>,
    override_model: Option<String>,
    strategy_mode: Option<String>,
    expected_plan_fingerprint: Option<String>,
) -> Result<OrchestrationRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let mode = parse_strategy_mode(strategy_mode)?;
    let settings = strategy_settings();
    let prepared = orchestrator::prepare_strategy_execution(
        goal,
        &settings,
        override_provider,
        override_model,
        mode,
    )?;

    if let Some(expected) = expected_plan_fingerprint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && expected != prepared.preview.plan_fingerprint
    {
        return Err(ErrorPayload::configuration(
            "Execution preview is stale: routing, context, or AI strategy changed. Refresh the Execution Packet before launching."
                .to_string(),
        ));
    }

    let opts = RunOptions {
        dry_run,
        max_cost,
        settings,
        authorization: ExecutionAuthorization {
            allow_paid_providers: approve_paid,
            allow_coding_agents: approve_coding_agents,
            allow_workspace_writes: approve_coding_agents,
        },
        coding_agent_timeout_secs: 600,
        agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
    };
    let run = orchestrator::run_plan(&prepared.plan, &opts).await?;

    // Strategy telemetry is deliberately best-effort. Execution and its
    // authoritative receipt have already completed; a ledger append failure
    // must not rewrite that outcome into a failed run.
    let preview = &prepared.preview;
    let _ = repodesk_core::engineering::instrumentation::record_ai_strategy_selected(
        &run,
        repodesk_core::engineering::instrumentation::AiStrategyTelemetry {
            requested_mode: preview.strategy.requested_mode.as_label(),
            resolved_profile: preview.strategy.profile.as_label(),
            plan_shape: plan_shape_label(preview.strategy.plan_shape),
            plan_fingerprint: &preview.plan_fingerprint,
            baseline_steps: preview.comparison.baseline_steps,
            planned_steps: preview.comparison.planned_steps,
            estimated_saved_tokens: preview.comparison.estimated_saved_tokens,
            context_fingerprint: preview.execution.context.context_fingerprint.as_deref(),
        },
    );

    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_labels_are_conservative() {
        assert_eq!(parse_strategy_mode(Some("auto".into())).unwrap(), AiStrategyMode::Auto);
        assert_eq!(parse_strategy_mode(Some("local-first".into())).unwrap(), AiStrategyMode::LocalFirst);
        assert!(parse_strategy_mode(Some("YOLO".into())).is_err());
    }

    #[test]
    fn plan_shape_labels_match_serialized_contract() {
        assert_eq!(plan_shape_label(AiPlanShape::SingleWriter), "single_writer");
        assert_eq!(
            plan_shape_label(AiPlanShape::AnalyzeWriterReview),
            "analyze_writer_review"
        );
    }

    #[test]
    fn long_goal_is_rejected_before_planning() {
        assert!(clean_goal(Some("x".repeat(MAX_GOAL_LEN + 1))).is_err());
    }
}
