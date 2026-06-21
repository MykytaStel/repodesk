//! Integration tests for the stateful safety/guard/security paths.
//!
//! These functions read the active project + task from `REPODESK_HOME`, so each
//! test runs against an isolated temporary home. `REPODESK_HOME` is a process-global
//! env var, so every test here is `#[serial]` to prevent cross-test interference.

use std::path::{Path, PathBuf};

use repodesk_core::api_clients::{ProviderSettings, ThinkingLevel};
use repodesk_core::guard::{GuardLevel, preflight};
use repodesk_core::judge::{JudgementDecision, judge_agent};
use repodesk_core::orchestrator::{
    ExecutionAuthorization, OrchestrationPlan, OrchestrationRun, ReviewAction, RunOptions,
    RunStatus, SubAgentResult, SubAgentStatus, SubAgentTask, list_runs, review_run, run_plan,
};
use repodesk_core::persistence::event_journal::{LogEventInput, log_event, read_task_events};
use repodesk_core::persistence::{count_action_runs, recent_action_runs, record_action_run};
use repodesk_core::projects::{AddProjectInput, add_project, read_active_project, use_project};
use repodesk_core::repopilot::{load_history, parse_review_json, record_report};
use repodesk_core::routing::types::{ExecutorKind, TaskKind};
use repodesk_core::safety::{SafetyLevel, scan_active_context};
use repodesk_core::security::{SecurityLevel, audit_security_policy};
use repodesk_core::tasks::{NewTaskInput, create_task, show_active_task};
use repodesk_core::usage::token_ledger::{LogTokenInput, cost_trend, log_token_event};
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
        verify_command: None,
    })
    .expect("create_task");

    Fixture {
        _home: home,
        run_dir: task.config.run_dir,
        project_path,
    }
}

fn init_git_repo(path: &Path) {
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .output()
                .expect("run git")
                .status
                .success(),
            "git {args:?} failed"
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(path.join("seed.txt"), "seed\n").expect("seed file");
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);
}

// --- repopilot health trend --------------------------------------------------

#[test]
#[serial]
fn repopilot_trend_appends_real_reviews_and_skips_errors() {
    let _fx = setup();

    // Fresh task: no history yet.
    assert!(load_history().expect("load").points.is_empty());

    let first = parse_review_json(
        r#"{"health_score": 80, "findings": [{"severity":"high","file":"a.rs"}]}"#,
    );
    let after_first = record_report(&first).expect("record first");
    assert_eq!(after_first.points.len(), 1);
    assert_eq!(after_first.points[0].health_score, Some(80));
    assert_eq!(after_first.points[0].blocking, 1);

    // An errored report (e.g. RepoPilot not installed) must not pollute the trend.
    let errored = parse_review_json("not json");
    let after_error = record_report(&errored).expect("record error");
    assert_eq!(after_error.points.len(), 1);

    let second = parse_review_json(r#"{"health_score": 95, "findings": []}"#);
    let after_second = record_report(&second).expect("record second");
    assert_eq!(after_second.points.len(), 2);
    assert_eq!(after_second.points[1].health_score, Some(95));

    // Persistence roundtrips through the run dir.
    assert_eq!(load_history().expect("reload").points.len(), 2);
}

// --- task switching ----------------------------------------------------------

#[test]
#[serial]
fn use_project_writes_canonical_active_project_name() {
    let _fx = setup();

    let active = use_project("DEMO").expect("case-insensitive project lookup");

    assert_eq!(active.name, "demo");
    assert_eq!(read_active_project().expect("active project"), "demo");
}

#[test]
#[serial]
fn list_tasks_orders_newest_first_and_use_task_switches_active() {
    use repodesk_core::tasks::{list_tasks, use_task};

    let _fx = setup(); // creates "demo task", set active.

    // Second task becomes the newly active one.
    create_task(NewTaskInput {
        title: "second task".to_string(),
        verify_command: None,
    })
    .expect("create second");

    let tasks = list_tasks().expect("list");
    assert_eq!(tasks.len(), 2);
    // Newest first; the second task is active, the first is not.
    assert_eq!(tasks[0].config.title, "second task");
    assert!(tasks[0].is_active);
    assert_eq!(tasks[1].config.title, "demo task");
    assert!(!tasks[1].is_active);

    // Switch back to the first task.
    let first_id = tasks[1].config.id.clone();
    let switched = use_task(&first_id).expect("use_task");
    assert_eq!(switched.config.id, first_id);

    let tasks = list_tasks().expect("relist");
    assert!(tasks[1].is_active, "first task is active after switch");
    assert!(!tasks[0].is_active);

    // A bogus id is rejected without disturbing the active pointer.
    assert!(use_task("../escape").is_err());
    assert!(use_task("does-not-exist").is_err());
    assert_eq!(
        repodesk_core::tasks::show_active_task()
            .expect("active")
            .config
            .id,
        first_id,
        "active task unchanged after rejected switches"
    );
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

// --- N7-A: orchestrator run history + per-task timeline ----------------------

/// A minimal persisted run with the given id/goal/status.
fn write_run(orchestrate_dir: &std::path::Path, run_id: &str, goal: &str, status: RunStatus) {
    let run = OrchestrationRun {
        run_id: run_id.to_string(),
        project: "demo".to_string(),
        task_id: "task".to_string(),
        goal: goal.to_string(),
        status,
        dry_run: false,
        started_at: "2026-06-17T10:00:00Z".to_string(),
        finished_at: "2026-06-17T10:01:00Z".to_string(),
        results: vec![SubAgentResult {
            task_id: "step-1".to_string(),
            agent: "ollama".to_string(),
            provider: "ollama".to_string(),
            model: "llama3".to_string(),
            status: SubAgentStatus::Ok,
            output: String::new(),
            input_tokens: 10,
            output_tokens: 5,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: Vec::new(),
            diff_path: None,
            workspace: None,
            notes: Vec::new(),
        }],
        total_input_tokens: 10,
        total_output_tokens: 5,
        total_cost_units: 0.0,
    };
    let json = serde_json::to_string_pretty(&run).expect("serialize run");
    std::fs::write(orchestrate_dir.join(format!("{run_id}.json")), &json).expect("write run");
    // The rolling pointer must be ignored by list_runs.
    std::fs::write(orchestrate_dir.join("latest.json"), &json).expect("write latest");
}

#[test]
#[serial]
fn list_runs_returns_summaries_newest_first_and_skips_latest_pointer() {
    let fx = setup();
    let dir = fx.run_dir.join("orchestrate");
    std::fs::create_dir_all(&dir).expect("orchestrate dir");

    write_run(
        &dir,
        "run-20260617-100000",
        "older goal",
        RunStatus::Completed,
    );
    write_run(
        &dir,
        "run-20260617-110000",
        "newer goal",
        RunStatus::Partial,
    );

    let runs = list_runs().expect("list_runs");
    // Two run files (latest.json is skipped, not counted as a third).
    assert_eq!(runs.len(), 2);
    // Newest-first by run id timestamp.
    assert_eq!(runs[0].run_id, "run-20260617-110000");
    assert_eq!(runs[0].goal, "newer goal");
    assert_eq!(runs[0].status, RunStatus::Partial);
    assert_eq!(runs[0].step_count, 1);
    assert_eq!(runs[1].run_id, "run-20260617-100000");
}

#[test]
#[serial]
fn review_run_accepts_isolated_worktree_changesets() {
    let fx = setup();
    init_git_repo(&fx.project_path);
    let dir = fx.run_dir.join("orchestrate");
    std::fs::create_dir_all(&dir).expect("orchestrate dir");
    let run_id = "run-isolated";
    let parent = repodesk_core::worktree::worktrees_parent(&fx.run_dir);
    let worktree = repodesk_core::worktree::create_run_worktree(
        &fx.project_path,
        &parent,
        run_id,
        "implement",
    )
    .expect("create worktree");
    let worktree_path = Path::new(&worktree.path);
    std::fs::write(worktree_path.join("seed.txt"), "seed\nagent\n").expect("edit worktree");
    std::fs::write(worktree_path.join("added.txt"), "new\n").expect("add worktree file");

    let run = OrchestrationRun {
        run_id: run_id.to_string(),
        project: "demo".to_string(),
        task_id: "task".to_string(),
        goal: "isolated".to_string(),
        status: RunStatus::Completed,
        dry_run: false,
        started_at: "2026-06-17T10:00:00Z".to_string(),
        finished_at: "2026-06-17T10:01:00Z".to_string(),
        results: vec![SubAgentResult {
            task_id: "implement".to_string(),
            agent: "codex_cli".to_string(),
            provider: "codex_cli".to_string(),
            model: String::new(),
            status: SubAgentStatus::Ok,
            output: String::new(),
            input_tokens: 10,
            output_tokens: 5,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: vec!["seed.txt".to_string(), "added.txt".to_string()],
            diff_path: None,
            workspace: Some(worktree.clone()),
            notes: Vec::new(),
        }],
        total_input_tokens: 10,
        total_output_tokens: 5,
        total_cost_units: 0.0,
    };
    let json = serde_json::to_string_pretty(&run).expect("serialize run");
    std::fs::write(dir.join(format!("{run_id}.json")), &json).expect("write run");

    let review = review_run(run_id, ReviewAction::Accept).expect("isolated accept");

    assert!(
        review.warnings.is_empty(),
        "unexpected warnings: {:?}",
        review.warnings
    );
    assert!(
        review
            .processed
            .iter()
            .any(|file| file.path == "seed.txt" && file.outcome == "applied and staged")
    );
    assert!(
        review
            .processed
            .iter()
            .any(|file| file.path == "added.txt" && file.outcome == "copied and staged")
    );
    assert_eq!(
        std::fs::read_to_string(fx.project_path.join("seed.txt")).expect("seed"),
        "seed\nagent\n"
    );
    assert_eq!(
        std::fs::read_to_string(fx.project_path.join("added.txt")).expect("added"),
        "new\n"
    );
    let staged = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&fx.project_path)
        .output()
        .expect("git diff cached");
    let staged = String::from_utf8_lossy(&staged.stdout);
    assert!(staged.lines().any(|line| line == "seed.txt"));
    assert!(staged.lines().any(|line| line == "added.txt"));

    repodesk_core::worktree::remove_run_worktree(&fx.project_path, &worktree)
        .expect("cleanup worktree");
}

#[test]
#[serial]
fn read_task_events_filters_to_the_active_task() {
    let _fx = setup();
    let active_id = show_active_task().expect("active task").config.id;

    log_event(LogEventInput {
        module_name: "orchestrator".to_string(),
        level: "info".to_string(),
        message: "run finished".to_string(),
        metadata: vec![],
    })
    .expect("log event");

    let events = read_task_events(&active_id, 10).expect("read_task_events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].task_id, active_id);
    assert_eq!(events[0].message, "run finished");

    // A different task id sees none of the active task's events.
    let other = read_task_events("some-other-task", 10).expect("read_task_events other");
    assert!(other.is_empty());
}

#[test]
#[serial]
fn cost_trend_returns_a_continuous_window_with_todays_usage() {
    let _fx = setup();

    // No usage yet: a 7-day window is still 7 zero points, oldest-first.
    let empty = cost_trend(7).expect("cost_trend empty");
    assert_eq!(empty.len(), 7);
    assert!(
        empty
            .iter()
            .all(|p| p.total_tokens == 0 && p.cost_units == 0.0)
    );

    // Log usage today; it lands on the last (most recent) point.
    log_token_event(LogTokenInput {
        agent: "ollama".to_string(),
        model: Some("llama3".to_string()),
        input_tokens: 1_000,
        output_tokens: 500,
        category: "orchestrate".to_string(),
        notes: None,
    })
    .expect("log token");

    let trend = cost_trend(7).expect("cost_trend");
    assert_eq!(trend.len(), 7);
    let today = &trend[trend.len() - 1];
    assert_eq!(today.total_tokens, 1_500);
    // Dates are ascending (oldest-first).
    assert!(trend.windows(2).all(|w| w[0].date <= w[1].date));
}

// --- N7-C: wave-based orchestrator execution --------------------------------

/// A diamond plan: analyze → {build, doc} → review. The two middle steps share
/// a wave (independent), exercising concurrent scheduling.
fn diamond_plan(project: &str, task_id: &str, provider: &str) -> OrchestrationPlan {
    let mk = |id: &str, deps: &[&str]| SubAgentTask {
        id: id.to_string(),
        title: id.to_string(),
        kind: TaskKind::Plan,
        agent: provider.to_string(),
        provider: provider.to_string(),
        executor_kind: ExecutorKind::LocalRuntime,
        executor_id: provider.to_string(),
        provider_id: Some(provider.to_string()),
        model: None,
        thinking: ThinkingLevel::None,
        instruction: format!("do {id}"),
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        verify_command: None,
        budget_tokens: 500,
        allow_write: false,
    };
    OrchestrationPlan {
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "diamond".to_string(),
        steps: vec![
            mk("analyze", &[]),
            mk("build", &["analyze"]),
            mk("doc", &["analyze"]),
            mk("review", &["build", "doc"]),
        ],
    }
}

/// A single step that routes to a paid completion provider (no dependencies),
/// so the paid-approval gate is exercised in isolation.
fn paid_provider_plan(project: &str, task_id: &str) -> OrchestrationPlan {
    OrchestrationPlan {
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "paid step".to_string(),
        steps: vec![SubAgentTask {
            id: "analyze".to_string(),
            title: "Analyze".to_string(),
            kind: TaskKind::Plan,
            agent: "openai_api".to_string(),
            provider: "openai_api".to_string(),
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: "openai_api".to_string(),
            provider_id: Some("openai_api".to_string()),
            model: Some("gpt-4o-mini".to_string()),
            thinking: ThinkingLevel::None,
            instruction: "Outline an approach.".to_string(),
            depends_on: Vec::new(),
            verify_command: None,
            budget_tokens: 500,
            allow_write: false,
        }],
    }
}

/// A coding-agent step carrying a `verify_command`, used to prove the verify
/// path is routed through the validated check runner (never raw `sh -c`).
fn coding_agent_plan_with_verify(
    project: &str,
    task_id: &str,
    verify_command: &str,
) -> OrchestrationPlan {
    let mut plan = coding_agent_plan(project, task_id);
    plan.steps[0].verify_command = Some(verify_command.to_string());
    plan
}

fn coding_agent_plan(project: &str, task_id: &str) -> OrchestrationPlan {
    OrchestrationPlan {
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "coding agent handoff".to_string(),
        steps: vec![SubAgentTask {
            id: "implement".to_string(),
            title: "Implement the change".to_string(),
            kind: TaskKind::Patch,
            agent: "codex_cli".to_string(),
            provider: "codex_cli".to_string(),
            executor_kind: ExecutorKind::CodingAgent,
            executor_id: "codex_cli".to_string(),
            provider_id: None,
            model: None,
            thinking: ThinkingLevel::None,
            instruction: "Prepare a bounded patch.".to_string(),
            depends_on: Vec::new(),
            verify_command: None,
            budget_tokens: 500,
            allow_write: true,
        }],
    }
}

#[tokio::test]
#[serial]
async fn dry_run_executes_waves_in_deterministic_index_order() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    let plan = diamond_plan("demo", &active_id, "chatgpt");
    let opts = RunOptions {
        dry_run: true,
        max_cost: None,
        settings: ProviderSettings::default(),
        ..RunOptions::default()
    };
    let run = run_plan(&plan, &opts).await.expect("run_plan");

    assert_eq!(run.status, RunStatus::DryRun);
    // Results are recorded in plan/index order regardless of wave grouping.
    let ids: Vec<&str> = run.results.iter().map(|r| r.task_id.as_str()).collect();
    assert_eq!(ids, vec!["analyze", "build", "doc", "review"]);
    // Dry run previews every step as Ok (no provider calls made).
    assert!(run.results.iter().all(|r| r.status == SubAgentStatus::Ok));
}

#[tokio::test]
#[serial]
async fn dry_run_cost_ceiling_blocks_steps_deterministically() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    let plan = diamond_plan("demo", &active_id, "chatgpt");
    // Per-step projected cost is identical, so a ceiling just above one step's
    // cost admits exactly the first step and blocks the rest — independent of
    // wave concurrency.
    let one_step_cost = run_plan(
        &plan,
        &RunOptions {
            dry_run: true,
            max_cost: None,
            settings: ProviderSettings::default(),
            ..RunOptions::default()
        },
    )
    .await
    .expect("baseline")
    .results[0]
        .cost_units;
    assert!(
        one_step_cost > 0.0,
        "chatgpt should project a non-zero cost"
    );

    let run = run_plan(
        &plan,
        &RunOptions {
            dry_run: true,
            max_cost: Some(one_step_cost * 1.5),
            settings: ProviderSettings::default(),
            ..RunOptions::default()
        },
    )
    .await
    .expect("ceiling run");

    // First step admitted; once the ceiling trips, everything after is stopped.
    assert_eq!(run.results[0].task_id, "analyze");
    assert_eq!(run.results[0].status, SubAgentStatus::Ok);
    assert!(
        run.results
            .iter()
            .skip(1)
            .all(|r| r.status == SubAgentStatus::Blocked || r.status == SubAgentStatus::Skipped),
        "downstream steps must be blocked/skipped once the ceiling trips"
    );
}

#[tokio::test]
#[serial]
async fn real_run_previews_coding_agent_without_provider_call() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;
    let plan = coding_agent_plan("demo", &active_id);

    let run = run_plan(
        &plan,
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            ..RunOptions::default()
        },
    )
    .await
    .expect("run_plan");

    assert_eq!(run.results.len(), 1);
    let result = &run.results[0];
    assert_eq!(result.status, SubAgentStatus::Skipped);
    assert_eq!(result.agent, "codex_cli");
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("command preview: codex exec --sandbox workspace-write --color never - [stdin: bounded prompt]")),
        "handoff notes should include the safe argv preview: {:?}",
        result.notes
    );
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("explicit orchestrator approval"))
    );
}

#[tokio::test]
#[serial]
#[cfg(unix)]
async fn approved_coding_agent_runs_through_argv_executor() {
    let fx = setup();
    init_git_repo(&fx.project_path);
    let active_id = show_active_task().expect("active").config.id;
    let bin_dir = TempDir::new().expect("bin tempdir");
    let fake_codex = bin_dir.path().join("codex");
    std::fs::write(
        &fake_codex,
        "#!/bin/sh\ncat >/dev/null\necho agent-output\n",
    )
    .expect("write fake codex");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake codex");

    let old_path = std::env::var_os("PATH");
    let new_path = match old_path.as_ref() {
        Some(path) => {
            let mut paths = vec![bin_dir.path().to_path_buf()];
            paths.extend(std::env::split_paths(path));
            std::env::join_paths(paths).expect("join path")
        }
        None => bin_dir.path().as_os_str().to_os_string(),
    };
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    let run_result = run_plan(
        &coding_agent_plan("demo", &active_id),
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            authorization: ExecutionAuthorization {
                allow_paid_providers: false,
                allow_coding_agents: true,
                allow_workspace_writes: true,
            },
            coding_agent_timeout_secs: 5,
            ..RunOptions::default()
        },
    )
    .await;

    unsafe {
        match old_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }

    let run = run_result.expect("run_plan");

    assert_eq!(run.results.len(), 1);
    let result = &run.results[0];
    assert_eq!(result.status, SubAgentStatus::Ok);
    assert!(result.output.contains("agent-output"));
    assert!(
        result.workspace.is_some(),
        "approved coding-agent runs should use an isolated workspace"
    );
    assert!(
        result.notes.iter().any(|note| note.contains("stdout:")),
        "execution notes should include stdout receipt path: {:?}",
        result.notes
    );
}

#[tokio::test]
#[serial]
#[cfg(unix)]
async fn approved_coding_agent_blocks_when_isolated_workspace_cannot_be_created() {
    let fx = setup();
    let active_id = show_active_task().expect("active").config.id;
    let bin_dir = TempDir::new().expect("bin tempdir");
    let marker = bin_dir.path().join("launched");
    let fake_codex = bin_dir.path().join("codex");
    std::fs::write(
        &fake_codex,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho launched > {}\necho agent-output\n",
            marker.display()
        ),
    )
    .expect("write fake codex");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake codex");

    let old_path = std::env::var_os("PATH");
    let new_path = match old_path.as_ref() {
        Some(path) => {
            let mut paths = vec![bin_dir.path().to_path_buf()];
            paths.extend(std::env::split_paths(path));
            std::env::join_paths(paths).expect("join path")
        }
        None => bin_dir.path().as_os_str().to_os_string(),
    };
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    let run_result = run_plan(
        &coding_agent_plan("demo", &active_id),
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            authorization: ExecutionAuthorization {
                allow_paid_providers: false,
                allow_coding_agents: true,
                allow_workspace_writes: true,
            },
            coding_agent_timeout_secs: 5,
            ..RunOptions::default()
        },
    )
    .await;

    unsafe {
        match old_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }

    let run = run_result.expect("run_plan");
    let result = &run.results[0];
    assert_eq!(result.status, SubAgentStatus::Blocked);
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("isolated workspace is required")),
        "expected fail-closed workspace note: {:?}",
        result.notes
    );
    assert!(
        !marker.exists(),
        "coding-agent process must not launch when isolated workspace creation fails"
    );
    assert!(
        !fx.project_path.join("launched").exists(),
        "active checkout must remain untouched"
    );
}

#[tokio::test]
#[serial]
async fn real_run_skips_paid_provider_without_approval() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;
    let plan = paid_provider_plan("demo", &active_id);

    // Default authorization denies paid spend: the step must be skipped, not
    // called, and no cost may be charged.
    let run = run_plan(
        &plan,
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            ..RunOptions::default()
        },
    )
    .await
    .expect("run_plan");

    assert_eq!(run.results.len(), 1);
    let result = &run.results[0];
    assert_eq!(result.status, SubAgentStatus::Skipped);
    assert_eq!(result.cost_units, 0.0);
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("paid provider execution requires explicit approval")),
        "expected paid-approval skip note: {:?}",
        result.notes
    );
}

#[tokio::test]
#[serial]
async fn approved_paid_provider_passes_the_gate() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;
    let plan = paid_provider_plan("demo", &active_id);

    // With paid approval the gate is cleared, so the step proceeds to the
    // provider call. No API key is configured, so it fails at the provider —
    // crucially *not* skipped with the approval note.
    let run = run_plan(
        &plan,
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            authorization: ExecutionAuthorization {
                allow_paid_providers: true,
                allow_coding_agents: false,
                allow_workspace_writes: false,
            },
            ..RunOptions::default()
        },
    )
    .await
    .expect("run_plan");

    let result = &run.results[0];
    assert_ne!(result.status, SubAgentStatus::Skipped);
    assert!(
        !result
            .notes
            .iter()
            .any(|note| note.contains("paid provider execution requires explicit approval")),
        "approved paid step must not carry the approval-skip note: {:?}",
        result.notes
    );
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("provider unavailable")),
        "expected an unavailable-provider failure (no key configured): {:?}",
        result.notes
    );
}

#[tokio::test]
#[serial]
#[cfg(unix)]
async fn coding_agent_verify_command_is_validated_not_shelled() {
    let fx = setup();
    init_git_repo(&fx.project_path);
    let active_id = show_active_task().expect("active").config.id;
    let bin_dir = TempDir::new().expect("bin tempdir");
    let fake_codex = bin_dir.path().join("codex");
    std::fs::write(
        &fake_codex,
        "#!/bin/sh\ncat >/dev/null\necho agent-output\n",
    )
    .expect("write fake codex");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake codex");

    let old_path = std::env::var_os("PATH");
    let new_path = match old_path.as_ref() {
        Some(path) => {
            let mut paths = vec![bin_dir.path().to_path_buf()];
            paths.extend(std::env::split_paths(path));
            std::env::join_paths(paths).expect("join path")
        }
        None => bin_dir.path().as_os_str().to_os_string(),
    };
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    // A verify command with a shell metacharacter and a chained `rm -rf /` must
    // be rejected by the validated check runner before any process spawns — it
    // can never reach a raw `sh -c`.
    let run_result = run_plan(
        &coding_agent_plan_with_verify("demo", &active_id, "cargo test; rm -rf /"),
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            authorization: ExecutionAuthorization {
                allow_paid_providers: false,
                allow_coding_agents: true,
                allow_workspace_writes: true,
            },
            coding_agent_timeout_secs: 5,
            ..RunOptions::default()
        },
    )
    .await;

    unsafe {
        match old_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }

    let run = run_result.expect("run_plan");
    let result = &run.results[0];
    // Agent succeeded, but verification failed validation → overall Failed.
    assert_eq!(result.status, SubAgentStatus::Failed);
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("verify failed") && note.contains("Validation Error")),
        "expected a verify-validation failure note: {:?}",
        result.notes
    );
}

#[tokio::test]
#[serial]
#[cfg(unix)]
async fn write_capable_coding_agent_blocks_without_workspace_write_authorization() {
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;
    let bin_dir = TempDir::new().expect("bin tempdir");
    let marker = bin_dir.path().join("launched");
    let fake_codex = bin_dir.path().join("codex");
    std::fs::write(
        &fake_codex,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho launched > {}\necho agent-output\n",
            marker.display()
        ),
    )
    .expect("write fake codex");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake codex");

    let old_path = std::env::var_os("PATH");
    let new_path = match old_path.as_ref() {
        Some(path) => {
            let mut paths = vec![bin_dir.path().to_path_buf()];
            paths.extend(std::env::split_paths(path));
            std::env::join_paths(paths).expect("join path")
        }
        None => bin_dir.path().as_os_str().to_os_string(),
    };
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    // The coding agent is approved, but workspace writes are NOT. The implement
    // step is write-capable, so it must be blocked before any process launch.
    let run_result = run_plan(
        &coding_agent_plan("demo", &active_id),
        &RunOptions {
            dry_run: false,
            max_cost: None,
            settings: ProviderSettings::default(),
            authorization: ExecutionAuthorization {
                allow_paid_providers: false,
                allow_coding_agents: true,
                allow_workspace_writes: false,
            },
            coding_agent_timeout_secs: 5,
            ..RunOptions::default()
        },
    )
    .await;

    unsafe {
        match old_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }

    let run = run_result.expect("run_plan");
    let result = &run.results[0];
    assert_eq!(result.status, SubAgentStatus::Blocked);
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("workspace writes are not authorized")),
        "expected a workspace-write authorization block: {:?}",
        result.notes
    );
    assert!(
        !marker.exists(),
        "coding-agent process must not launch when workspace writes are unauthorized"
    );
}

// --- P8: git file diff -------------------------------------------------------

#[test]
#[serial]
fn file_diff_returns_unstaged_changes_and_rejects_traversal() {
    use repodesk_core::git_workspace::file_diff;
    let fx = setup();

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
    std::fs::write(fx.project_path.join("a.txt"), "line one\n").expect("write");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
    std::fs::write(fx.project_path.join("a.txt"), "line one\nline two\n").expect("modify");

    let diff = file_diff(&fx.project_path, "a.txt", false);
    assert!(
        diff.contains("+line two"),
        "diff should show the added line:\n{diff}"
    );
    assert!(diff.contains("a.txt"), "diff header should name the file");

    // Path traversal / absolute paths never produce a diff.
    assert_eq!(file_diff(&fx.project_path, "../escape", false), "");
    assert_eq!(file_diff(&fx.project_path, "/etc/passwd", false), "");
    assert_eq!(file_diff(&fx.project_path, "", false), "");
}

// --- N8-A: orchestrator outcome ledger (the Hermes learning signal) ----------

/// A plan + run pair with a mix of outcomes: one clean step, one failure, one
/// skipped step — exercising all three auto-verdicts.
fn mixed_run(project: &str, task_id: &str) -> (OrchestrationPlan, OrchestrationRun) {
    let step = |id: &str, kind: TaskKind| SubAgentTask {
        id: id.to_string(),
        title: id.to_string(),
        kind,
        agent: "ollama".to_string(),
        provider: "ollama".to_string(),
        executor_kind: ExecutorKind::LocalRuntime,
        executor_id: "ollama".to_string(),
        provider_id: Some("ollama".to_string()),
        model: Some("llama3".to_string()),
        thinking: ThinkingLevel::None,
        instruction: format!("do {id}"),
        depends_on: Vec::new(),
        verify_command: None,
        budget_tokens: 500,
        allow_write: false,
    };
    let plan = OrchestrationPlan {
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "mixed".to_string(),
        steps: vec![
            step("analyze", TaskKind::Plan),
            step("implement", TaskKind::Patch),
            step("review", TaskKind::Review),
        ],
    };
    let result = |id: &str, status: SubAgentStatus, cost: f64| SubAgentResult {
        task_id: id.to_string(),
        agent: "ollama".to_string(),
        provider: "ollama".to_string(),
        model: "llama3".to_string(),
        status,
        output: String::new(),
        input_tokens: 10,
        output_tokens: 5,
        cost_units: cost,
        captured_proposals: 0,
        changed_files: Vec::new(),
        diff_path: None,
        workspace: None,
        notes: Vec::new(),
    };
    let run = OrchestrationRun {
        run_id: "run-20260619-120000".to_string(),
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "mixed".to_string(),
        status: RunStatus::Partial,
        dry_run: false,
        started_at: "2026-06-19T12:00:00Z".to_string(),
        finished_at: "2026-06-19T12:01:00Z".to_string(),
        results: vec![
            result("analyze", SubAgentStatus::Ok, 0.2),
            result("implement", SubAgentStatus::Failed, 0.0),
            result("review", SubAgentStatus::Skipped, 0.0),
        ],
        total_input_tokens: 30,
        total_output_tokens: 15,
        total_cost_units: 0.2,
    };
    (plan, run)
}

#[test]
#[serial]
fn record_run_writes_one_outcome_per_step_with_auto_verdicts() {
    use repodesk_core::outcomes::{self, Verdict};
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    let (plan, run) = mixed_run("demo", &active_id);
    let written = outcomes::record_run(&plan, &run).expect("record_run");
    assert_eq!(written, 3);

    let rows = outcomes::list_outcomes(10).expect("list_outcomes");
    assert_eq!(rows.len(), 3);
    // All start provisional (auto, unconfirmed).
    assert!(
        rows.iter()
            .all(|r| r.verdict_source == "auto" && !r.confirmed)
    );

    let verdict_of = |step: &str| {
        rows.iter()
            .find(|r| r.step_id == step)
            .unwrap_or_else(|| panic!("missing step {step}"))
            .verdict
    };
    assert_eq!(verdict_of("analyze"), Verdict::Good);
    assert_eq!(verdict_of("implement"), Verdict::Bad);
    assert_eq!(verdict_of("review"), Verdict::Neutral);

    // The patch step carries its TaskKind from the plan, not the run.
    let implement = rows.iter().find(|r| r.step_id == "implement").unwrap();
    assert_eq!(implement.task_kind, "patch");
}

#[test]
#[serial]
fn outcome_stats_aggregate_success_rate_per_kind_and_provider() {
    use repodesk_core::outcomes;
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    let (plan, run) = mixed_run("demo", &active_id);
    outcomes::record_run(&plan, &run).expect("record_run");

    let stats = outcomes::outcome_stats("demo").expect("stats");
    let stat_for = |kind: &str| {
        stats
            .iter()
            .find(|s| s.task_kind == kind && s.provider == "ollama")
            .unwrap_or_else(|| panic!("missing stat for {kind}"))
    };

    // plan/ollama: one good, no bad → 100% success.
    assert_eq!(stat_for("plan").success_rate, Some(1.0));
    // patch/ollama: one bad, no good → 0% success.
    assert_eq!(stat_for("patch").success_rate, Some(0.0));
    // review/ollama: only a neutral row → no scored signal yet.
    let review = stat_for("review");
    assert_eq!(review.success_rate, None);
    assert_eq!(review.neutral, 1);
}

#[test]
#[serial]
fn confirm_outcome_flips_verdict_to_human_and_dry_runs_are_not_recorded() {
    use repodesk_core::outcomes::{self, Verdict};
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    let (plan, mut run) = mixed_run("demo", &active_id);

    // A dry run carries no learning signal — nothing is recorded.
    run.dry_run = true;
    assert_eq!(outcomes::record_run(&plan, &run).expect("dry"), 0);
    assert!(outcomes::list_outcomes(10).expect("list").is_empty());

    // The real run records, then a human overrides the failed step to "good".
    run.dry_run = false;
    outcomes::record_run(&plan, &run).expect("record");
    let implement_id = outcomes::list_outcomes(10)
        .expect("list")
        .into_iter()
        .find(|r| r.step_id == "implement")
        .expect("implement row")
        .id;

    outcomes::confirm_outcome(implement_id, Verdict::Good).expect("confirm");

    let updated = outcomes::list_outcomes(10)
        .expect("list")
        .into_iter()
        .find(|r| r.id == implement_id)
        .expect("row still present");
    assert_eq!(updated.verdict, Verdict::Good);
    assert_eq!(updated.verdict_source, "human");
    assert!(updated.confirmed);

    // Confirming an unknown id is an error, not a silent no-op.
    assert!(outcomes::confirm_outcome(999_999, Verdict::Bad).is_err());
}

// --- N8-B: learned routing bias from the outcome ledger ----------------------

/// Record a run of `count` plan-kind steps on `provider`, all with `status`.
fn record_plan_steps(project: &str, task_id: &str, provider: &str, count: usize, ok: bool) {
    use repodesk_core::outcomes;
    let steps: Vec<SubAgentTask> = (0..count)
        .map(|i| SubAgentTask {
            id: format!("step-{i}"),
            title: format!("step {i}"),
            kind: TaskKind::Plan,
            agent: provider.to_string(),
            provider: provider.to_string(),
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: provider.to_string(),
            provider_id: Some(provider.to_string()),
            model: Some("m".to_string()),
            thinking: ThinkingLevel::None,
            instruction: String::new(),
            depends_on: Vec::new(),
            verify_command: None,
            budget_tokens: 100,
            allow_write: false,
        })
        .collect();
    let results: Vec<SubAgentResult> = steps
        .iter()
        .map(|s| SubAgentResult {
            task_id: s.id.clone(),
            agent: provider.to_string(),
            provider: provider.to_string(),
            model: "m".to_string(),
            status: if ok {
                SubAgentStatus::Ok
            } else {
                SubAgentStatus::Failed
            },
            output: String::new(),
            input_tokens: 1,
            output_tokens: 1,
            cost_units: 0.1,
            captured_proposals: 0,
            changed_files: Vec::new(),
            diff_path: None,
            workspace: None,
            notes: Vec::new(),
        })
        .collect();
    let plan = OrchestrationPlan {
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "g".to_string(),
        steps,
    };
    let run = OrchestrationRun {
        run_id: format!("run-2026{provider}{count}{ok}"),
        project: project.to_string(),
        task_id: task_id.to_string(),
        goal: "g".to_string(),
        status: RunStatus::Completed,
        dry_run: false,
        started_at: "2026-06-19T12:00:00Z".to_string(),
        finished_at: "2026-06-19T12:01:00Z".to_string(),
        results,
        total_input_tokens: count,
        total_output_tokens: count,
        total_cost_units: 0.1 * count as f64,
    };
    outcomes::record_run(&plan, &run).expect("record_run");
}

#[test]
#[serial]
fn routing_bias_rewards_success_punishes_failure_and_needs_enough_signal() {
    use repodesk_core::outcomes;
    use repodesk_core::routing::types::TaskKind as TK;
    let _fx = setup();
    let active_id = show_active_task().expect("active").config.id;

    // 4 successful plan steps on ollama → above the min-weight threshold, so a
    // positive nudge is learned.
    record_plan_steps("demo", &active_id, "ollama", 4, true);
    // 4 failed plan steps on chatgpt → a negative nudge.
    record_plan_steps("demo", &active_id, "chatgpt", 4, false);
    // Only 1 plan step on gemini → below threshold, no entry.
    record_plan_steps("demo", &active_id, "gemini", 1, true);

    let bias = outcomes::routing_bias("demo").expect("routing_bias");

    let ollama = bias.lookup(TK::Plan, "ollama").expect("ollama entry");
    assert!(
        ollama.adjustment > 0,
        "all-success should nudge positively, got {}",
        ollama.adjustment
    );
    let chatgpt = bias.lookup(TK::Plan, "chatgpt").expect("chatgpt entry");
    assert!(
        chatgpt.adjustment < 0,
        "all-failure should nudge negatively, got {}",
        chatgpt.adjustment
    );
    // Below the signal threshold: no learned entry yet.
    assert!(bias.lookup(TK::Plan, "gemini").is_none());
    // A pair with no data at all is also absent.
    assert!(bias.lookup(TK::Patch, "ollama").is_none());
}

// --- N8-C: bounded autonomous loop -------------------------------------------

#[tokio::test]
#[serial]
async fn loop_dry_run_is_a_single_preview_pass() {
    use repodesk_core::orchestrator::{LoopOptions, LoopStatus, run_loop};
    let _fx = setup();

    let opts = LoopOptions {
        max_iterations: 3,
        dry_run: true,
        settings: ProviderSettings::default(),
        ..LoopOptions::default()
    };
    let loop_run = run_loop(Some("ship it".to_string()), &opts)
        .await
        .expect("run_loop");

    // A dry run never loops: one preview pass, terminal DryRun, and no real
    // spend. The reported cost may be a projection when a paid/CLI route is
    // available on this machine.
    assert_eq!(loop_run.status, LoopStatus::DryRun);
    assert_eq!(loop_run.iterations.len(), 1);
    assert!(loop_run.total_cost_units.is_finite());
    assert!(loop_run.total_cost_units >= 0.0);
    assert_eq!(loop_run.goal, "ship it");
}

#[tokio::test]
#[serial]
async fn loop_pauses_before_paid_spend_without_approval() {
    use repodesk_core::orchestrator::{LoopOptions, LoopStatus, run_loop};
    let _fx = setup();

    // A configured paid key makes the patch step route to a paid provider, so
    // the plan has a paid step. Without approval the loop must refuse to spend.
    let mut settings = ProviderSettings::default();
    settings.openai.api_key = Some("sk-test".to_string());

    let opts = LoopOptions {
        max_iterations: 3,
        dry_run: false,
        approve_paid: false,
        settings,
        ..LoopOptions::default()
    };
    let loop_run = run_loop(None, &opts).await.expect("run_loop");

    // Stopped on the first attempt, before any provider call, with no cost.
    assert_eq!(loop_run.status, LoopStatus::NeedsApproval);
    assert_eq!(loop_run.iterations.len(), 1);
    assert!(loop_run.iterations[0].run_id.is_empty());
    assert_eq!(loop_run.total_cost_units, 0.0);
}
