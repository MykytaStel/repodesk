//! Evidence-first projection for one persisted orchestration run.
//!
//! The run file supplies execution facts; canonical TaskRunReceipt data wins for
//! review/verification/commit when available. The append-only engineering ledger
//! is a historical fallback, never a reason to fabricate proof.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engineering::acceptance_evidence::{
    AcceptanceEvidenceReport, active_verification_is_fresh, derive_acceptance_evidence,
    read_acceptance_evidence,
};
use crate::engineering::domain::EvidenceRef;
use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, read_events};
use crate::engineering::work_item_contract::read_work_item_contract;
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::orchestrator::{OrchestrationRun, RunStatus, SubAgentStatus, load_run};
use crate::tasks::show_active_task;
use crate::workflow::{CheckReceipt, ReviewDecision, TaskRunReceipt, load_receipt_for_run};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunWorkerEvidence {
    pub step_id: String,
    pub agent: String,
    pub provider: String,
    pub model: String,
    pub status: SubAgentStatus,
    pub changed_files: Vec<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_units: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContextEvidence {
    pub estimated_tokens: Option<usize>,
    pub evidence: Vec<EvidenceRef>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReviewEvidence {
    pub state: String,
    pub reviewed_paths: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunVerificationEvidence {
    pub state: String,
    pub verification_id: Option<String>,
    pub commands: Vec<CheckReceipt>,
    pub evidence: Vec<EvidenceRef>,
    pub verified_at: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCommitEvidence {
    pub committed: bool,
    pub commit_sha: Option<String>,
    pub committed_paths: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvidenceSnapshot {
    pub run_id: String,
    pub project: String,
    pub work_item_id: String,
    pub goal: String,
    pub status: RunStatus,
    pub dry_run: bool,
    pub started_at: String,
    pub finished_at: String,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_cost_units: f64,
    pub workers: Vec<RunWorkerEvidence>,
    pub changed_files: Vec<String>,
    pub context: RunContextEvidence,
    pub review: RunReviewEvidence,
    pub verification: RunVerificationEvidence,
    pub commit: RunCommitEvidence,
    pub acceptance: AcceptanceEvidenceReport,
}

pub fn load_active_run_evidence(run_id: &str) -> RepoDeskResult<RunEvidenceSnapshot> {
    let task = show_active_task()?;
    let events = read_events(&task.config.run_dir)?;
    load_active_run_evidence_from_events(run_id, &events)
}

/// Variant for aggregate desktop snapshots that already replayed the task ledger.
/// This keeps selected-run polling at one engineering-event read per refresh.
pub fn load_active_run_evidence_from_events(
    run_id: &str,
    events: &[EngineeringEvent],
) -> RepoDeskResult<RunEvidenceSnapshot> {
    validate_run_id(run_id)?;
    let task = show_active_task()?;
    let run = load_run(run_id)?.ok_or_else(|| {
        RepoDeskError::Api(format!(
            "No persisted run '{run_id}' exists for the active Work Item"
        ))
    })?;
    if run.task_id != task.config.id || run.project != task.config.project_name {
        return Err(RepoDeskError::Api(
            "Persisted run does not belong to the active Work Item".into(),
        ));
    }

    let receipt = load_receipt_for_run(run_id)?;
    let verification_fresh = match receipt.as_ref() {
        Some(receipt) => active_verification_is_fresh(receipt)?,
        None => false,
    };
    let contract = read_work_item_contract(&task.config.run_dir)?;
    // Acceptance bindings are Work Item-local, but a historical run must never
    // inherit a newer run's link merely because the criterion id is identical.
    let store = read_acceptance_evidence(&task.config.run_dir)?.map(|mut store| {
        store.bindings.retain(|binding| binding.run_id == run_id);
        store
    });
    let acceptance = derive_acceptance_evidence(
        &task,
        contract.as_ref(),
        receipt.as_ref(),
        store.as_ref(),
        verification_fresh,
    );

    Ok(derive_run_evidence(
        &run,
        receipt.as_ref(),
        events,
        acceptance,
        verification_fresh,
    ))
}

pub fn derive_run_evidence(
    run: &OrchestrationRun,
    receipt: Option<&TaskRunReceipt>,
    events: &[EngineeringEvent],
    acceptance: AcceptanceEvidenceReport,
    verification_fresh: bool,
) -> RunEvidenceSnapshot {
    let workers = run
        .results
        .iter()
        .map(|result| RunWorkerEvidence {
            step_id: result.task_id.clone(),
            agent: result.agent.clone(),
            provider: result.provider.clone(),
            model: result.model.clone(),
            status: result.status,
            changed_files: result.changed_files.clone(),
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
            cost_units: result.cost_units,
        })
        .collect();
    let changed_files = run
        .results
        .iter()
        .flat_map(|result| result.changed_files.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    RunEvidenceSnapshot {
        run_id: run.run_id.clone(),
        project: run.project.clone(),
        work_item_id: run.task_id.clone(),
        goal: run.goal.clone(),
        status: run.status,
        dry_run: run.dry_run,
        started_at: run.started_at.clone(),
        finished_at: run.finished_at.clone(),
        total_input_tokens: run.total_input_tokens,
        total_output_tokens: run.total_output_tokens,
        total_cost_units: run.total_cost_units,
        workers,
        changed_files,
        context: derive_context(events, &run.run_id),
        review: derive_review(receipt, events, &run.run_id),
        verification: derive_verification(receipt, events, &run.run_id, verification_fresh),
        commit: derive_commit(receipt, events, &run.run_id),
        acceptance,
    }
}

fn derive_context(events: &[EngineeringEvent], run_id: &str) -> RunContextEvidence {
    let execution_start = events
        .iter()
        .find(|event| {
            event.kind == EngineeringEventKind::ExecutionStarted
                && event
                    .execution_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == run_id)
        })
        .map(|event| event.occurred_at);

    let context = events.iter().rev().find(|event| {
        event.kind == EngineeringEventKind::ContextBuilt
            && execution_start.is_none_or(|started| event.occurred_at <= started)
    });

    match context {
        Some(event) => RunContextEvidence {
            estimated_tokens: event
                .attributes
                .get("estimated_tokens")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            evidence: event.evidence.clone(),
            source: "engineering_event".into(),
        },
        None => RunContextEvidence {
            estimated_tokens: None,
            evidence: Vec::new(),
            source: "unavailable".into(),
        },
    }
}

fn derive_review(
    receipt: Option<&TaskRunReceipt>,
    events: &[EngineeringEvent],
    run_id: &str,
) -> RunReviewEvidence {
    if let Some(review) = receipt.and_then(|receipt| receipt.review.as_ref()) {
        return RunReviewEvidence {
            state: match review.decision {
                ReviewDecision::Accepted => "accepted",
                ReviewDecision::Rejected => "rejected",
            }
            .into(),
            reviewed_paths: review.reviewed_paths.clone(),
            source: "task_run_receipt".into(),
        };
    }

    let event = matching_events(events, run_id)
        .find(|event| event.kind == EngineeringEventKind::ChangeSetReviewed);
    match event {
        Some(event) => RunReviewEvidence {
            state: event
                .attributes
                .get("decision")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            reviewed_paths: string_array(event, "files"),
            source: "engineering_event".into(),
        },
        None => RunReviewEvidence {
            state: "not_reviewed".into(),
            reviewed_paths: Vec::new(),
            source: "unavailable".into(),
        },
    }
}

fn derive_verification(
    receipt: Option<&TaskRunReceipt>,
    events: &[EngineeringEvent],
    run_id: &str,
    verification_fresh: bool,
) -> RunVerificationEvidence {
    if let Some(verification) = receipt.and_then(|receipt| receipt.verification.as_ref()) {
        let state = if !verification_fresh {
            "stale"
        } else if verification.success {
            "passed"
        } else {
            "failed"
        };
        return RunVerificationEvidence {
            state: state.into(),
            verification_id: None,
            commands: verification.commands.clone(),
            evidence: Vec::new(),
            verified_at: Some(verification.verified_at.clone()),
            source: "task_run_receipt".into(),
        };
    }

    let event = matching_events(events, run_id).find(|event| {
        event.kind == EngineeringEventKind::VerificationFinished
            || event.kind == EngineeringEventKind::VerificationStarted
    });
    match event {
        Some(event) if event.kind == EngineeringEventKind::VerificationFinished => {
            let success = event
                .attributes
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            RunVerificationEvidence {
                state: if success { "passed" } else { "failed" }.into(),
                verification_id: event.verification_id.as_ref().map(ToString::to_string),
                commands: Vec::new(),
                evidence: event.evidence.clone(),
                verified_at: Some(event.occurred_at.to_rfc3339()),
                source: "engineering_event".into(),
            }
        }
        Some(event) => RunVerificationEvidence {
            state: "running".into(),
            verification_id: event.verification_id.as_ref().map(ToString::to_string),
            commands: Vec::new(),
            evidence: event.evidence.clone(),
            verified_at: None,
            source: "engineering_event".into(),
        },
        None => RunVerificationEvidence {
            state: "not_run".into(),
            verification_id: None,
            commands: Vec::new(),
            evidence: Vec::new(),
            verified_at: None,
            source: "unavailable".into(),
        },
    }
}

fn derive_commit(
    receipt: Option<&TaskRunReceipt>,
    events: &[EngineeringEvent],
    run_id: &str,
) -> RunCommitEvidence {
    if let Some(finish) = receipt.and_then(|receipt| receipt.finish.as_ref()) {
        return RunCommitEvidence {
            committed: true,
            commit_sha: Some(finish.commit_sha.clone()),
            committed_paths: finish.committed_paths.clone(),
            source: "task_run_receipt".into(),
        };
    }

    let event = matching_events(events, run_id)
        .find(|event| event.kind == EngineeringEventKind::CommitCreated);
    match event {
        Some(event) => RunCommitEvidence {
            committed: true,
            commit_sha: event.evidence.first().map(|evidence| evidence.locator.clone()),
            committed_paths: string_array(event, "files"),
            source: "engineering_event".into(),
        },
        None => RunCommitEvidence {
            committed: false,
            commit_sha: None,
            committed_paths: Vec::new(),
            source: "unavailable".into(),
        },
    }
}

fn matching_events<'a>(
    events: &'a [EngineeringEvent],
    run_id: &'a str,
) -> impl Iterator<Item = &'a EngineeringEvent> {
    let changeset_id = format!("{run_id}-changeset");
    events.iter().rev().filter(move |event| {
        event
            .execution_id
            .as_ref()
            .is_some_and(|id| id.as_str() == run_id)
            || event
                .changeset_id
                .as_ref()
                .is_some_and(|id| id.as_str() == changeset_id.as_str())
    })
}

fn string_array(event: &EngineeringEvent, key: &str) -> Vec<String> {
    event
        .attributes
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn validate_run_id(run_id: &str) -> RepoDeskResult<()> {
    let safe = run_id.starts_with("run-")
        && run_id.len() <= 120
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if safe {
        Ok(())
    } else {
        Err(RepoDeskError::Api("Invalid run id".into()))
    }
}
