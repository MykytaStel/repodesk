//! Compact trust projection for the current ChangeSet.
//!
//! The passport is not another source of truth. It deliberately composes the
//! canonical governance, run receipt, and acceptance-evidence projections into
//! one operator-facing answer to: "what exactly is this change and why may I
//! trust it?".

use serde::{Deserialize, Serialize};

use crate::workflow::TaskRunReceipt;

use super::{
    AcceptanceEvidenceReport, ChangeGovernanceSnapshot, ChangeReviewState,
    ChangeVerificationState, CommitGate, ScopeComplianceStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAttributionStrength {
    /// The ChangeSet is linked to a recorded execution. This deliberately does
    /// not claim exact worktree isolation until isolation identity is recorded
    /// as first-class evidence.
    RecordedRun,
    /// The ChangeSet came through an explicit manual handoff/import path.
    Manual,
    /// No execution identity is available for the current ChangeSet.
    Unattributed,
}

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
    pub attribution: ChangeAttributionStrength,
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
    let attribution = match governance.origin.execution_mode.as_deref() {
        Some("manual") => ChangeAttributionStrength::Manual,
        _ if governance.origin.execution_id.is_some() => ChangeAttributionStrength::RecordedRun,
        _ => ChangeAttributionStrength::Unattributed,
    };

    ChangeSetPassport {
        work_item_id: governance.work_item_id.clone(),
        changeset_id: governance.changeset_id.clone(),
        run_id: receipt.map(|receipt| receipt.run_id.clone()),
        baseline_commit: receipt.and_then(|receipt| receipt.base_commit.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::{
        ChangeOrigin, ChangeVerificationEvidence, CommitGateState,
    };

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

    #[test]
    fn passport_never_upgrades_attribution_beyond_recorded_evidence() {
        let report = AcceptanceEvidenceReport {
            configured: true,
            work_item_id: "task-1".into(),
            current_run_id: Some("run-1".into()),
            criteria: Vec::new(),
            proven: 0,
            failed: 0,
            unproven: 0,
        };

        let passport = derive_changeset_passport(&governance(), &report, None);
        assert_eq!(passport.attribution, ChangeAttributionStrength::RecordedRun);
        assert!(passport.verification_fresh);
        assert!(passport.gate.ready);
    }

    #[test]
    fn manual_origin_is_explicit() {
        let mut governance = governance();
        governance.origin.execution_mode = Some("manual".into());
        let report = AcceptanceEvidenceReport {
            configured: false,
            work_item_id: "task-1".into(),
            current_run_id: None,
            criteria: Vec::new(),
            proven: 0,
            failed: 0,
            unproven: 0,
        };

        let passport = derive_changeset_passport(&governance, &report, None);
        assert_eq!(passport.attribution, ChangeAttributionStrength::Manual);
    }
}
