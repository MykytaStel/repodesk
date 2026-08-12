//! Best-effort instrumentation that projects existing RepoDesk workflows into
//! the append-only Engineering Event Ledger.
//!
//! The engineering workflow remains the source of truth. Telemetry failures
//! must never make an otherwise-successful task, run, review, verification, or
//! commit fail, so callers intentionally ignore errors returned by these helpers.

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::engineering::domain::{
    ChangeSet, ChangeSetId, EvidenceKind, EvidenceRef, ExecutionId, VerificationId, WorkItem,
    WorkItemId, WorkerKind, WorkerRef,
};
use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, append_event};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::orchestrator::types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, SubAgentResult, SubAgentStatus,
};
use crate::tasks::{TaskConfig, show_active_task};

fn domain_error(error: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Api(format!("engineering instrumentation: {error}"))
}

fn work_item_id(value: &str) -> RepoDeskResult<WorkItemId> {
    WorkItemId::try_new(value.to_string()).map_err(domain_error)
}

fn execution_id(value: &str) -> RepoDeskResult<ExecutionId> {
    ExecutionId::try_new(value.to_string()).map_err(domain_error)
}

fn changeset_id(run_id: &str) -> RepoDeskResult<ChangeSetId> {
    ChangeSetId::try_new(format!("{run_id}-changeset")).map_err(domain_error)
}

fn event_for_task(
    task: &TaskConfig,
    kind: EngineeringEventKind,
) -> RepoDeskResult<EngineeringEvent> {
    Ok(EngineeringEvent::new(
        task.project_name.clone(),
        work_item_id(&task.id)?,
        kind,
    ))
}

fn append_for_task(task: &TaskConfig, event: EngineeringEvent) -> RepoDeskResult<()> {
    append_event(&task.run_dir, &event)?;
    Ok(())
}

fn active_task_for_run(run: &OrchestrationRun) -> RepoDeskResult<TaskConfig> {
    let task = show_active_task()?.config;
    if task.id != run.task_id || task.project_name != run.project {
        return Err(RepoDeskError::Api(format!(
            "engineering instrumentation active task mismatch: run {}/{} vs task {}/{}",
            run.project, run.task_id, task.project_name, task.id
        )));
    }
    Ok(task)
}

fn parse_event_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Completed => "completed",
        RunStatus::Partial => "partial",
        RunStatus::Failed => "failed",
        RunStatus::DryRun => "dry_run",
    }
}

fn result_is_handoff_target(result: &SubAgentResult) -> bool {
    result.input_tokens > 0 && matches!(result.status, SubAgentStatus::Ok | SubAgentStatus::Failed)
}

fn execution_mode_label(workers: &BTreeSet<WorkerRef>) -> &'static str {
    if workers
        .iter()
        .any(|worker| worker.kind == WorkerKind::Manual)
    {
        "manual"
    } else {
        "managed"
    }
}

/// Record creation of the RepoDesk 2 Work Item projected from a legacy task.
pub fn record_work_item_created(task: &TaskConfig) -> RepoDeskResult<()> {
    let work_item = WorkItem::try_from(task).map_err(domain_error)?;
    let event = event_for_task(task, EngineeringEventKind::WorkItemCreated)?
        .with_attribute("title", Value::String(work_item.title))
        .with_attribute(
            "verify_command_configured",
            Value::Bool(work_item.verify_command.is_some()),
        );
    append_for_task(task, event)
}

/// Record a concrete context artifact and its conservative token estimate.
pub fn record_context_built(
    task: &TaskConfig,
    context_file: &str,
    token_estimate_file: &str,
    estimated_tokens: usize,
) -> RepoDeskResult<()> {
    let mut event = event_for_task(task, EngineeringEventKind::ContextBuilt)?
        .with_attribute("estimated_tokens", json!(estimated_tokens));

    if let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Context, context_file.to_string()) {
        event = event.with_evidence(evidence);
    }
    if let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Other, token_estimate_file.to_string())
    {
        event = event.with_evidence(evidence);
    }

    append_for_task(task, event)
}

/// Record a persisted orchestration run as a pair of execution events plus
/// concrete worker handoffs and an optional changeset event. This is called
/// after the run finishes, but preserves the run's own start/finish timestamps.
pub fn record_orchestration_run(
    plan: Option<&OrchestrationPlan>,
    run: &OrchestrationRun,
) -> RepoDeskResult<()> {
    let task = active_task_for_run(run)?;
    let work_id = work_item_id(&run.task_id)?;
    let exec_id = execution_id(&run.run_id)?;

    let workers: BTreeSet<WorkerRef> = run
        .results
        .iter()
        .map(WorkerRef::from_legacy_result)
        .collect();
    let execution_mode = execution_mode_label(&workers);

    let mut started = EngineeringEvent::new(
        run.project.clone(),
        work_id.clone(),
        EngineeringEventKind::ExecutionStarted,
    )
    .with_execution(exec_id.clone())
    .with_attribute("dry_run", Value::Bool(run.dry_run))
    .with_attribute("execution_mode", Value::String(execution_mode.to_string()))
    .with_attribute("step_count", json!(run.results.len()))
    .with_attribute("worker_count", json!(workers.len()))
    .with_attribute("workers", json!(&workers));
    started.occurred_at = parse_event_time(&run.started_at);
    append_for_task(&task, started)?;

    if let Some(plan) = plan {
        let results: HashMap<&str, &SubAgentResult> = run
            .results
            .iter()
            .map(|result| (result.task_id.as_str(), result))
            .collect();

        for target_step in &plan.steps {
            let Some(target_result) = results.get(target_step.id.as_str()).copied() else {
                continue;
            };
            if !result_is_handoff_target(target_result) {
                continue;
            }
            let target_worker = WorkerRef::from_legacy_result(target_result);

            for source_step_id in &target_step.depends_on {
                let Some(source_result) = results.get(source_step_id.as_str()).copied() else {
                    continue;
                };
                if source_result.status != SubAgentStatus::Ok {
                    continue;
                }
                let source_worker = WorkerRef::from_legacy_result(source_result);
                if source_worker == target_worker {
                    continue;
                }

                let handoff = EngineeringEvent::new(
                    run.project.clone(),
                    work_id.clone(),
                    EngineeringEventKind::WorkerHandoff,
                )
                .with_execution(exec_id.clone())
                .with_worker(target_worker.clone())
                .with_attribute("from_worker", Value::String(source_worker.id.clone()))
                .with_attribute("to_worker", Value::String(target_worker.id.clone()))
                .with_attribute("source_step", Value::String(source_step_id.clone()))
                .with_attribute("target_step", Value::String(target_step.id.clone()));
                append_for_task(&task, handoff)?;
            }
        }
    }

    let mut finished = EngineeringEvent::new(
        run.project.clone(),
        work_id.clone(),
        EngineeringEventKind::ExecutionFinished,
    )
    .with_execution(exec_id.clone())
    .with_attribute(
        "status",
        Value::String(run_status_label(run.status).to_string()),
    )
    .with_attribute("dry_run", Value::Bool(run.dry_run))
    .with_attribute("execution_mode", Value::String(execution_mode.to_string()))
    .with_attribute("input_tokens", json!(run.total_input_tokens))
    .with_attribute("output_tokens", json!(run.total_output_tokens))
    .with_attribute("cost_units", json!(run.total_cost_units))
    .with_attribute("result_count", json!(run.results.len()))
    .with_attribute("worker_count", json!(workers.len()))
    .with_attribute("workers", json!(&workers));
    finished.occurred_at = parse_event_time(&run.finished_at);
    append_for_task(&task, finished)?;

    if let Some(changeset) = ChangeSet::try_from_run(run).map_err(domain_error)? {
        let mut event = EngineeringEvent::new(
            run.project.clone(),
            work_id,
            EngineeringEventKind::ChangeSetCreated,
        )
        .with_execution(exec_id)
        .with_changeset(changeset.id)
        .with_attribute("file_count", json!(changeset.files.len()))
        .with_attribute("files", json!(changeset.files));
        for evidence in changeset.evidence {
            event = event.with_evidence(evidence);
        }
        append_for_task(&task, event)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct AiStrategyTelemetry<'a> {
    pub requested_mode: &'a str,
    pub resolved_profile: &'a str,
    pub plan_shape: &'a str,
    pub plan_fingerprint: &'a str,
    pub baseline_steps: usize,
    pub planned_steps: usize,
    pub estimated_saved_tokens: usize,
    pub context_fingerprint: Option<&'a str>,
}

/// Record which evidence-backed AI strategy produced a concrete run. This is
/// best-effort telemetry only: failure to append it must never rewrite the
/// already-observed execution outcome.
pub fn record_ai_strategy_selected(
    run: &OrchestrationRun,
    telemetry: AiStrategyTelemetry<'_>,
) -> RepoDeskResult<()> {
    let task = active_task_for_run(run)?;
    let event = event_for_task(&task, EngineeringEventKind::AiStrategySelected)?
        .with_execution(execution_id(&run.run_id)?)
        .with_attribute(
            "requested_mode",
            Value::String(telemetry.requested_mode.to_string()),
        )
        .with_attribute(
            "resolved_profile",
            Value::String(telemetry.resolved_profile.to_string()),
        )
        .with_attribute(
            "plan_shape",
            Value::String(telemetry.plan_shape.to_string()),
        )
        .with_attribute(
            "plan_fingerprint",
            Value::String(telemetry.plan_fingerprint.to_string()),
        )
        .with_attribute("baseline_steps", json!(telemetry.baseline_steps))
        .with_attribute("planned_steps", json!(telemetry.planned_steps))
        .with_attribute(
            "estimated_saved_tokens",
            json!(telemetry.estimated_saved_tokens),
        );
    let event = if let Some(fingerprint) = telemetry.context_fingerprint {
        event.with_attribute(
            "context_fingerprint",
            Value::String(fingerprint.to_string()),
        )
    } else {
        event
    };
    append_for_task(&task, event)
}

pub fn record_changeset_reviewed(
    task: &TaskConfig,
    run_id: &str,
    decision: &str,
    reviewed_paths: &[String],
    digest: &str,
) -> RepoDeskResult<()> {
    let event = event_for_task(task, EngineeringEventKind::ChangeSetReviewed)?
        .with_execution(execution_id(run_id)?)
        .with_changeset(changeset_id(run_id)?)
        .with_attribute("decision", Value::String(decision.to_string()))
        .with_attribute("file_count", json!(reviewed_paths.len()))
        .with_attribute("files", json!(reviewed_paths))
        .with_attribute("changeset_digest", Value::String(digest.to_string()));
    append_for_task(task, event)
}

pub fn new_verification_id(run_id: &str) -> RepoDeskResult<VerificationId> {
    VerificationId::try_new(format!("verify-{run_id}-{}", Utc::now().timestamp_micros()))
        .map_err(domain_error)
}

pub fn record_verification_started(
    task: &TaskConfig,
    run_id: &str,
    verification_id: VerificationId,
) -> RepoDeskResult<()> {
    let event = event_for_task(task, EngineeringEventKind::VerificationStarted)?
        .with_execution(execution_id(run_id)?)
        .with_changeset(changeset_id(run_id)?)
        .with_verification(verification_id);
    append_for_task(task, event)
}

#[derive(Debug, Clone, Copy)]
pub struct VerificationFinishedTelemetry<'a> {
    pub success: bool,
    pub command_count: usize,
    pub summary_path: Option<&'a str>,
    pub log_path: Option<&'a str>,
    pub error: Option<&'a str>,
}

pub fn record_verification_finished(
    task: &TaskConfig,
    run_id: &str,
    verification_id: VerificationId,
    telemetry: VerificationFinishedTelemetry<'_>,
) -> RepoDeskResult<()> {
    let mut event = event_for_task(task, EngineeringEventKind::VerificationFinished)?
        .with_execution(execution_id(run_id)?)
        .with_changeset(changeset_id(run_id)?)
        .with_verification(verification_id)
        .with_attribute("success", Value::Bool(telemetry.success))
        .with_attribute("command_count", json!(telemetry.command_count));

    if let Some(error) = telemetry.error {
        event = event.with_attribute("error", Value::String(error.to_string()));
    }
    if let Some(path) = telemetry.summary_path
        && let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Verification, path.to_string())
    {
        event = event.with_evidence(evidence);
    }
    if let Some(path) = telemetry.log_path
        && let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Verification, path.to_string())
    {
        event = event.with_evidence(evidence);
    }

    append_for_task(task, event)
}

pub fn record_commit_created(
    task: &TaskConfig,
    run_id: &str,
    commit_sha: &str,
    committed_paths: &[String],
) -> RepoDeskResult<()> {
    let mut event = event_for_task(task, EngineeringEventKind::CommitCreated)?
        .with_execution(execution_id(run_id)?)
        .with_changeset(changeset_id(run_id)?)
        .with_attribute("file_count", json!(committed_paths.len()))
        .with_attribute("files", json!(committed_paths));
    if let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Commit, commit_sha.to_string()) {
        event = event.with_evidence(evidence);
    }
    append_for_task(task, event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_ids_are_run_scoped() {
        let id = new_verification_id("run-1").unwrap();
        assert!(id.as_str().starts_with("verify-run-1-"));
    }
}
