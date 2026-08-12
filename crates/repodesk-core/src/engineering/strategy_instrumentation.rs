//! Strategy-specific evidence writer.
//!
//! The general workflow instrumentation remains stable. New strategy runs use
//! this enriched structural event so intended-vs-actual feedback can compare the
//! exact preview prediction with the eventual execution/review/verification
//! evidence without persisting prompts or model outputs.

use serde_json::{Value, json};

use super::events::{EngineeringEvent, EngineeringEventKind, append_event};
use crate::engineering::domain::{ExecutionId, WorkItemId};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::orchestrator::OrchestrationRun;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, Copy)]
pub struct StrategySelectionTelemetry<'a> {
    pub requested_mode: &'a str,
    pub resolved_profile: &'a str,
    pub plan_shape: &'a str,
    pub plan_fingerprint: &'a str,
    pub baseline_steps: usize,
    pub planned_steps: usize,
    pub baseline_estimated_tokens: usize,
    pub planned_estimated_tokens: usize,
    pub estimated_saved_tokens: usize,
    pub baseline_estimated_cost_units: f64,
    pub planned_estimated_cost_units: f64,
    pub context_fingerprint: Option<&'a str>,
}

pub fn record_strategy_selection(
    run: &OrchestrationRun,
    telemetry: StrategySelectionTelemetry<'_>,
) -> RepoDeskResult<()> {
    let task = show_active_task()?.config;
    if task.id != run.task_id || task.project_name != run.project {
        return Err(RepoDeskError::Api(format!(
            "strategy instrumentation active task mismatch: run {}/{} vs task {}/{}",
            run.project, run.task_id, task.project_name, task.id
        )));
    }
    let work_item_id = WorkItemId::try_new(run.task_id.clone())
        .map_err(|error| RepoDeskError::Api(format!("strategy instrumentation: {error}")))?;
    let execution_id = ExecutionId::try_new(run.run_id.clone())
        .map_err(|error| RepoDeskError::Api(format!("strategy instrumentation: {error}")))?;

    let mut event = EngineeringEvent::new(
        run.project.clone(),
        work_item_id,
        EngineeringEventKind::AiStrategySelected,
    )
    .with_execution(execution_id)
    .with_attribute(
        "requested_mode",
        Value::String(telemetry.requested_mode.to_string()),
    )
    .with_attribute(
        "resolved_profile",
        Value::String(telemetry.resolved_profile.to_string()),
    )
    .with_attribute("plan_shape", Value::String(telemetry.plan_shape.to_string()))
    .with_attribute(
        "plan_fingerprint",
        Value::String(telemetry.plan_fingerprint.to_string()),
    )
    .with_attribute("baseline_steps", json!(telemetry.baseline_steps))
    .with_attribute("planned_steps", json!(telemetry.planned_steps))
    .with_attribute(
        "baseline_estimated_tokens",
        json!(telemetry.baseline_estimated_tokens),
    )
    .with_attribute(
        "planned_estimated_tokens",
        json!(telemetry.planned_estimated_tokens),
    )
    .with_attribute(
        "estimated_saved_tokens",
        json!(telemetry.estimated_saved_tokens),
    )
    .with_attribute(
        "baseline_estimated_cost_units",
        json!(telemetry.baseline_estimated_cost_units),
    )
    .with_attribute(
        "planned_estimated_cost_units",
        json!(telemetry.planned_estimated_cost_units),
    );

    if let Some(context_fingerprint) = telemetry.context_fingerprint {
        event = event.with_attribute(
            "context_fingerprint",
            Value::String(context_fingerprint.to_string()),
        );
    }

    append_event(&task.run_dir, &event).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_shape_keeps_prediction_values_explicit() {
        let telemetry = StrategySelectionTelemetry {
            requested_mode: "auto",
            resolved_profile: "lean",
            plan_shape: "single_writer",
            plan_fingerprint: "abc",
            baseline_steps: 3,
            planned_steps: 1,
            baseline_estimated_tokens: 12_000,
            planned_estimated_tokens: 4_000,
            estimated_saved_tokens: 8_000,
            baseline_estimated_cost_units: 0.3,
            planned_estimated_cost_units: 0.1,
            context_fingerprint: Some("context"),
        };
        assert_eq!(telemetry.baseline_steps - telemetry.planned_steps, 2);
        assert_eq!(telemetry.estimated_saved_tokens, 8_000);
    }
}
