//! Regression coverage for the durable Finish boundary.
//!
//! These tests simulate the crash/persistence gap directly: a valid Finish
//! intent exists, the exact reviewed commit has already landed, but FinishReceipt
//! has not. Retrying Finish must repair evidence and must not create commit #2.

use std::path::{Path, PathBuf};

use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::orchestrator::types::{RunStatus, SubAgentStatus};
use repodesk_core::orchestrator::{ReviewAction, ReviewedFile, RunReview, record_review};
use repodesk_core::projects::{AddProjectInput, add_project, use_project};
use repodesk_core::tasks::{NewTaskInput, create_task, show_active_task};
use repodesk_core::workflow::{
    ExecutionMode, ExecutionReceipt, StepReceipt, TaskRunReceipt, commit_reviewed_index, head_sha,
    index_tree_sha, load_receipt, run_verification, save_receipt,
};
use serial_test::serial;
use tempfile::TempDir;

struct Fixture {
    _home: TempDir,
    project_path: PathBuf,
}

fn git(path: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("run git")
}

fn git_ok(path: &Path, args: &[&str]) {
    let output = git(path, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn setup() -> Fixture {
    let home = TempDir::new().expect("tempdir");
    // SAFETY: tests are serial, so REPODESK_HOME is not mutated concurrently.
    unsafe {
        std::env::set_var("REPODESK_HOME", home.path());
    }
    repodesk_core::init::init_home().expect("init home");

    let project_path = home.path().join("project");
    std::fs::create_dir_all(&project_path).expect("project dir");
    add_project(AddProjectInput {
        name: "demo".into(),
        path: project_path.clone(),
        project_type: "generic".into(),
        main_language: None,
    })
    .expect("add project");
    use_project("demo").expect("use project");
    create_task(NewTaskInput {
        title: "demo".into(),
        verify_command: None,
    })
    .expect("create task");

    git_ok(&project_path, &["init", "-q"]);
    git_ok(&project_path, &["config", "user.email", "t@example.com"]);
    git_ok(&project_path, &["config", "user.name", "Test"]);
    std::fs::write(project_path.join("seed.txt"), "seed\n").unwrap();
    git_ok(&project_path, &["add", "."]);
    git_ok(&project_path, &["commit", "-qm", "init"]);

    Fixture {
        _home: home,
        project_path,
    }
}

fn seed_verified_change(project_path: &Path) {
    std::fs::write(project_path.join("a.txt"), "a\n").unwrap();
    let changed = vec!["a.txt".to_string()];
    let digest = repodesk_core::workflow::changeset_digest(&changed);
    save_receipt(&TaskRunReceipt {
        task_id: "demo".into(),
        run_id: "run1".into(),
        execution_mode: ExecutionMode::AgentRun,
        base_commit: None,
        execution: ExecutionReceipt {
            status: RunStatus::Completed,
            required_steps: vec![StepReceipt {
                task_id: "impl".into(),
                status: SubAgentStatus::Ok,
                allow_write: true,
                changed_files: changed,
                change_evidence_status: ChangeEvidenceStatus::Complete,
                change_attribution: Default::default(),
            }],
            changeset_digest: Some(digest),
        },
        review: None,
        verification: None,
        finish: None,
    })
    .expect("save run receipt");

    git_ok(project_path, &["add", "a.txt"]);
    record_review(
        "run1",
        ReviewAction::Accept,
        &RunReview {
            run_id: "run1".into(),
            action: ReviewAction::Accept,
            project: "demo".into(),
            processed: vec![ReviewedFile {
                path: "a.txt".into(),
                outcome: "applied and staged".into(),
            }],
            warnings: vec![],
        },
    )
    .expect("accept");
    run_verification().expect("verify");
}

fn write_prepared_finish_intent(project_path: &Path) -> PathBuf {
    let receipt = load_receipt().unwrap().unwrap();
    let review = receipt.review.as_ref().unwrap();
    let verification = receipt.verification.as_ref().unwrap();
    let path = show_active_task()
        .unwrap()
        .config
        .run_dir
        .join("finish-intent.json");
    let intent = serde_json::json!({
        "run_id": receipt.run_id,
        "parent_head_sha": verification.head_sha,
        "reviewed_tree_sha": review.index_tree_after_accept.as_ref().unwrap(),
        "changeset_digest": review.changeset_digest,
        "committed_paths": ["a.txt"],
        "commit_sha": null,
        "recorded_at": "simulated-crash-boundary"
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&intent).unwrap()).unwrap();

    assert_eq!(
        head_sha(project_path).as_deref(),
        Some(verification.head_sha.as_str())
    );
    assert_eq!(
        index_tree_sha(project_path).as_deref(),
        review.index_tree_after_accept.as_deref()
    );
    path
}

#[test]
#[serial]
fn retry_after_commit_without_finish_receipt_repairs_evidence_without_recommitting() {
    if !git_available() {
        return;
    }
    let fixture = setup();
    let repo = fixture.project_path.as_path();
    seed_verified_change(repo);
    let intent_path = write_prepared_finish_intent(repo);

    // Simulate the exact crash window: `git commit` succeeded after the durable
    // Prepared intent, but no FinishReceipt was persisted.
    git_ok(repo, &["commit", "-qm", "already committed"]);
    let committed_sha = head_sha(repo).unwrap();
    assert!(load_receipt().unwrap().unwrap().finish.is_none());

    let outcome = commit_reviewed_index("must not create another commit")
        .expect("retry should repair Finish evidence");
    assert_eq!(outcome.commit_sha, committed_sha);
    assert_eq!(outcome.committed_paths, vec!["a.txt".to_string()]);

    let commit_count = git(repo, &["rev-list", "--count", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&commit_count.stdout).trim(), "2");

    let receipt = load_receipt().unwrap().unwrap();
    let finish = receipt.finish.expect("FinishReceipt repaired");
    assert_eq!(finish.commit_sha, committed_sha);
    assert!(
        !intent_path.exists(),
        "resolved intent should be cleaned up"
    );
}

#[test]
#[serial]
fn pending_finish_refuses_a_different_committed_tree_and_never_recommits() {
    if !git_available() {
        return;
    }
    let fixture = setup();
    let repo = fixture.project_path.as_path();
    seed_verified_change(repo);
    let _intent_path = write_prepared_finish_intent(repo);

    // Replace the verified bytes before committing. The pending intent still
    // points at the original reviewed tree, so recovery must fail closed.
    std::fs::write(repo.join("a.txt"), "different\n").unwrap();
    git_ok(repo, &["add", "a.txt"]);
    git_ok(repo, &["commit", "-qm", "foreign tree"]);
    let foreign_sha = head_sha(repo).unwrap();

    let error = commit_reviewed_index("must stay blocked")
        .expect_err("different committed tree must not be adopted as Finish evidence");
    assert!(
        error.to_string().contains("Finish") || error.to_string().contains("finish"),
        "unexpected error: {error}"
    );
    assert_eq!(head_sha(repo).as_deref(), Some(foreign_sha.as_str()));

    let commit_count = git(repo, &["rev-list", "--count", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&commit_count.stdout).trim(), "2");
    assert!(load_receipt().unwrap().unwrap().finish.is_none());
}
