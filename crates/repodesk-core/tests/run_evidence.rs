use chrono::Utc;
use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::engineering::{
    AcceptanceEvidenceReport, EngineeringEvent, EngineeringEventKind, WorkItemId,
    derive_run_evidence,
};
use repodesk_core::orchestrator::{OrchestrationRun, RunStatus, SubAgentResult, SubAgentStatus};
use repodesk_core::workflow::{
    CheckReceipt, ExecutionMode, ExecutionReceipt, FinishReceipt, ReviewDecision, ReviewReceipt,
    TaskRunReceipt, VerificationReceipt,
};

fn acceptance() -> AcceptanceEvidenceReport {
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

fn result(
    task_id: &str,
    changed_files: &[&str],
    input_tokens: usize,
    output_tokens: usize,
) -> SubAgentResult {
    SubAgentResult {
        task_id: task_id.into(),
        agent: "codex".into(),
        provider: "codex_cli".into(),
        model: "default".into(),
        status: SubAgentStatus::Ok,
        output: String::new(),
        input_tokens,
        output_tokens,
        cost_units: 0.0,
        captured_proposals: 0,
        changed_files: changed_files.iter().map(|path| (*path).into()).collect(),
        change_evidence_status: ChangeEvidenceStatus::Complete,
        execution_issues: Vec::new(),
        diff_path: None,
        workspace: None,
        notes: Vec::new(),
    }
}

fn run() -> OrchestrationRun {
    OrchestrationRun {
        run_id: "run-1".into(),
        project: "repodesk".into(),
        task_id: "task-1".into(),
        goal: "Build evidence".into(),
        status: RunStatus::Completed,
        dry_run: false,
        started_at: "2026-08-07T18:00:00Z".into(),
        finished_at: "2026-08-07T18:01:00Z".into(),
        results: vec![
            result("impl", &["src/lib.rs", "src/shared.rs"], 100, 50),
            result("tests", &["src/shared.rs", "tests/evidence.rs"], 80, 30),
        ],
        total_input_tokens: 180,
        total_output_tokens: 80,
        total_cost_units: 0.0,
    }
}

fn receipt() -> TaskRunReceipt {
    TaskRunReceipt {
        task_id: "task-1".into(),
        run_id: "run-1".into(),
        execution_mode: ExecutionMode::AgentRun,
        base_commit: Some("base".into()),
        execution: ExecutionReceipt {
            status: RunStatus::Completed,
            required_steps: Vec::new(),
            changeset_digest: Some("digest".into()),
        },
        review: Some(ReviewReceipt {
            run_id: "run-1".into(),
            decision: ReviewDecision::Accepted,
            reviewed_paths: vec![
                "src/lib.rs".into(),
                "src/shared.rs".into(),
                "tests/evidence.rs".into(),
            ],
            changeset_digest: "digest".into(),
            index_tree_after_accept: Some("tree".into()),
        }),
        verification: Some(VerificationReceipt {
            run_id: "run-1".into(),
            head_sha: "head".into(),
            index_tree_sha: "tree".into(),
            changeset_digest: "digest".into(),
            commands: vec![CheckReceipt {
                command: "cargo test".into(),
                success: true,
            }],
            success: true,
            verified_at: "2026-08-07T18:02:00Z".into(),
        }),
        finish: Some(FinishReceipt {
            run_id: "run-1".into(),
            commit_sha: "abc123".into(),
            committed_paths: vec![
                "src/lib.rs".into(),
                "src/shared.rs".into(),
                "tests/evidence.rs".into(),
            ],
            finished_at: "2026-08-07T18:03:00Z".into(),
        }),
    }
}

#[test]
fn run_evidence_deduplicates_changed_files_and_prefers_receipt() {
    let work_item = WorkItemId::try_new("task-1").unwrap();
    let mut misleading_review = EngineeringEvent::new(
        "repodesk",
        work_item,
        EngineeringEventKind::ChangeSetReviewed,
    );
    misleading_review.occurred_at = Utc::now();
    misleading_review.attributes.insert(
        "decision".into(),
        serde_json::Value::String("rejected".into()),
    );

    let snapshot = derive_run_evidence(
        &run(),
        Some(&receipt()),
        &[misleading_review],
        acceptance(),
        true,
    );

    assert_eq!(
        snapshot.changed_files,
        vec!["src/lib.rs", "src/shared.rs", "tests/evidence.rs"]
    );
    assert_eq!(snapshot.review.state, "accepted");
    assert_eq!(snapshot.review.source, "task_run_receipt");
    assert_eq!(snapshot.verification.state, "passed");
    assert_eq!(snapshot.verification.commands.len(), 1);
    assert!(snapshot.commit.committed);
    assert_eq!(snapshot.commit.commit_sha.as_deref(), Some("abc123"));
}

#[test]
fn canonical_verification_is_labeled_stale_when_tree_moved() {
    let snapshot = derive_run_evidence(&run(), Some(&receipt()), &[], acceptance(), false);
    assert_eq!(snapshot.verification.state, "stale");
    assert_eq!(snapshot.verification.source, "task_run_receipt");
}
