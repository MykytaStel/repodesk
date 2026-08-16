//! Integration tests for the evidence-bound Work flow: the receipt writers and
//! the bounded commit, exercised against a real temporary git repo + active
//! task. `REPODESK_HOME` is process-global, so every test is `#[serial]`.

use std::path::{Path, PathBuf};

use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::orchestrator::types::{RunStatus, SubAgentStatus};
use repodesk_core::orchestrator::{ReviewAction, ReviewedFile, RunReview, record_review};
use repodesk_core::projects::{AddProjectInput, add_project, use_project};
use repodesk_core::tasks::{NewTaskInput, create_task};
use repodesk_core::workflow::{
    Evidence, ExecutionMode, ExecutionReceipt, Phase, ReviewDecision, StepReceipt, TaskRunReceipt,
    commit_reviewed_index, derive_progress, derive_signals, head_sha, index_tree_sha, load_receipt,
    run_verification, save_receipt,
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
    assert!(git(path, args).status.success(), "git {args:?} failed");
}

fn git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Fresh home + active project (a real git repo) + active task.
fn setup() -> Fixture {
    let home = TempDir::new().expect("tempdir");
    // SAFETY: every caller is `#[serial]`, so the env is not touched concurrently.
    unsafe {
        std::env::set_var("REPODESK_HOME", home.path());
    }
    repodesk_core::init::init_home().expect("init_home");

    let project_path = home.path().join("project");
    std::fs::create_dir_all(&project_path).expect("project dir");
    // A "generic" project has no default checks, so verification passes
    // vacuously — these tests exercise the receipt binding, not real checks.
    add_project(AddProjectInput {
        name: "demo".to_string(),
        path: project_path.clone(),
        project_type: "generic".to_string(),
        main_language: None,
    })
    .expect("add_project");
    use_project("demo").expect("use_project");
    create_task(NewTaskInput {
        title: "demo".to_string(),
        verify_command: None,
    })
    .expect("create_task");

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

/// Save a run receipt whose implementation step changed `changed`.
fn seed_receipt(run_id: &str, changed: &[&str]) {
    let changed: Vec<String> = changed.iter().map(|s| s.to_string()).collect();
    let digest = if changed.is_empty() {
        None
    } else {
        Some(repodesk_core::workflow::changeset_digest(&changed))
    };
    let receipt = TaskRunReceipt {
        task_id: "demo".into(),
        run_id: run_id.into(),
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
            }],
            changeset_digest: digest,
        },
        review: None,
        verification: None,
        finish: None,
    };
    save_receipt(&receipt).expect("save receipt");
}

fn run_review(paths_outcomes: &[(&str, &str)]) -> RunReview {
    RunReview {
        run_id: "run1".into(),
        action: ReviewAction::Accept,
        project: "demo".into(),
        processed: paths_outcomes
            .iter()
            .map(|(path, outcome)| ReviewedFile {
                path: path.to_string(),
                outcome: outcome.to_string(),
            })
            .collect(),
        warnings: vec![],
    }
}

/// Build the same `Evidence` the desktop/CLI build, post-Prepare facts all true.
fn evidence(project_path: &Path) -> Evidence {
    let receipt = load_receipt().ok().flatten();
    let finish_commit_exists = match receipt.as_ref().and_then(|r| r.finish.as_ref()) {
        Some(finish) => repodesk_core::workflow::commit_exists(project_path, &finish.commit_sha),
        None => false,
    };
    Evidence {
        project_ok: true,
        task_ok: true,
        goal_defined: true,
        context_ok: true,
        safety_ok: true,
        route_ready: true,
        cost_estimated: true,
        baseline_checks_ran: false,
        mode: ExecutionMode::AgentRun,
        receipt,
        head_sha: head_sha(project_path),
        index_tree_sha: index_tree_sha(project_path),
        finish_commit_exists,
    }
}

#[test]
#[serial]
fn accept_with_a_skipped_path_does_not_mark_review_done() {
    if !git_available() {
        return;
    }
    let fx = setup();
    std::fs::write(fx.project_path.join("a.txt"), "a\n").unwrap();
    seed_receipt("run1", &["a.txt", "b.txt"]);

    // One file applied, one skipped → accept must fail and write no review.
    let review = run_review(&[("a.txt", "applied and staged"), ("b.txt", "skipped: gone")]);
    let result = record_review("run1", ReviewAction::Accept, &review);
    assert!(result.is_err(), "skipped file must block accept");
    assert!(
        load_receipt().unwrap().unwrap().review.is_none(),
        "no Accepted receipt should be written"
    );
}

#[test]
#[serial]
fn full_path_execute_accept_verify_commit_completes() {
    if !git_available() {
        return;
    }
    let fx = setup();
    let repo = fx.project_path.as_path();

    // 1. Execute: the agent produced a.txt; record the run receipt.
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    seed_receipt("run1", &["a.txt"]);
    assert_eq!(
        derive_progress(&derive_signals(&evidence(repo)), ExecutionMode::AgentRun).current,
        Phase::Review,
        "a successful run with no review rests at Review"
    );

    // 2. Accept: stage exactly the run's file and record the Accepted receipt.
    git_ok(repo, &["add", "a.txt"]);
    record_review(
        "run1",
        ReviewAction::Accept,
        &run_review(&[("a.txt", "applied and staged")]),
    )
    .expect("accept");
    let reviewed = load_receipt().unwrap().unwrap().review.unwrap();
    assert_eq!(reviewed.decision, ReviewDecision::Accepted);
    assert_eq!(
        derive_progress(&derive_signals(&evidence(repo)), ExecutionMode::AgentRun).current,
        Phase::Verify
    );

    // 3. Verify: run verification (no checks configured → passes, bound to tree).
    run_verification().expect("verify");
    assert_eq!(
        derive_progress(&derive_signals(&evidence(repo)), ExecutionMode::AgentRun).current,
        Phase::Finish
    );

    // 4. Finish: commit only the reviewed, staged changeset.
    let outcome = commit_reviewed_index("done").expect("commit");
    assert_eq!(outcome.committed_paths, vec!["a.txt".to_string()]);
    let head = head_sha(repo).unwrap();
    assert_eq!(outcome.commit_sha, head, "finish records the real HEAD sha");
    assert!(repodesk_core::workflow::commit_exists(
        repo,
        &outcome.commit_sha
    ));

    // The flow is now complete.
    assert!(derive_progress(&derive_signals(&evidence(repo)), ExecutionMode::AgentRun).complete);

    // The commit contains exactly a.txt.
    let files = git(repo, &["show", "--name-only", "--pretty=format:", "HEAD"]);
    let listed = String::from_utf8_lossy(&files.stdout);
    assert!(listed.lines().any(|l| l == "a.txt"));
}

#[test]
#[serial]
fn verification_refuses_tree_changes_after_accept() {
    if !git_available() {
        return;
    }
    let fx = setup();
    let repo = fx.project_path.as_path();

    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    seed_receipt("run1", &["a.txt"]);
    git_ok(repo, &["add", "a.txt"]);
    record_review(
        "run1",
        ReviewAction::Accept,
        &run_review(&[("a.txt", "applied and staged")]),
    )
    .expect("accept");

    // Accept bound Review to tree T1. Staging any additional content creates
    // tree T2, so the old review must become stale before checks can run.
    std::fs::write(repo.join("stray.txt"), "stray\n").unwrap();
    git_ok(repo, &["add", "stray.txt"]);

    assert_eq!(
        derive_progress(&derive_signals(&evidence(repo)), ExecutionMode::AgentRun).current,
        Phase::Review,
        "changing the staged tree after Accept reopens Review"
    );
    let err = run_verification().expect_err("stale reviewed tree must block verification");
    assert!(
        err.to_string().contains("review is missing or stale"),
        "unexpected error: {err}"
    );
    assert!(
        load_receipt().unwrap().unwrap().verification.is_none(),
        "a blocked Verify must not mint verification evidence"
    );

    // Nothing was committed: HEAD is still the initial commit.
    let log = git(repo, &["log", "--oneline"]);
    assert_eq!(String::from_utf8_lossy(&log.stdout).lines().count(), 1);

    // Returning to the old path set is not enough to resurrect the previous
    // Accept. Review evidence was invalidated by T2, so the user must explicitly
    // Accept the current tree again before Verify can proceed.
    git_ok(repo, &["restore", "--staged", "stray.txt"]);
    let err = run_verification().expect_err("old Accept must stay invalidated");
    assert!(err.to_string().contains("review is missing or stale"));

    record_review(
        "run1",
        ReviewAction::Accept,
        &run_review(&[("a.txt", "applied and staged")]),
    )
    .expect("re-accept current tree");
    run_verification().expect("verify re-accepted tree");
    let outcome = commit_reviewed_index("done").expect("commit re-accepted tree");
    assert_eq!(outcome.committed_paths, vec!["a.txt".to_string()]);

    // The stray file was never committed.
    let files = git(repo, &["show", "--name-only", "--pretty=format:", "HEAD"]);
    let listed = String::from_utf8_lossy(&files.stdout);
    assert!(!listed.lines().any(|l| l == "stray.txt"));
}
