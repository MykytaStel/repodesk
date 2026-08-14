//! Live verification reconciliation for the event-derived ChangeSet projection.
//!
//! Historical engineering events remain immutable evidence that verification ran.
//! This module owns the separate question of whether that proof is still bound
//! to the current canonical run receipt and Git tree.

use super::{
    ChangeGovernanceSnapshot, ChangeVerificationState, CommitGate, CommitGateState,
};

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

    // Preserve earlier/higher-priority blockers such as scope or review. Only a
    // gate that would otherwise claim readiness is downgraded by stale proof.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::{
        ChangeOrigin, ChangeReviewState, ChangeVerificationEvidence, ScopeComplianceStatus,
    };

    fn ready_snapshot() -> ChangeGovernanceSnapshot {
        ChangeGovernanceSnapshot {
            work_item_id: "task-1".into(),
            changeset_id: Some("run-1-changeset".into()),
            origin: ChangeOrigin::default(),
            files: Vec::new(),
            scope_status: ScopeComplianceStatus::Compliant,
            review_state: ChangeReviewState::Accepted,
            verification: ChangeVerificationEvidence {
                state: ChangeVerificationState::Passed,
                verification_id: Some("verify-1".into()),
                command_count: 2,
                evidence: Vec::new(),
                error: None,
                fresh: None,
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
    fn fresh_receipt_preserves_ready_gate() {
        let mut snapshot = ready_snapshot();
        reconcile_verification_freshness(&mut snapshot, true, None);
        assert_eq!(snapshot.verification.fresh, Some(true));
        assert_eq!(snapshot.gate.state, CommitGateState::Ready);
        assert!(snapshot.gate.ready);
    }

    #[test]
    fn stale_receipt_downgrades_ready_gate() {
        let mut snapshot = ready_snapshot();
        reconcile_verification_freshness(
            &mut snapshot,
            false,
            Some("Index tree changed after verification.".into()),
        );
        assert_eq!(snapshot.verification.fresh, Some(false));
        assert_eq!(snapshot.gate.state, CommitGateState::VerificationStale);
        assert!(!snapshot.gate.ready);
        assert_eq!(
            snapshot.gate.blockers,
            vec!["Index tree changed after verification."]
        );
    }

    #[test]
    fn stale_receipt_does_not_hide_higher_priority_blocker() {
        let mut snapshot = ready_snapshot();
        snapshot.gate = CommitGate {
            state: CommitGateState::ScopeViolation,
            ready: false,
            blockers: vec!["scope".into()],
            warnings: Vec::new(),
        };
        reconcile_verification_freshness(&mut snapshot, false, None);
        assert_eq!(snapshot.gate.state, CommitGateState::ScopeViolation);
    }
}
