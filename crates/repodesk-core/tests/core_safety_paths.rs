//! Integration tests for the stateful safety/guard/security paths.
//!
//! These functions read the active project + task from `REPODESK_HOME`, so each
//! test runs against an isolated temporary home. `REPODESK_HOME` is a process-global
//! env var, so every test here is `#[serial]` to prevent cross-test interference.

use std::path::PathBuf;

use repodesk_core::guard::{GuardLevel, preflight};
use repodesk_core::judge::{JudgementDecision, judge_agent};
use repodesk_core::persistence::{count_action_runs, recent_action_runs, record_action_run};
use repodesk_core::projects::{AddProjectInput, add_project, use_project};
use repodesk_core::safety::{SafetyLevel, scan_active_context};
use repodesk_core::security::{SecurityLevel, audit_security_policy};
use repodesk_core::tasks::{NewTaskInput, create_task};
use repodesk_core::workflow::{ActionRunResult, CommandResult};
use serial_test::serial;
use tempfile::TempDir;

/// An isolated RepoDesk home with one active project and one active task.
struct Fixture {
    _home: TempDir,
    run_dir: PathBuf,
    project_path: PathBuf,
}

impl Fixture {
    fn write_run_file(&self, name: &str, content: &str) {
        std::fs::write(self.run_dir.join(name), content).expect("write run file");
    }
}

/// Build a fresh home + active project/task. Callers must be `#[serial]`.
fn setup() -> Fixture {
    let home = TempDir::new().expect("tempdir");
    // SAFETY: all callers are `#[serial]`, so no other thread touches the env
    // concurrently while we set the per-test home.
    unsafe {
        std::env::set_var("REPODESK_HOME", home.path());
    }

    repodesk_core::init::init_home().expect("init_home");

    let project_path = home.path().join("project");
    std::fs::create_dir_all(&project_path).expect("project dir");

    add_project(AddProjectInput {
        name: "demo".to_string(),
        path: project_path.clone(),
        project_type: "rust".to_string(),
        main_language: Some("rust".to_string()),
    })
    .expect("add_project");
    use_project("demo").expect("use_project");

    let task = create_task(NewTaskInput {
        title: "demo task".to_string(),
    })
    .expect("create_task");

    Fixture {
        _home: home,
        run_dir: task.config.run_dir,
        project_path,
    }
}

// --- safety::scan_active_context ---------------------------------------------

#[test]
#[serial]
fn scan_active_context_is_ok_for_clean_content() {
    let fx = setup();
    fx.write_run_file(
        "context.md",
        "Ordinary task notes. Nothing sensitive here.\n",
    );

    let report = scan_active_context().expect("scan");
    assert_eq!(report.level, SafetyLevel::Ok);
    assert!(report.findings.is_empty());
}

#[test]
#[serial]
fn scan_active_context_blocks_on_private_key() {
    let fx = setup();
    fx.write_run_file(
        "context.md",
        "config dump:\n-----BEGIN PRIVATE KEY-----\nMII...\n",
    );

    let report = scan_active_context().expect("scan");
    assert_eq!(report.level, SafetyLevel::Block);
}

#[test]
#[serial]
fn scan_active_context_errors_without_context_file() {
    let _fx = setup();
    // No context.md written -> the scan cannot read the file.
    assert!(scan_active_context().is_err());
}

// --- security::audit_security_policy -----------------------------------------

#[test]
#[serial]
fn audit_warns_when_context_and_checks_missing() {
    let _fx = setup();
    // Default policy is conservative, but required artifacts are absent.
    let audit = audit_security_policy().expect("audit");
    assert_eq!(audit.level, SecurityLevel::Warning);
    assert!(audit.findings.iter().any(|f| f.contains("context.md")));
    assert!(
        audit
            .findings
            .iter()
            .any(|f| f.contains("checks-summary.md"))
    );
}

#[test]
#[serial]
fn audit_is_ok_when_required_artifacts_present() {
    let fx = setup();
    fx.write_run_file("context.md", "task context\n");
    fx.write_run_file("checks-summary.md", "Overall status: `passed`\n");

    let audit = audit_security_policy().expect("audit");
    assert_eq!(audit.level, SecurityLevel::Ok);
}

// --- guard::preflight --------------------------------------------------------

#[test]
#[serial]
fn preflight_blocks_when_context_missing() {
    let _fx = setup();
    let result = preflight("codex").expect("preflight");
    assert_eq!(result.level, GuardLevel::Block);
    assert!(result.reasons.iter().any(|r| r.contains("context.md")));
}

#[test]
#[serial]
fn preflight_warns_when_prompt_and_summary_missing() {
    let fx = setup();
    fx.write_run_file("context.md", "small clean context\n");
    // No prompt.codex.md and no checks-summary.md -> two warnings, no block.
    let result = preflight("codex").expect("preflight");
    assert_eq!(result.level, GuardLevel::Warning);
}

#[test]
#[serial]
fn preflight_is_ok_when_everything_ready() {
    let fx = setup();
    fx.write_run_file("context.md", "small clean context\n");
    fx.write_run_file("prompt.codex.md", "bounded prompt\n");
    fx.write_run_file("checks-summary.md", "Overall status: `passed`\n");

    let result = preflight("codex").expect("preflight");
    assert_eq!(result.level, GuardLevel::Ok);
}

// --- judge::judge_agent (composes preflight + safety + budget) ----------------

#[test]
#[serial]
fn judge_blocks_when_context_missing() {
    let _fx = setup();
    // No context.md -> preflight blocks -> overall BLOCK.
    let report = judge_agent("codex").expect("judge");
    assert_eq!(report.decision, JudgementDecision::Block);
}

#[test]
#[serial]
fn judge_warns_with_bare_context() {
    let fx = setup();
    fx.write_run_file("context.md", "small clean context\n");
    // Context is safe and within budget, but prompt/summary are missing -> WARN.
    let report = judge_agent("codex").expect("judge");
    assert_eq!(report.decision, JudgementDecision::Warn);
}

#[test]
#[serial]
fn judge_allows_when_everything_ready() {
    let fx = setup();
    fx.write_run_file("context.md", "small clean context\n");
    fx.write_run_file("prompt.codex.md", "bounded prompt\n");
    fx.write_run_file("checks-summary.md", "Overall status: `passed`\n");

    let report = judge_agent("codex").expect("judge");
    assert_eq!(report.decision, JudgementDecision::Allow);
}

#[test]
#[serial]
fn judge_blocks_on_secret_context_even_when_prepared() {
    let fx = setup();
    fx.write_run_file("context.md", "-----BEGIN PRIVATE KEY-----\nMII...\n");
    fx.write_run_file("prompt.codex.md", "bounded prompt\n");
    fx.write_run_file("checks-summary.md", "Overall status: `passed`\n");

    let report = judge_agent("codex").expect("judge");
    assert_eq!(report.decision, JudgementDecision::Block);
}

// --- context::build_context (bounded by construction) ------------------------

#[test]
#[serial]
fn build_context_includes_git_metadata_but_not_file_bodies() {
    let fx = setup();

    // Make the project a real git repo so build_context actually exercises its
    // git-metadata path (branch / status / diff-stat / changed-file *names*).
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&fx.project_path)
            .output()
            .expect("run git");
        assert!(status.status.success(), "git {:?} failed", args);
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test"]);
    git(&["config", "commit.gpgsign", "false"]);

    // A tracked source file whose *body* is a secret that must never be ingested
    // into the context pack — only RepoDesk-managed files + git metadata may be.
    let body_secret = "TOPSECRETBODY_DO_NOT_LEAK_8f3a";
    let source = fx.project_path.join("leaked_source.rs");
    std::fs::write(
        &source,
        format!("fn main() {{ let k = \"{body_secret}\"; }}\n"),
    )
    .expect("write source");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
    // Modify it so it shows up as a changed file (status + diff --name-only).
    std::fs::write(
        &source,
        format!("fn main() {{ let k = \"{body_secret}\"; let n = 1; }}\n"),
    )
    .expect("modify source");

    repodesk_core::context::build_context().expect("build_context");
    let context = std::fs::read_to_string(fx.run_dir.join("context.md")).expect("read context");

    // The changed-file *name* is included (proves the git-metadata path ran)...
    assert!(
        context.contains("leaked_source.rs"),
        "expected changed-file name in context pack (git metadata)"
    );
    // ...but the file *body* must never be.
    assert!(
        !context.contains(body_secret),
        "context pack leaked raw repo file contents"
    );
}

// --- persistence::action_history (SQLite, migration v2) ----------------------

#[test]
#[serial]
fn action_history_round_trips_through_sqlite() {
    let _fx = setup();

    let run = ActionRunResult {
        id: "context-build".to_string(),
        title: "Build context".to_string(),
        risk: "safe".to_string(),
        category: "Context".to_string(),
        started_at_ms: 1,
        finished_at_ms: 2,
        result: CommandResult {
            ok: true,
            command: "repodesk context build".to_string(),
            stdout: "done".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        },
    };

    record_action_run(&run).expect("record");
    record_action_run(&run).expect("record 2");

    assert_eq!(count_action_runs().expect("count"), 2);
    let recent = recent_action_runs(10).expect("recent");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, "context-build");
    assert!(recent[0].result.ok);
}
