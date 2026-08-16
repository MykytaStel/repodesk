//! Persistence boundary between execution outcome and workflow evidence.
//!
//! The raw runner owns provider/process execution and its historical run file.
//! This module is the public execution boundary: once the runner returns a real
//! run, failure to persist the Work-flow execution receipt is no longer allowed
//! to masquerade as an execution failure or disappear silently. Receipt failures
//! become an explicit, recoverable state, and Review is blocked until that state
//! is repaired.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::change_attribution::classify_step_attribution;
#[cfg(test)]
use crate::change_evidence::ChangeEvidenceStatus;
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::event_journal::{LogEventInput, log_event};
use crate::tasks::show_active_task;
use crate::workflow::phase::ExecutionMode;
use crate::workflow::receipt::{
    ExecutionReceipt, StepReceipt, TaskRunReceipt, changeset_digest, head_sha,
    load_receipt_for_run, save_receipt,
};

use super::runner::{self, RunOptions};
use super::types::{OrchestrationPlan, OrchestrationRun};

const MAX_RECOVERY_BYTES: u64 = 1024 * 1024;

/// Whether execution evidence is safe to consume by Review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceStatus {
    /// The exact run has a persisted execution receipt.
    Ready,
    /// The agent already ran, but the execution receipt is missing/unusable.
    RecoveryRequired,
    /// The receipt exists, but its captured changeset provenance is not review-safe.
    Incomplete,
    /// Dry runs intentionally carry no execution receipt.
    NotRequired,
}

/// Public evidence state for one persisted orchestration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvidenceState {
    pub run_id: String,
    pub status: ExecutionEvidenceStatus,
    /// True when RepoDesk has a durable recovery payload that can be replayed
    /// without executing the agent again.
    pub recoverable: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceRecoveryRecord {
    run_id: String,
    receipt: TaskRunReceipt,
    persistence_error: String,
    recorded_at: String,
}

/// Public orchestration execution boundary. Provider/process failures still
/// return normally through the run's `RunStatus`. Once the raw runner itself
/// returns a completed run object, execution-receipt persistence is handled as a
/// separate state: it may require evidence repair, but it never rewrites the
/// already-observed execution outcome into a generic execution error.
pub async fn run_plan(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
) -> RepoDeskResult<OrchestrationRun> {
    run_plan_with_id(plan, opts, runner::reserve_run_id()).await
}

/// Evidence-aware execution boundary for callers that must reserve the run id
/// before launch (for example strategy-selection telemetry). The reserved id
/// changes identity timing only; it must never bypass receipt finalization.
pub async fn run_plan_with_id(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
    run_id: String,
) -> RepoDeskResult<OrchestrationRun> {
    let run = runner::run_plan_with_id(plan, opts, run_id).await?;
    finalize_after_execution(plan, run)
}

fn finalize_after_execution(
    plan: &OrchestrationPlan,
    run: OrchestrationRun,
) -> RepoDeskResult<OrchestrationRun> {
    if !run.dry_run {
        match finalize_execution_evidence(plan, &run) {
            Ok(state) if state.status == ExecutionEvidenceStatus::RecoveryRequired => {
                log_recovery_required(&run.run_id, state.detail.as_deref());
            }
            Ok(_) => {}
            Err(error) => {
                // Execution has already happened. Never turn this into an
                // execution failure; Review will fail closed because the receipt
                // is absent, while this warning preserves the persistence fault.
                log_recovery_required(&run.run_id, Some(&error.to_string()));
            }
        }
    }

    Ok(run)
}

/// Inspect whether Review may trust execution evidence for `run_id`.
pub fn evidence_state_for_run(run_id: &str) -> RepoDeskResult<ExecutionEvidenceState> {
    validate_run_id(run_id)?;
    let run = runner::load_run(run_id)?.ok_or_else(|| {
        routing_error(format!(
            "no orchestration run '{run_id}' for the active task"
        ))
    })?;

    if run.dry_run {
        return Ok(ExecutionEvidenceState {
            run_id: run_id.to_string(),
            status: ExecutionEvidenceStatus::NotRequired,
            recoverable: false,
            detail: Some("dry runs intentionally do not create execution evidence".to_string()),
        });
    }

    match load_receipt_for_run(run_id) {
        Ok(Some(receipt)) if execution_receipt_matches_run(&receipt, &run) => {
            return Ok(matching_receipt_state(run_id, &receipt));
        }
        Ok(Some(_)) => {
            return recovery_state(
                run_id,
                "execution receipt exists but does not match the persisted run",
            );
        }
        Ok(None) => {}
        Err(error) => {
            return recovery_state(
                run_id,
                &format!("execution receipt could not be loaded: {error}"),
            );
        }
    }

    recovery_state(
        run_id,
        "execution completed but its workflow receipt is missing",
    )
}

/// Replay a durable recovery payload without launching the agent again.
pub fn repair_execution_evidence(run_id: &str) -> RepoDeskResult<ExecutionEvidenceState> {
    validate_run_id(run_id)?;
    let run = runner::load_run(run_id)?.ok_or_else(|| {
        routing_error(format!(
            "no orchestration run '{run_id}' for the active task"
        ))
    })?;
    if run.dry_run {
        return Ok(ExecutionEvidenceState {
            run_id: run_id.to_string(),
            status: ExecutionEvidenceStatus::NotRequired,
            recoverable: false,
            detail: Some("dry runs intentionally do not create execution evidence".to_string()),
        });
    }

    if let Ok(Some(receipt)) = load_receipt_for_run(run_id)
        && execution_receipt_matches_run(&receipt, &run)
    {
        let _ = clear_recovery_record(run_id);
        return Ok(matching_receipt_state(run_id, &receipt));
    }

    let record = read_recovery_record(run_id)?.ok_or_else(|| routing_error(format!(
        "execution evidence for '{run_id}' requires recovery, but no durable recovery payload is available; rerun the execution instead of reviewing unbound changes"
    )))?;
    if record.run_id != run_id || record.receipt.run_id != run_id {
        return Err(routing_error(
            "execution evidence recovery payload does not belong to the requested run",
        ));
    }

    save_receipt(&record.receipt)?;
    let repaired = load_receipt_for_run(run_id)?
        .filter(|receipt| execution_receipt_matches_run(receipt, &run))
        .ok_or_else(|| {
            routing_error(
                "execution evidence repair did not produce a receipt bound to the persisted run",
            )
        })?;
    let _ = clear_recovery_record(run_id);

    Ok(matching_receipt_state(run_id, &repaired))
}

/// Fail closed before Review mutates the active checkout.
pub(crate) fn require_review_evidence_ready(run_id: &str) -> RepoDeskResult<()> {
    let state = evidence_state_for_run(run_id)?;
    match state.status {
        ExecutionEvidenceStatus::Ready => Ok(()),
        ExecutionEvidenceStatus::NotRequired => Err(routing_error(
            "review blocked: dry-run execution has no reviewable execution evidence",
        )),
        ExecutionEvidenceStatus::Incomplete => Err(routing_error(
            "review blocked: execution evidence is incomplete; rerun execution to obtain trustworthy changeset provenance",
        )),
        ExecutionEvidenceStatus::RecoveryRequired => {
            let detail = state
                .detail
                .as_deref()
                .unwrap_or("execution evidence requires recovery");
            Err(routing_error(format!(
                "review blocked: {detail}. Repair execution evidence before Accept/Reject; do not rerun the agent merely to repair persistence"
            )))
        }
    }
}

fn finalize_execution_evidence(
    plan: &OrchestrationPlan,
    run: &OrchestrationRun,
) -> RepoDeskResult<ExecutionEvidenceState> {
    if let Some(receipt) = load_receipt_for_run(&run.run_id)?
        && execution_receipt_matches_run(&receipt, run)
    {
        let _ = clear_recovery_record(&run.run_id);
        return Ok(matching_receipt_state(&run.run_id, &receipt));
    }

    let mode = crate::workflow::load_phase_state()
        .map(|state| state.execution_mode)
        .unwrap_or_default();
    let base_commit = crate::projects::get_active_project()
        .ok()
        .and_then(|project| head_sha(&project.path));
    let receipt = build_execution_receipt(plan, run, mode, base_commit);

    match save_receipt(&receipt) {
        Ok(()) => {
            let _ = clear_recovery_record(&run.run_id);
            Ok(matching_receipt_state(&run.run_id, &receipt))
        }
        Err(error) => {
            let detail = format!("execution receipt persistence failed: {error}");
            let record = EvidenceRecoveryRecord {
                run_id: run.run_id.clone(),
                receipt,
                persistence_error: error.to_string(),
                recorded_at: Utc::now().to_rfc3339(),
            };
            let recoverable = write_recovery_record(&record).is_ok();
            Ok(ExecutionEvidenceState {
                run_id: run.run_id.clone(),
                status: ExecutionEvidenceStatus::RecoveryRequired,
                recoverable,
                detail: Some(if recoverable {
                    detail
                } else {
                    format!(
                        "{detail}; RepoDesk also could not persist the recovery payload, so a safe replay may be unavailable"
                    )
                }),
            })
        }
    }
}

fn build_execution_receipt(
    plan: &OrchestrationPlan,
    run: &OrchestrationRun,
    execution_mode: ExecutionMode,
    base_commit: Option<String>,
) -> TaskRunReceipt {
    let allow_write_of = |task_id: &str| {
        plan.steps
            .iter()
            .find(|step| step.id == task_id)
            .map(|step| step.allow_write)
            .unwrap_or(false)
    };

    let mut seen = HashSet::new();
    let mut changed = Vec::new();
    let required_steps = run
        .results
        .iter()
        .map(|result| {
            for path in &result.changed_files {
                if seen.insert(path.clone()) {
                    changed.push(path.clone());
                }
            }
            let allow_write = allow_write_of(&result.task_id);
            StepReceipt {
                task_id: result.task_id.clone(),
                status: result.status,
                allow_write,
                changed_files: result.changed_files.clone(),
                change_evidence_status: result.change_evidence_status,
                change_attribution: classify_step_attribution(
                    &run.run_id,
                    &result.task_id,
                    false,
                    result.change_evidence_status,
                    result.workspace.as_ref(),
                ),
            }
        })
        .collect();

    TaskRunReceipt {
        task_id: run.task_id.clone(),
        run_id: run.run_id.clone(),
        execution_mode,
        base_commit,
        execution: ExecutionReceipt {
            status: run.status,
            required_steps,
            changeset_digest: (!changed.is_empty()).then(|| changeset_digest(&changed)),
        },
        review: None,
        verification: None,
        finish: None,
    }
}

fn execution_receipt_matches_run(receipt: &TaskRunReceipt, run: &OrchestrationRun) -> bool {
    if receipt.task_id != run.task_id
        || receipt.run_id != run.run_id
        || receipt.execution.status != run.status
        || receipt.execution.required_steps.len() != run.results.len()
    {
        return false;
    }

    for result in &run.results {
        let Some(step) = receipt
            .execution
            .required_steps
            .iter()
            .find(|step| step.task_id == result.task_id)
        else {
            return false;
        };
        let expected_attribution = classify_step_attribution(
            &run.run_id,
            &result.task_id,
            false,
            result.change_evidence_status,
            result.workspace.as_ref(),
        );
        if step.status != result.status
            || step.changed_files != result.changed_files
            || step.change_evidence_status != result.change_evidence_status
            || step.change_attribution != expected_attribution
        {
            return false;
        }
    }

    let mut seen = HashSet::new();
    let mut changed = Vec::new();
    for result in &run.results {
        for path in &result.changed_files {
            if seen.insert(path.clone()) {
                changed.push(path.clone());
            }
        }
    }
    let expected_digest = (!changed.is_empty()).then(|| changeset_digest(&changed));
    receipt.execution.changeset_digest == expected_digest
}

fn matching_receipt_status(receipt: &TaskRunReceipt) -> ExecutionEvidenceStatus {
    if receipt
        .execution
        .required_steps
        .iter()
        .any(|step| step.allow_write && !step.change_evidence_status.is_complete())
    {
        ExecutionEvidenceStatus::Incomplete
    } else {
        ExecutionEvidenceStatus::Ready
    }
}

fn matching_receipt_state(run_id: &str, receipt: &TaskRunReceipt) -> ExecutionEvidenceState {
    let status = matching_receipt_status(receipt);
    let detail = (status == ExecutionEvidenceStatus::Incomplete).then(|| {
        "execution receipt exists, but one or more write-capable steps lack complete changeset provenance; rerun execution before Review"
            .to_string()
    });
    ExecutionEvidenceState {
        run_id: run_id.to_string(),
        status,
        recoverable: false,
        detail,
    }
}

fn recovery_state(run_id: &str, detail: &str) -> RepoDeskResult<ExecutionEvidenceState> {
    let recoverable = read_recovery_record(run_id)?.is_some();
    Ok(ExecutionEvidenceState {
        run_id: run_id.to_string(),
        status: ExecutionEvidenceStatus::RecoveryRequired,
        recoverable,
        detail: Some(detail.to_string()),
    })
}

fn recovery_dir() -> RepoDeskResult<PathBuf> {
    Ok(show_active_task()?
        .config
        .run_dir
        .join("orchestrate")
        .join("evidence-recovery"))
}

fn recovery_path(run_id: &str) -> RepoDeskResult<PathBuf> {
    validate_run_id(run_id)?;
    Ok(recovery_dir()?.join(format!("{run_id}.json")))
}

fn write_recovery_record(record: &EvidenceRecoveryRecord) -> RepoDeskResult<()> {
    let path = recovery_path(&record.run_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| routing_error("execution evidence recovery path has no parent"))?;
    fs::create_dir_all(parent)?;

    let nonce = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Utc::now().timestamp_micros().saturating_mul(1_000));
    let temp = parent.join(format!(
        ".{}.{}.{nonce}.tmp",
        record.run_id,
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(record)?;
    if bytes.len() as u64 > MAX_RECOVERY_BYTES {
        return Err(routing_error(format!(
            "execution evidence recovery payload exceeds {MAX_RECOVERY_BYTES} bytes"
        )));
    }

    let write_result = (|| -> RepoDeskResult<()> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, &path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

fn read_recovery_record(run_id: &str) -> RepoDeskResult<Option<EvidenceRecoveryRecord>> {
    let path = recovery_path(run_id)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(routing_error(
            "execution evidence recovery payload is not a regular file",
        ));
    }
    if metadata.len() > MAX_RECOVERY_BYTES {
        return Err(routing_error(format!(
            "execution evidence recovery payload exceeds {MAX_RECOVERY_BYTES} bytes"
        )));
    }

    let bytes = fs::read(&path)?;
    let record: EvidenceRecoveryRecord = serde_json::from_slice(&bytes)?;
    if record.run_id != run_id || record.receipt.run_id != run_id {
        return Err(routing_error(
            "execution evidence recovery payload is bound to a different run",
        ));
    }
    Ok(Some(record))
}

fn clear_recovery_record(run_id: &str) -> RepoDeskResult<()> {
    let path = recovery_path(run_id)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_run_id(run_id: &str) -> RepoDeskResult<()> {
    let safe = run_id.starts_with("run-")
        && run_id.len() <= 160
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if safe {
        Ok(())
    } else {
        Err(routing_error("invalid orchestration run id"))
    }
}

fn log_recovery_required(run_id: &str, detail: Option<&str>) {
    let _ = log_event(LogEventInput {
        module_name: "orchestrator".to_string(),
        level: "warn".to_string(),
        message: format!("orchestration {run_id} requires execution-evidence recovery"),
        metadata: vec![
            ("run_id".to_string(), run_id.to_string()),
            (
                "detail".to_string(),
                detail
                    .unwrap_or("execution receipt is unavailable")
                    .to_string(),
            ),
        ],
    });
}

fn routing_error(detail: impl Into<String>) -> RepoDeskError {
    RepoDeskError::RoutingFailed {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{RunStatus, SubAgentResult, SubAgentStatus, SubAgentTask};
    use super::*;
    use crate::api_clients::ThinkingLevel;
    use crate::change_attribution::{ChangeAttributionEvidence, ChangeAttributionStrength};
    use crate::routing::types::{ExecutorKind, TaskKind};
    use crate::worktree::RunWorktree;

    fn task(id: &str, allow_write: bool) -> SubAgentTask {
        SubAgentTask {
            id: id.into(),
            title: id.into(),
            kind: TaskKind::Patch,
            agent: "codex_cli".into(),
            provider: "codex".into(),
            executor_kind: ExecutorKind::CodingAgent,
            executor_id: "codex_cli".into(),
            provider_id: None,
            model: None,
            thinking: ThinkingLevel::None,
            instruction: String::new(),
            depends_on: vec![],
            budget_tokens: 100,
            allow_write,
            verify_command: None,
        }
    }

    fn exact_attribution() -> ChangeAttributionEvidence {
        ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::ExactIsolated,
            workspace_id: Some("workspace-1".into()),
            baseline_commit: Some("base".into()),
            reason: Some(
                "managed isolated worktree matches run and step identity with complete changeset evidence"
                    .into(),
            ),
        }
    }

    fn run() -> OrchestrationRun {
        OrchestrationRun {
            run_id: "run-test-1".into(),
            project: "project".into(),
            task_id: "task".into(),
            goal: "goal".into(),
            status: RunStatus::Completed,
            dry_run: false,
            started_at: "start".into(),
            finished_at: "finish".into(),
            results: vec![SubAgentResult {
                task_id: "impl".into(),
                agent: "codex_cli".into(),
                provider: "codex".into(),
                model: String::new(),
                status: SubAgentStatus::Ok,
                output: String::new(),
                input_tokens: 1,
                output_tokens: 1,
                cost_units: 0.0,
                captured_proposals: 0,
                changed_files: vec!["src/lib.rs".into()],
                change_evidence_status: ChangeEvidenceStatus::Complete,
                execution_issues: vec![],
                diff_path: None,
                workspace: Some(RunWorktree {
                    workspace_id: "workspace-1".into(),
                    run_id: "run-test-1".into(),
                    step_id: "impl".into(),
                    path: "/tmp/private-worktree".into(),
                    base_commit: "base".into(),
                    created_at: "now".into(),
                    metadata_path: None,
                }),
                notes: vec![],
            }],
            total_input_tokens: 1,
            total_output_tokens: 1,
            total_cost_units: 0.0,
        }
    }

    #[test]
    fn recovery_receipt_preserves_execution_outcome_and_write_requirement() {
        let run = run();
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", true)],
        };
        let receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

        assert_eq!(receipt.execution.status, RunStatus::Completed);
        assert!(receipt.execution.required_steps[0].allow_write);
        assert_eq!(
            receipt.execution.required_steps[0].change_evidence_status,
            ChangeEvidenceStatus::Complete
        );
        assert_eq!(
            receipt.execution.required_steps[0].change_attribution,
            exact_attribution()
        );
        assert_eq!(
            matching_receipt_status(&receipt),
            ExecutionEvidenceStatus::Ready
        );
        assert_eq!(
            receipt.execution.changeset_digest,
            Some(changeset_digest(&["src/lib.rs".into()]))
        );
        assert!(execution_receipt_matches_run(&receipt, &run));
    }

    #[test]
    fn matching_receipt_with_unavailable_write_evidence_is_incomplete() {
        let mut run = run();
        run.results[0].change_evidence_status = ChangeEvidenceStatus::Unavailable;
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", true)],
        };
        let receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

        assert_eq!(
            receipt.execution.required_steps[0].change_evidence_status,
            ChangeEvidenceStatus::Unavailable
        );
        assert_eq!(
            receipt.execution.required_steps[0].change_attribution.strength,
            ChangeAttributionStrength::Unattributed
        );
        assert_eq!(
            matching_receipt_status(&receipt),
            ExecutionEvidenceStatus::Incomplete
        );
    }

    #[test]
    fn legacy_unknown_non_write_step_does_not_require_changeset_proof() {
        let mut run = run();
        run.results[0].change_evidence_status = ChangeEvidenceStatus::LegacyUnknown;
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", false)],
        };
        let receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

        assert_eq!(
            matching_receipt_status(&receipt),
            ExecutionEvidenceStatus::Ready
        );
    }

    #[test]
    fn receipt_mismatch_is_not_ready_evidence() {
        let run = run();
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", true)],
        };
        let mut receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));
        receipt.execution.required_steps[0].change_attribution = ChangeAttributionEvidence::default();
        assert!(!execution_receipt_matches_run(&receipt, &run));
    }

    #[test]
    fn run_id_validation_rejects_path_escape() {
        assert!(validate_run_id("run-safe_1").is_ok());
        assert!(validate_run_id("../run-escape").is_err());
        assert!(validate_run_id("run-a/b").is_err());
    }
}
