use repodesk_core::engineering::{
    VerificationReplayInput, VerificationReplayReasonCode, VerificationReplayStatus,
    derive_verification_replay,
};
use repodesk_core::orchestrator::RunStatus;
use repodesk_core::workflow::{
    CheckReceipt, ExecutionMode, ExecutionReceipt, TaskRunReceipt, VerificationReceipt,
};

fn receipt(success: bool) -> TaskRunReceipt {
    TaskRunReceipt {
        task_id: "task-replay".into(),
        run_id: "run-replay".into(),
        execution_mode: ExecutionMode::AgentRun,
        base_commit: Some("base".into()),
        execution: ExecutionReceipt {
            status: RunStatus::Completed,
            required_steps: Vec::new(),
            changeset_digest: Some("digest-replay".into()),
        },
        review: None,
        verification: Some(VerificationReceipt {
            run_id: "run-replay".into(),
            head_sha: "head-replay".into(),
            index_tree_sha: "tree-replay".into(),
            changeset_digest: "digest-replay".into(),
            commands: vec![
                CheckReceipt {
                    command: "cargo test".into(),
                    success,
                },
                CheckReceipt {
                    command: "cargo fmt --check".into(),
                    success: true,
                },
            ],
            success,
            verified_at: "2026-09-07T12:00:00Z".into(),
        }),
        finish: None,
    }
}

fn input<'a>(receipt: Option<&'a TaskRunReceipt>) -> VerificationReplayInput<'a> {
    VerificationReplayInput {
        receipt,
        verification_id: Some("verify-replay"),
        current_head_sha: Some("head-replay"),
        current_index_tree_sha: Some("tree-replay"),
        current_changeset_digest: Some("digest-replay"),
        committed_tree_sha: None,
    }
}

#[test]
fn exact_receipt_is_current_and_reusable() {
    let replay = derive_verification_replay(input(Some(&receipt(true))));

    assert_eq!(replay.status, VerificationReplayStatus::Current);
    assert_eq!(replay.reason_code, VerificationReplayReasonCode::ExactMatch);
    assert!(replay.can_rerun == false);
    assert_eq!(replay.passed_commands, 2);
    assert_eq!(replay.failed_commands, 0);
}

#[test]
fn failed_command_is_current_negative_evidence_not_fake_green() {
    let replay = derive_verification_replay(input(Some(&receipt(false))));

    assert_eq!(replay.status, VerificationReplayStatus::Current);
    assert_eq!(
        replay.reason_code,
        VerificationReplayReasonCode::CurrentFailedEvidence
    );
    assert_eq!(replay.passed_commands, 1);
    assert_eq!(replay.failed_commands, 1);
}

#[test]
fn changed_index_tree_is_stale_and_recommends_rerun() {
    let stored_receipt = receipt(true);
    let mut facts = input(Some(&stored_receipt));
    facts.current_index_tree_sha = Some("tree-new");

    let replay = derive_verification_replay(facts);

    assert_eq!(replay.status, VerificationReplayStatus::Stale);
    assert_eq!(
        replay.reason_code,
        VerificationReplayReasonCode::IndexTreeChanged
    );
    assert!(replay.can_rerun);
}

#[test]
fn changed_changeset_digest_is_stale_even_when_trees_match() {
    let stored_receipt = receipt(true);
    let mut facts = input(Some(&stored_receipt));
    facts.current_changeset_digest = Some("digest-new");

    let replay = derive_verification_replay(facts);

    assert_eq!(replay.status, VerificationReplayStatus::Stale);
    assert_eq!(
        replay.reason_code,
        VerificationReplayReasonCode::ChangesetChanged
    );
}

#[test]
fn missing_receipt_is_explicitly_missing() {
    let replay = derive_verification_replay(input(None));

    assert_eq!(replay.status, VerificationReplayStatus::Missing);
    assert_eq!(
        replay.reason_code,
        VerificationReplayReasonCode::ReceiptMissing
    );
    assert!(replay.can_rerun);
}

#[test]
fn unavailable_current_tree_never_claims_stale_or_current() {
    let stored_receipt = receipt(true);
    let mut facts = input(Some(&stored_receipt));
    facts.current_head_sha = None;

    let replay = derive_verification_replay(facts);

    assert_eq!(replay.status, VerificationReplayStatus::Unavailable);
    assert_eq!(
        replay.reason_code,
        VerificationReplayReasonCode::CurrentHeadUnavailable
    );
    assert!(!replay.can_rerun);
}
