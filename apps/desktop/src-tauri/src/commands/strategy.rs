//! Desktop execution boundary for evidence-backed AI strategies.
//!
//! This intentionally lives beside the legacy Orchestrate commands so existing
//! advanced/manual flows keep their stable contract. Work uses these commands to
//! preview and launch the same fingerprinted strategy-shaped plan.

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::engineering::{AiPlanShape, AiStrategyMode, StrategySelectionTelemetry};
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
    if let Some(value) = &goal {
        let chars = value.chars().count();
        if chars > MAX_GOAL_LEN {
            return Err(ErrorPayload::resource_limit(format!(
                "goal is too long ({chars} > {MAX_GOAL_LEN} chars)"
            )));
        }
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

fn validate_approval_plan_lock(
    dry_run: bool,
    requires_approval: bool,
    plan_fingerprint: &str,
    approval_plan_fingerprint: Option<&str>,
) -> Result<(), ErrorPayload> {
    if dry_run || !requires_approval {
        return Ok(());
    }

    let approved = approval_plan_fingerprint
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if approved != Some(plan_fingerprint) {
        return Err(ErrorPayload::configuration(
            "Execution approvals are stale or missing: paid/write capabilities must be approved for the exact current plan lock. Refresh the Execution Packet and approve it again."
                .to_string(),
        ));
    }

    Ok(())
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
    approval_plan_fingerprint: Option<String>,
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

    if !dry_run && !prepared.preview.execution.context.prepared {
        return Err(ErrorPayload::configuration(
            "Strategy execution requires a prepared Context Pipeline. Return to Prepare and rebuild the AI context packet before launching."
                .to_string(),
        ));
    }

    let expected_fingerprint = expected_plan_fingerprint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !dry_run && expected_fingerprint.is_none() {
        return Err(ErrorPayload::configuration(
            "Strategy execution requires an approved preview plan lock. Refresh the Execution Packet before launching."
                .to_string(),
        ));
    }
    if let Some(expected) = expected_fingerprint
        && expected != prepared.preview.plan_fingerprint
    {
        return Err(ErrorPayload::configuration(
            "Execution preview is stale: routing, context, or AI strategy changed. Refresh the Execution Packet before launching."
                .to_string(),
        ));
    }

    let requires_approval = prepared.preview.execution.requires_coding_agent_approval
        || prepared.preview.execution.requires_paid_approval;
    validate_approval_plan_lock(
        dry_run,
        requires_approval,
        &prepared.preview.plan_fingerprint,
        approval_plan_fingerprint.as_deref(),
    )?;

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

    // Bind intent evidence to the exact execution identity before any provider
    // request or coding-agent process is launched. Telemetry is best-effort and
    // never weakens the execution/receipt gates.
    let run_id = orchestrator::reserve_run_id();
    let preview = &prepared.preview;
    let _ = repodesk_core::engineering::record_strategy_selection_for_execution(
        &prepared.plan.project,
        &prepared.plan.task_id,
        &run_id,
        StrategySelectionTelemetry {
            requested_mode: preview.strategy.requested_mode.as_label(),
            resolved_profile: preview.strategy.profile.as_label(),
            plan_shape: plan_shape_label(preview.strategy.plan_shape),
            plan_fingerprint: &preview.plan_fingerprint,
            baseline_steps: preview.comparison.baseline_steps,
            planned_steps: preview.comparison.planned_steps,
            baseline_estimated_tokens: preview.comparison.baseline_estimated_tokens,
            planned_estimated_tokens: preview.comparison.planned_estimated_tokens,
            estimated_saved_tokens: preview.comparison.estimated_saved_tokens,
            baseline_estimated_cost_units: preview.comparison.baseline_estimated_cost_units,
            planned_estimated_cost_units: preview.comparison.planned_estimated_cost_units,
            context_fingerprint: preview.execution.context.context_fingerprint.as_deref(),
        },
    );

    let run = orchestrator::run_plan_with_id(&prepared.plan, &opts, run_id).await?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_labels_are_conservative() {
        assert_eq!(
            parse_strategy_mode(Some("auto".into())).unwrap(),
            AiStrategyMode::Auto
        );
        assert_eq!(
            parse_strategy_mode(Some("local-first".into())).unwrap(),
            AiStrategyMode::LocalFirst
        );
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

    #[test]
    fn goal_limit_counts_unicode_characters_not_utf8_bytes() {
        assert!(clean_goal(Some("ї".repeat(MAX_GOAL_LEN))).is_ok());
        assert!(clean_goal(Some("ї".repeat(MAX_GOAL_LEN + 1))).is_err());
    }

    #[test]
    fn capability_approval_must_match_exact_plan_lock() {
        assert!(validate_approval_plan_lock(false, true, "plan-b", Some("plan-a")).is_err());
        assert!(validate_approval_plan_lock(false, true, "plan-b", None).is_err());
        assert!(validate_approval_plan_lock(false, true, "plan-b", Some("plan-b")).is_ok());
    }

    #[test]
    fn dry_run_or_ungated_plan_does_not_require_capability_approval_lock() {
        assert!(validate_approval_plan_lock(true, true, "plan-b", None).is_ok());
        assert!(validate_approval_plan_lock(false, false, "plan-b", None).is_ok());
    }
}
