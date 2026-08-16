use super::*;
use crate::change_attribution::ChangeAttributionStrength;
use tempfile::tempdir;

fn step(id: &str, status: SubAgentStatus, allow_write: bool) -> StepReceipt {
    StepReceipt {
        task_id: id.to_string(),
        status,
        allow_write,
        changed_files: Vec::new(),
        change_evidence_status: ChangeEvidenceStatus::Complete,
        change_attribution: ChangeAttributionEvidence::default(),
    }
}

fn reviewed_receipt(tree: Option<&str>) -> TaskRunReceipt {
    let paths = vec!["src/a.rs".to_string()];
    let digest = changeset_digest(&paths);
    TaskRunReceipt {
        task_id: "t1".into(),
        run_id: "r1".into(),
        execution_mode: ExecutionMode::AgentRun,
        base_commit: Some("base".into()),
        execution: ExecutionReceipt {
            status: RunStatus::Completed,
            required_steps: vec![step("impl", SubAgentStatus::Ok, true)],
            changeset_digest: Some(digest.clone()),
        },
        review: Some(ReviewReceipt {
            run_id: "r1".into(),
            decision: ReviewDecision::Accepted,
            reviewed_paths: paths,
            changeset_digest: digest.clone(),
            index_tree_after_accept: tree.map(str::to_string),
        }),
        verification: Some(VerificationReceipt {
            run_id: "r1".into(),
            head_sha: "head".into(),
            index_tree_sha: tree.unwrap_or_default().into(),
            changeset_digest: digest,
            commands: vec![],
            success: true,
            verified_at: "now".into(),
        }),
        finish: None,
    }
}

#[test]
fn legacy_step_receipt_defaults_attribution_to_unknown() {
    let json = r#"{
        "task_id":"impl",
        "status":"ok",
        "allow_write":true,
        "changed_files":["src/lib.rs"],
        "change_evidence_status":"complete"
    }"#;
    let receipt: StepReceipt = serde_json::from_str(json).expect("legacy receipt");
    assert_eq!(
        receipt.change_attribution.strength,
        ChangeAttributionStrength::LegacyUnknown
    );
}

#[test]
fn step_receipt_round_trips_typed_attribution() {
    let mut receipt = step("impl", SubAgentStatus::Ok, true);
    receipt.change_attribution = ChangeAttributionEvidence {
        strength: ChangeAttributionStrength::ExactIsolated,
        workspace_id: Some("workspace-1".into()),
        baseline_commit: Some("abc123".into()),
        reason: Some("managed isolated worktree".into()),
    };
    let encoded = serde_json::to_string(&receipt).expect("serialize");
    let decoded: StepReceipt = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded, receipt);
}

#[test]
fn execution_requires_all_write_steps_ok() {
    let exec = ExecutionReceipt {
        status: RunStatus::Partial,
        required_steps: vec![
            step("prepare", SubAgentStatus::Ok, false),
            step("implement", SubAgentStatus::Failed, true),
        ],
        changeset_digest: None,
    };
    assert!(!exec.succeeded());

    let exec_ok = ExecutionReceipt {
        status: RunStatus::Partial,
        required_steps: vec![
            step("prepare", SubAgentStatus::Skipped, false),
            step("implement", SubAgentStatus::Ok, true),
        ],
        changeset_digest: None,
    };
    assert!(exec_ok.succeeded());
}

#[test]
fn execution_without_write_steps_needs_all_ok() {
    let analysis = ExecutionReceipt {
        status: RunStatus::Completed,
        required_steps: vec![
            step("analyze", SubAgentStatus::Ok, false),
            step("summarize", SubAgentStatus::Ok, false),
        ],
        changeset_digest: None,
    };
    assert!(analysis.succeeded());
    let blocked = ExecutionReceipt {
        status: RunStatus::Partial,
        required_steps: vec![
            step("analyze", SubAgentStatus::Ok, false),
            step("summarize", SubAgentStatus::Blocked, false),
        ],
        changeset_digest: None,
    };
    assert!(!blocked.succeeded());
}

#[test]
fn empty_execution_is_not_success() {
    let exec = ExecutionReceipt {
        status: RunStatus::Failed,
        required_steps: vec![],
        changeset_digest: None,
    };
    assert!(!exec.succeeded());
}

#[test]
fn digest_is_order_independent_and_path_sensitive() {
    let a = changeset_digest(&["src/b.rs".into(), "src/a.rs".into()]);
    let b = changeset_digest(&["src/a.rs".into(), "src/b.rs".into()]);
    assert_eq!(a, b);
    let c = changeset_digest(&["src/a.rs".into()]);
    assert_ne!(a, c);
}

#[test]
fn verification_validity_tracks_head_index_and_digest() {
    let v = VerificationReceipt {
        run_id: "r1".into(),
        head_sha: "head".into(),
        index_tree_sha: "tree".into(),
        changeset_digest: "dig".into(),
        commands: vec![],
        success: true,
        verified_at: "now".into(),
    };
    assert!(v.valid_for("head", "tree", "dig"));
    assert!(!v.valid_for("head2", "tree", "dig"));
    assert!(!v.valid_for("head", "tree2", "dig"));
    assert!(!v.valid_for("head", "tree", "dig2"));
    let failed = VerificationReceipt {
        success: false,
        ..v
    };
    assert!(!failed.valid_for("head", "tree", "dig"));
}

#[test]
fn accepted_review_is_bound_to_exact_tree_not_only_paths() {
    let receipt = reviewed_receipt(Some("tree-t1"));
    assert_eq!(
        reviewed_tree_sha_for(&receipt, "tree-t1")
            .unwrap()
            .as_deref(),
        Some("tree-t1")
    );
    assert!(reviewed_tree_sha_for(&receipt, "tree-t2").is_err());
}

#[test]
fn staged_tree_change_invalidates_review_and_verification() {
    let mut receipt = reviewed_receipt(Some("tree-t1"));
    assert!(receipt.invalidate_stale_review_tree(Some("tree-t2")));
    assert!(receipt.review.is_none());
    assert!(receipt.verification.is_none());
    assert!(receipt.finish.is_none());
}

#[test]
fn accepted_review_without_tree_proof_is_rejected() {
    let receipt = reviewed_receipt(None);
    assert!(validate_receipt(&receipt).is_err());
}

#[test]
fn receipt_file_round_trips_through_atomic_storage() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("task-run-receipt.json");
    let receipt = reviewed_receipt(Some("tree-t1"));

    persist_receipt_file(&path, &receipt).unwrap();
    let loaded = read_receipt_file(&path).unwrap().unwrap();
    assert_eq!(loaded.run_id, receipt.run_id);
    assert_eq!(
        loaded.review.unwrap().index_tree_after_accept.as_deref(),
        Some("tree-t1")
    );
}

#[test]
fn corrupt_receipt_is_an_error_not_missing_evidence() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("task-run-receipt.json");
    fs::write(&path, b"{not-json").unwrap();

    let error = read_receipt_file(&path).expect_err("corruption must fail closed");
    assert!(error.to_string().contains("corrupt or invalid JSON"));
}

#[cfg(unix)]
#[test]
fn symlinked_receipt_is_rejected() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let target = dir.path().join("target.json");
    let link = dir.path().join("task-run-receipt.json");
    fs::write(&target, b"{}").unwrap();
    symlink(&target, &link).unwrap();

    let error = read_receipt_file(&link).expect_err("symlink must fail closed");
    assert!(error.to_string().contains("symlink"));
}
