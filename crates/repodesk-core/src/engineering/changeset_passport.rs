//! Compact trust projection for the current ChangeSet.
//!
//! The passport is not another source of truth. It deliberately composes the
//! canonical governance, run receipt, and acceptance-evidence projections into
//! one operator-facing answer to: "what exactly is this change and why may I
//! trust it?".

use serde::{Deserialize, Serialize};

use crate::change_attribution::{
    ChangeAttributionEvidence, ChangeAttributionStrength, aggregate_change_attribution,
};
use crate::workflow::TaskRunReceipt;

use super::{
    AcceptanceEvidenceReport, ChangeGovernanceSnapshot, ChangeReviewState, ChangeVerificationState,
    CommitGate, ScopeComplianceStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCoverageSummary {
    pub configured: bool,
    pub total: usize,
    pub proven: usize,
    pub failed: usize,
    pub unproven: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSetPassport {
    pub work_item_id: String,
    pub changeset_id: Option<String>,
    pub run_id: Option<String>,
    pub baseline_commit: Option<String>,
    pub attribution: ChangeAttributionEvidence,
    pub changed_file_count: usize,
    pub scope_status: ScopeComplianceStatus,
    pub review_state: ChangeReviewState,
    pub verification_state: ChangeVerificationState,
    pub verification_fresh: bool,
    pub acceptance: AcceptanceCoverageSummary,
    pub committed: bool,
    pub commit_sha: Option<String>,
    pub gate: CommitGate,
}

pub fn derive_changeset_passport(
    governance: &ChangeGovernanceSnapshot,
    acceptance: &AcceptanceEvidenceReport,
    receipt: Option<&TaskRunReceipt>,
) -> ChangeSetPassport {
    let attribution = receipt
        .map(derive_receipt_change_attribution)
        .unwrap_or_else(unattributed_without_receipt);

    ChangeSetPassport {
        work_item_id: governance.work_item_id.clone(),
        changeset_id: governance.changeset_id.clone(),
        run_id: receipt.map(|receipt| receipt.run_id.clone()),
        baseline_commit: receipt
            .and_then(|receipt| receipt.base_commit.clone())
            .or_else(|| attribution.baseline_commit.clone()),
        attribution,
        changed_file_count: governance.files.len(),
        scope_status: governance.scope_status,
        review_state: governance.review_state,
        verification_state: governance.verification.state,
        verification_fresh: governance.verification.fresh == Some(true),
        acceptance: AcceptanceCoverageSummary {
            configured: acceptance.configured,
            total: acceptance.criteria.len(),
            proven: acceptance.proven,
            failed: acceptance.failed,
            unproven: acceptance.unproven,
        },
        committed: governance.committed,
        commit_sha: governance.commit_sha.clone(),
        gate: governance.gate.clone(),
    }
}

/// Derive ChangeSet-level producer attribution exclusively from durable step
/// receipts. This is shared by the Passport and Safe Commit Manifest so both
/// trust surfaces use the same weakest-proof-wins semantics.
pub(crate) fn derive_receipt_change_attribution(
    receipt: &TaskRunReceipt,
) -> ChangeAttributionEvidence {
    let contributors: Vec<ChangeAttributionEvidence> = receipt
        .execution
        .required_steps
        .iter()
        .filter(|step| step.allow_write && !step.changed_files.is_empty())
        .map(|step| step.change_attribution.clone())
        .collect();

    if contributors.is_empty() {
        if receipt.execution.changeset_digest.is_some() {
            ChangeAttributionEvidence {
                strength: ChangeAttributionStrength::Unattributed,
                reason: Some(
                    "the receipt has a ChangeSet but no producer-attribution record for a contributing write step"
                        .to_string(),
                ),
                ..ChangeAttributionEvidence::default()
            }
        } else {
            ChangeAttributionEvidence::default()
        }
    } else {
        aggregate_change_attribution(&contributors)
    }
}

fn unattributed_without_receipt() -> ChangeAttributionEvidence {
    ChangeAttributionEvidence {
        strength: ChangeAttributionStrength::Unattributed,
        reason: Some("no durable execution receipt is available for this ChangeSet".to_string()),
        ..ChangeAttributionEvidence::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_evidence::ChangeEvidenceStatus;
    use crate::engineering::{ChangeOrigin, ChangeVerificationEvidence, CommitGateState};
    use crate::orchestrator::types::{RunStatus, SubAgentStatus};
    use crate::workflow::{ExecutionMode, ExecutionReceipt, StepReceipt};

    fn governance() -> ChangeGovernanceSnapshot {
        ChangeGovernanceSnapshot {
            work_item_id: "task-1".into(),
            changeset_id: Some("run-1-changeset".into()),
            origin: ChangeOrigin {
                execution_id: Some("run-1".into()),
                execution_mode: Some("managed".into()),
                workers: Vec::new(),
            },
            files: Vec::new(),
            scope_status: ScopeComplianceStatus::Compliant,
            review_state: ChangeReviewState::Accepted,
            verification: ChangeVerificationEvidence {
                state: ChangeVerificationState::Passed,
                verification_id: Some("verify-1".into()),
                command_count: 2,
                evidence: Vec::new(),
                error: None,
                fresh: Some(true),
                stale_reason: None,
            },
            scope_override: None,
            committed: false,
            commit_sha: None,
            gate: CommitGate {
                state: CommitGateState::Ready,
                ready: true,
                blockers: Vec::new(),
                warnings: Vec::new(),
            },
        }
    }

    fn report() -> AcceptanceEvidenceReport {
        AcceptanceEvidenceReport {
            configured: true,
            work_item_id: "task-1".into(),
            current_run_id: Some("run-1".into()),
            criteria: Vec::new(),
            proven: 0,
            failed: 0,
            unproven: 0,
        }
    }

    fn receipt(attribution: ChangeAttributionEvidence) -> TaskRunReceipt {
        let changed = vec!["src/lib.rs".to_string()];
        TaskRunReceipt {
            task_id: "task-1".into(),
            run_id: "run-1".into(),
            execution_mode: ExecutionMode::AgentRun,
            base_commit: Some("base".into()),
            execution: ExecutionReceipt {
                status: RunStatus::Completed,
                required_steps: vec![StepReceipt {
                    task_id: "impl".into(),
                    status: SubAgentStatus::Ok,
                    allow_write: true,
                    changed_files: changed.clone(),
                    change_evidence_status: ChangeEvidenceStatus::Complete,
                    change_attribution: attribution,
                }],
                changeset_digest: Some(crate::workflow::changeset_digest(&changed)),
            },
            review: None,
            verification: None,
            finish: None,
        }
    }

    #[test]
    fn execution_id_alone_never_upgrades_attribution() {
        let passport = derive_changeset_passport(&governance(), &report(), None);
        assert_eq!(
            passport.attribution.strength,
            ChangeAttributionStrength::Unattributed
        );
        assert!(passport.verification_fresh);
        assert!(passport.gate.ready);
    }

    #[test]
    fn passport_uses_exact_isolated_receipt_evidence() {
        let receipt = receipt(ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::ExactIsolated,
            workspace_id: Some("workspace-1".into()),
            baseline_commit: Some("base".into()),
            reason: Some("exact".into()),
        });
        let passport = derive_changeset_passport(&governance(), &report(), Some(&receipt));
        assert_eq!(
            passport.attribution.strength,
            ChangeAttributionStrength::ExactIsolated
        );
        assert_eq!(passport.attribution.workspace_id.as_deref(), Some("workspace-1"));
    }

    #[test]
    fn legacy_receipt_stays_legacy_unknown() {
        let passport = derive_changeset_passport(
            &governance(),
            &report(),
            Some(&receipt(ChangeAttributionEvidence::default())),
        );
        assert_eq!(
            passport.attribution.strength,
            ChangeAttributionStrength::LegacyUnknown
        );
    }

    #[test]
    fn manual_attribution_comes_from_receipt_not_origin_string() {
        let mut governance = governance();
        governance.origin.execution_mode = Some("managed".into());
        let receipt = receipt(ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::Manual,
            reason: Some("manual handoff".into()),
            ..ChangeAttributionEvidence::default()
        });
        let passport = derive_changeset_passport(&governance, &report(), Some(&receipt));
        assert_eq!(passport.attribution.strength, ChangeAttributionStrength::Manual);
    }
}
