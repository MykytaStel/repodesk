//! Deterministic governance read model for the latest Work Item ChangeSet.
//!
//! RepoDesk derives the historical projection from the canonical engineering
//! ledger plus the versioned Work Item Contract. Live callers must additionally
//! reconcile a passed verification event with the current canonical run receipt
//! before the commit gate may claim the ChangeSet is ready.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engineering::domain::{ChangeSetId, EvidenceRef, ExecutionId, WorkerRef};
use crate::engineering::events::{
    EngineeringEvent, EngineeringEventKind, append_event, read_events,
};
use crate::engineering::work_item_contract::{
    ScopeComplianceStatus, WorkItemContractSnapshot, load_work_item_contract_snapshot,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::tasks::{TaskInfo, show_active_task};

const MAX_OVERRIDE_REASON_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeFileScopeState {
    Allowed,
    OutOfScope,
    Protected,
    Ungoverned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeFileGovernance {
    pub path: String,
    pub scope_state: ChangeFileScopeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChangeOrigin {
    pub execution_id: Option<String>,
    pub execution_mode: Option<String>,
    pub workers: Vec<WorkerRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeReviewState {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeVerificationState {
    NotRun,
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeVerificationEvidence {
    pub state: ChangeVerificationState,
    pub verification_id: Option<String>,
    pub command_count: usize,
    pub evidence: Vec<EvidenceRef>,
    pub error: Option<String>,
    /// `None` means this is only the historical ledger projection. Normal live
    /// Changes snapshots reconcile it against the canonical TaskRunReceipt and
    /// current Git tree before exposing commit readiness.
    #[serde(default)]
    pub fresh: Option<bool>,
    #[serde(default)]
    pub stale_reason: Option<String>,
}

impl Default for ChangeVerificationEvidence {
    fn default() -> Self {
        Self {
            state: ChangeVerificationState::NotRun,
            verification_id: None,
            command_count: 0,
            evidence: Vec::new(),
            error: None,
            fresh: None,
            stale_reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeOverrideEvidence {
    pub event_id: String,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitGateState {
    NoChangeSet,
    ScopeViolation,
    NeedsReview,
    Rejected,
    VerificationRequired,
    VerificationRunning,
    VerificationFailed,
    VerificationStale,
    Ready,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitGate {
    pub state: CommitGateState,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeGovernanceSnapshot {
    pub work_item_id: String,
    pub changeset_id: Option<String>,
    pub origin: ChangeOrigin,
    pub files: Vec<ChangeFileGovernance>,
    pub scope_status: ScopeComplianceStatus,
    pub review_state: ChangeReviewState,
    pub verification: ChangeVerificationEvidence,
    pub scope_override: Option<ScopeOverrideEvidence>,
    pub committed: bool,
    pub commit_sha: Option<String>,
    pub gate: CommitGate,
}

pub fn load_active_change_governance() -> RepoDeskResult<ChangeGovernanceSnapshot> {
    let task = show_active_task()?;
    load_change_governance(&task)
}

pub fn load_change_governance(task: &TaskInfo) -> RepoDeskResult<ChangeGovernanceSnapshot> {
    let events = read_events(&task.config.run_dir)?;
    let contract = load_work_item_contract_snapshot(task)?;
    Ok(derive_change_governance(
        &task.config.id,
        &events,
        &contract,
    ))
}

/// Reconcile the historical event projection with the authoritative live
/// VerificationReceipt. A successful `VerificationFinished` event is immutable
/// history; it is not proof that the *current* HEAD/index still match what was
/// verified. Only this reconciliation may turn the event projection into live
/// commit readiness.
pub fn reconcile_verification_freshness(
    snapshot: &mut ChangeGovernanceSnapshot,
    fresh: bool,
    stale_reason: Option<String>,
) {
    if snapshot.verification.state != ChangeVerificationState::Passed {
        snapshot.verification.fresh = None;
        snapshot.verification.stale_reason = None;
        return;
    }

    snapshot.verification.fresh = Some(fresh);
    snapshot.verification.stale_reason = if fresh {
        None
    } else {
        Some(stale_reason.unwrap_or_else(|| {
            "The VerificationReceipt no longer matches the current reviewed ChangeSet tree."
                .to_string()
        }))
    };

    // Preserve earlier/higher-priority blockers such as scope or review. The
    // freshness reconciliation only replaces a gate that would otherwise claim
    // a passed historical verification makes this ChangeSet commit-ready.
    if !fresh && snapshot.gate.state == CommitGateState::Ready {
        let reason = snapshot
            .verification
            .stale_reason
            .clone()
            .unwrap_or_else(|| "Verification is stale for the current ChangeSet.".to_string());
        snapshot.gate = CommitGate {
            state: CommitGateState::VerificationStale,
            ready: false,
            blockers: vec![reason],
            warnings: snapshot.gate.warnings.clone(),
        };
    }
}

pub fn record_active_scope_override(reason: &str) -> RepoDeskResult<ChangeGovernanceSnapshot> {
    let task = show_active_task()?;
    let reason = validate_override_reason(reason)?;
    let current = load_change_governance(&task)?;

    if current.gate.state != CommitGateState::ScopeViolation {
        return Err(RepoDeskError::Api(
            "A scope override can only be recorded for the current scope violation".into(),
        ));
    }

    let changeset_id = current
        .changeset_id
        .as_deref()
        .ok_or_else(|| RepoDeskError::Api("No current ChangeSet to override".into()))?;
    let changeset_id = ChangeSetId::try_new(changeset_id.to_string())
        .map_err(|error| RepoDeskError::Api(error.to_string()))?;
    let work_item_id = crate::engineering::domain::WorkItemId::try_new(task.config.id.clone())
        .map_err(|error| RepoDeskError::Api(error.to_string()))?;

    let mut event = EngineeringEvent::new(
        task.config.project_name.clone(),
        work_item_id,
        EngineeringEventKind::HumanOverride,
    )
    .with_changeset(changeset_id)
    .with_attribute("override_kind", Value::String("scope_violation".into()))
    .with_attribute("reason", Value::String(reason))
    .with_attribute(
        "out_of_scope_files",
        json!(
            current
                .files
                .iter()
                .filter(|file| file.scope_state == ChangeFileScopeState::OutOfScope)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>()
        ),
    )
    .with_attribute(
        "protected_files",
        json!(
            current
                .files
                .iter()
                .filter(|file| file.scope_state == ChangeFileScopeState::Protected)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>()
        ),
    );

    if let Some(execution_id) = current.origin.execution_id {
        let execution_id = ExecutionId::try_new(execution_id)
            .map_err(|error| RepoDeskError::Api(error.to_string()))?;
        event = event.with_execution(execution_id);
    }

    append_event(&task.config.run_dir, &event)?;
    load_change_governance(&task)
}

pub fn derive_change_governance(
    work_item_id: &str,
    events: &[EngineeringEvent],
    contract: &WorkItemContractSnapshot,
) -> ChangeGovernanceSnapshot {
    let Some((changeset_index, changeset_event)) = events
        .iter()
        .enumerate()
        .rev()
        .find(|(_, event)| event.kind == EngineeringEventKind::ChangeSetCreated)
    else {
        return ChangeGovernanceSnapshot {
            work_item_id: work_item_id.to_string(),
            changeset_id: None,
            origin: ChangeOrigin::default(),
            files: Vec::new(),
            scope_status: ScopeComplianceStatus::NotEvaluated,
            review_state: ChangeReviewState::Proposed,
            verification: ChangeVerificationEvidence::default(),
            scope_override: None,
            committed: false,
            commit_sha: None,
            gate: CommitGate {
                state: CommitGateState::NoChangeSet,
                ready: false,
                blockers: vec!["No recorded ChangeSet for this Work Item.".into()],
                warnings: contract_warning(contract),
            },
        };
    };

    let changeset_id = changeset_event
        .changeset_id
        .as_ref()
        .map(ToString::to_string);
    let execution_id = changeset_event
        .execution_id
        .as_ref()
        .map(ToString::to_string);
    let changed_files = string_array_attribute(changeset_event, "files");
    let origin = derive_origin(events, changeset_event.execution_id.as_ref());
    let files = classify_files(&changed_files, contract);
    let review_state = derive_review_state(events, changeset_event.changeset_id.as_ref());
    let verification = derive_verification(events, changeset_event.changeset_id.as_ref());
    let (committed, commit_sha) = derive_commit(events, changeset_event.changeset_id.as_ref());
    let scope_override = derive_valid_scope_override(
        events,
        changeset_index,
        changeset_event.changeset_id.as_ref(),
    );
    let gate = derive_gate(
        contract,
        review_state,
        &verification,
        scope_override.as_ref(),
        committed,
    );

    ChangeGovernanceSnapshot {
        work_item_id: work_item_id.to_string(),
        changeset_id,
        origin: ChangeOrigin {
            execution_id: execution_id.or(origin.execution_id),
            ..origin
        },
        files,
        scope_status: contract.compliance.status,
        review_state,
        verification,
        scope_override,
        committed,
        commit_sha,
        gate,
    }
}

fn classify_files(
    changed_files: &[String],
    contract: &WorkItemContractSnapshot,
) -> Vec<ChangeFileGovernance> {
    let allowed: BTreeSet<&str> = contract
        .compliance
        .allowed_changed_files
        .iter()
        .map(String::as_str)
        .collect();
    let outside: BTreeSet<&str> = contract
        .compliance
        .out_of_scope_files
        .iter()
        .map(String::as_str)
        .collect();
    let protected: BTreeSet<&str> = contract
        .compliance
        .protected_changed_files
        .iter()
        .map(String::as_str)
        .collect();

    changed_files
        .iter()
        .map(|path| {
            let scope_state = if protected.contains(path.as_str()) {
                ChangeFileScopeState::Protected
            } else if outside.contains(path.as_str()) {
                ChangeFileScopeState::OutOfScope
            } else if allowed.contains(path.as_str()) {
                ChangeFileScopeState::Allowed
            } else {
                ChangeFileScopeState::Ungoverned
            };
            ChangeFileGovernance {
                path: path.clone(),
                scope_state,
            }
        })
        .collect()
}

fn derive_origin(events: &[EngineeringEvent], execution_id: Option<&ExecutionId>) -> ChangeOrigin {
    let Some(execution_id) = execution_id else {
        return ChangeOrigin::default();
    };

    for event in events.iter().rev() {
        if event.kind != EngineeringEventKind::ExecutionFinished
            || event.execution_id.as_ref() != Some(execution_id)
        {
            continue;
        }

        let workers = event
            .attributes
            .get("workers")
            .cloned()
            .and_then(|value| serde_json::from_value::<Vec<WorkerRef>>(value).ok())
            .unwrap_or_default();
        let execution_mode = event
            .attributes
            .get("execution_mode")
            .and_then(Value::as_str)
            .map(str::to_string);
        return ChangeOrigin {
            execution_id: Some(execution_id.to_string()),
            execution_mode,
            workers,
        };
    }

    ChangeOrigin {
        execution_id: Some(execution_id.to_string()),
        ..ChangeOrigin::default()
    }
}

fn derive_review_state(
    events: &[EngineeringEvent],
    changeset_id: Option<&ChangeSetId>,
) -> ChangeReviewState {
    let Some(changeset_id) = changeset_id else {
        return ChangeReviewState::Proposed;
    };

    for event in events.iter().rev() {
        if event.kind != EngineeringEventKind::ChangeSetReviewed
            || event.changeset_id.as_ref() != Some(changeset_id)
        {
            continue;
        }
        return match event.attributes.get("decision").and_then(Value::as_str) {
            Some("accept") | Some("accepted") => ChangeReviewState::Accepted,
            Some("reject") | Some("rejected") => ChangeReviewState::Rejected,
            _ => ChangeReviewState::Proposed,
        };
    }

    ChangeReviewState::Proposed
}

fn derive_verification(
    events: &[EngineeringEvent],
    changeset_id: Option<&ChangeSetId>,
) -> ChangeVerificationEvidence {
    let Some(changeset_id) = changeset_id else {
        return ChangeVerificationEvidence::default();
    };

    for event in events.iter().rev() {
        if event.changeset_id.as_ref() != Some(changeset_id) {
            continue;
        }
        match event.kind {
            EngineeringEventKind::VerificationFinished => {
                let success = event
                    .attributes
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                return ChangeVerificationEvidence {
                    state: if success {
                        ChangeVerificationState::Passed
                    } else {
                        ChangeVerificationState::Failed
                    },
                    verification_id: event.verification_id.as_ref().map(ToString::to_string),
                    command_count: event
                        .attributes
                        .get("command_count")
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as usize,
                    evidence: event.evidence.clone(),
                    error: event
                        .attributes
                        .get("error")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    fresh: None,
                    stale_reason: None,
                };
            }
            EngineeringEventKind::VerificationStarted => {
                return ChangeVerificationEvidence {
                    state: ChangeVerificationState::Running,
                    verification_id: event.verification_id.as_ref().map(ToString::to_string),
                    ..ChangeVerificationEvidence::default()
                };
            }
            _ => {}
        }
    }

    ChangeVerificationEvidence::default()
}

fn derive_commit(
    events: &[EngineeringEvent],
    changeset_id: Option<&ChangeSetId>,
) -> (bool, Option<String>) {
    let Some(changeset_id) = changeset_id else {
        return (false, None);
    };

    for event in events.iter().rev() {
        if event.kind != EngineeringEventKind::CommitCreated
            || event.changeset_id.as_ref() != Some(changeset_id)
        {
            continue;
        }
        let sha = event
            .evidence
            .iter()
            .find(|evidence| evidence.kind == crate::engineering::domain::EvidenceKind::Commit)
            .map(|evidence| evidence.locator.clone());
        return (true, sha);
    }

    (false, None)
}

fn derive_valid_scope_override(
    events: &[EngineeringEvent],
    changeset_index: usize,
    changeset_id: Option<&ChangeSetId>,
) -> Option<ScopeOverrideEvidence> {
    let changeset_id = changeset_id?;
    let latest_scope_change = events
        .iter()
        .enumerate()
        .rev()
        .find(|(_, event)| event.kind == EngineeringEventKind::ScopeChanged)
        .map(|(index, _)| index);

    for (index, event) in events.iter().enumerate().rev() {
        if index <= changeset_index {
            break;
        }
        if event.kind != EngineeringEventKind::HumanOverride
            || event.changeset_id.as_ref() != Some(changeset_id)
            || event
                .attributes
                .get("override_kind")
                .and_then(Value::as_str)
                != Some("scope_violation")
        {
            continue;
        }
        if latest_scope_change.is_some_and(|scope_index| scope_index > index) {
            return None;
        }
        let reason = event
            .attributes
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Human override")
            .to_string();
        return Some(ScopeOverrideEvidence {
            event_id: event.id.to_string(),
            reason,
            occurred_at: event.occurred_at,
        });
    }

    None
}

fn derive_gate(
    contract: &WorkItemContractSnapshot,
    review_state: ChangeReviewState,
    verification: &ChangeVerificationEvidence,
    scope_override: Option<&ScopeOverrideEvidence>,
    committed: bool,
) -> CommitGate {
    let mut warnings = contract_warning(contract);

    if committed {
        return CommitGate {
            state: CommitGateState::Committed,
            ready: false,
            blockers: Vec::new(),
            warnings,
        };
    }

    if review_state == ChangeReviewState::Rejected {
        return CommitGate {
            state: CommitGateState::Rejected,
            ready: false,
            blockers: vec!["The current ChangeSet was rejected.".into()],
            warnings,
        };
    }

    if contract.compliance.status == ScopeComplianceStatus::Violation {
        if scope_override.is_none() {
            return CommitGate {
                state: CommitGateState::ScopeViolation,
                ready: false,
                blockers: vec![
                    "The ChangeSet contains out-of-scope or protected files. Record an explicit human override or change the ChangeSet.".into(),
                ],
                warnings,
            };
        }
        warnings
            .push("Scope violation explicitly overridden by a human for this ChangeSet.".into());
    }

    if review_state != ChangeReviewState::Accepted {
        return CommitGate {
            state: CommitGateState::NeedsReview,
            ready: false,
            blockers: vec!["The ChangeSet has not been accepted by a human reviewer.".into()],
            warnings,
        };
    }

    match verification.state {
        ChangeVerificationState::NotRun => CommitGate {
            state: CommitGateState::VerificationRequired,
            ready: false,
            blockers: vec!["No finished verification receipt exists for this ChangeSet.".into()],
            warnings,
        },
        ChangeVerificationState::Running => CommitGate {
            state: CommitGateState::VerificationRunning,
            ready: false,
            blockers: vec!["Verification is still running for this ChangeSet.".into()],
            warnings,
        },
        ChangeVerificationState::Failed => CommitGate {
            state: CommitGateState::VerificationFailed,
            ready: false,
            blockers: vec!["The latest verification receipt failed.".into()],
            warnings,
        },
        ChangeVerificationState::Passed => CommitGate {
            state: CommitGateState::Ready,
            ready: true,
            blockers: Vec::new(),
            warnings,
        },
    }
}

fn contract_warning(contract: &WorkItemContractSnapshot) -> Vec<String> {
    if contract.configured {
        Vec::new()
    } else {
        vec!["No typed Engineering Contract is configured; scope governance is unavailable.".into()]
    }
}

fn string_array_attribute(event: &EngineeringEvent, key: &str) -> Vec<String> {
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

fn validate_override_reason(reason: &str) -> RepoDeskResult<String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(RepoDeskError::Api(
            "A human override requires a concrete reason".into(),
        ));
    }
    if reason.chars().count() > MAX_OVERRIDE_REASON_CHARS
        || reason.chars().any(|character| character.is_control())
    {
        return Err(RepoDeskError::Api(format!(
            "Override reason must be a single line of at most {MAX_OVERRIDE_REASON_CHARS} characters"
        )));
    }
    Ok(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::domain::{WorkItemId, WorkerKind};
    use crate::engineering::work_item_contract::{
        ScopeComplianceReport, WorkItemContract, WorkItemContractReadiness,
    };

    fn event(kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new("repodesk", WorkItemId::try_new("task-1").unwrap(), kind)
    }

    fn contract(status: ScopeComplianceStatus) -> WorkItemContractSnapshot {
        WorkItemContractSnapshot {
            configured: true,
            contract: WorkItemContract {
                version: 1,
                project: "repodesk".into(),
                work_item_id: "task-1".into(),
                goal: "Bound the change".into(),
                allowed_paths: vec!["src".into()],
                protected_paths: vec!["src/security".into()],
                acceptance_criteria: vec!["tests pass".into()],
                updated_at: Utc::now(),
            },
            readiness: WorkItemContractReadiness {
                goal_defined: true,
                scope_defined: true,
                acceptance_defined: true,
                protected_paths_defined: true,
            },
            compliance: ScopeComplianceReport {
                status,
                changed_files: vec!["src/lib.rs".into(), "README.md".into()],
                allowed_changed_files: vec!["src/lib.rs".into()],
                out_of_scope_files: if status == ScopeComplianceStatus::Violation {
                    vec!["README.md".into()]
                } else {
                    Vec::new()
                },
                protected_changed_files: Vec::new(),
            },
        }
    }

    fn changeset_events() -> Vec<EngineeringEvent> {
        let execution = ExecutionId::try_new("run-1").unwrap();
        let changeset = ChangeSetId::try_new("run-1-changeset").unwrap();
        let worker = WorkerRef {
            kind: WorkerKind::CodingAgent,
            id: "codex_cli".into(),
            provider: None,
            model: None,
        };
        vec![
            event(EngineeringEventKind::ExecutionFinished)
                .with_execution(execution.clone())
                .with_attribute("execution_mode", Value::String("managed".into()))
                .with_attribute("workers", json!([worker])),
            event(EngineeringEventKind::ChangeSetCreated)
                .with_execution(execution)
                .with_changeset(changeset)
                .with_attribute("files", json!(["src/lib.rs", "README.md"])),
        ]
    }

    fn accepted_verified_events() -> Vec<EngineeringEvent> {
        let mut events = changeset_events();
        let changeset = ChangeSetId::try_new("run-1-changeset").unwrap();
        let verification = crate::engineering::domain::VerificationId::try_new("verify-1").unwrap();
        events.push(
            event(EngineeringEventKind::ChangeSetReviewed)
                .with_changeset(changeset.clone())
                .with_attribute("decision", Value::String("accept".into())),
        );
        events.push(
            event(EngineeringEventKind::VerificationFinished)
                .with_changeset(changeset)
                .with_verification(verification)
                .with_attribute("success", Value::Bool(true))
                .with_attribute("command_count", json!(2)),
        );
        events
    }

    #[test]
    fn violation_blocks_readiness_until_human_override() {
        let mut events = changeset_events();
        let report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Violation),
        );
        assert_eq!(report.gate.state, CommitGateState::ScopeViolation);
        assert!(!report.gate.ready);
        assert_eq!(report.origin.workers.len(), 1);

        let changeset = ChangeSetId::try_new("run-1-changeset").unwrap();
        events.push(
            event(EngineeringEventKind::HumanOverride)
                .with_changeset(changeset)
                .with_attribute("override_kind", Value::String("scope_violation".into()))
                .with_attribute(
                    "reason",
                    Value::String("Required documentation update".into()),
                ),
        );
        let report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Violation),
        );
        assert!(report.scope_override.is_some());
        assert_eq!(report.gate.state, CommitGateState::NeedsReview);
    }

    #[test]
    fn accepted_and_verified_changeset_is_ready_after_freshness_reconciliation() {
        let events = accepted_verified_events();
        let mut report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Compliant),
        );
        assert_eq!(report.review_state, ChangeReviewState::Accepted);
        assert_eq!(report.verification.state, ChangeVerificationState::Passed);
        assert_eq!(report.verification.fresh, None);
        assert_eq!(report.gate.state, CommitGateState::Ready);

        reconcile_verification_freshness(&mut report, true, None);
        assert_eq!(report.verification.fresh, Some(true));
        assert_eq!(report.gate.state, CommitGateState::Ready);
        assert!(report.gate.ready);
    }

    #[test]
    fn stale_passed_verification_cannot_keep_commit_gate_ready() {
        let events = accepted_verified_events();
        let mut report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Compliant),
        );

        reconcile_verification_freshness(
            &mut report,
            false,
            Some("Index tree changed after verification.".into()),
        );

        assert_eq!(report.verification.fresh, Some(false));
        assert_eq!(report.gate.state, CommitGateState::VerificationStale);
        assert!(!report.gate.ready);
        assert_eq!(
            report.gate.blockers,
            vec!["Index tree changed after verification."]
        );
    }

    #[test]
    fn stale_verification_does_not_hide_scope_blocker() {
        let events = accepted_verified_events();
        let mut report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Violation),
        );

        reconcile_verification_freshness(&mut report, false, None);

        assert_eq!(report.gate.state, CommitGateState::ScopeViolation);
        assert!(!report.gate.ready);
    }

    #[test]
    fn scope_change_after_override_invalidates_it() {
        let mut events = changeset_events();
        let changeset = ChangeSetId::try_new("run-1-changeset").unwrap();
        events.push(
            event(EngineeringEventKind::HumanOverride)
                .with_changeset(changeset)
                .with_attribute("override_kind", Value::String("scope_violation".into()))
                .with_attribute("reason", Value::String("One-time exception".into())),
        );
        events.push(event(EngineeringEventKind::ScopeChanged));

        let report = derive_change_governance(
            "task-1",
            &events,
            &contract(ScopeComplianceStatus::Violation),
        );
        assert!(report.scope_override.is_none());
        assert_eq!(report.gate.state, CommitGateState::ScopeViolation);
    }
}
