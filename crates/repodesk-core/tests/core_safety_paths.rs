//! Integration tests for the stateful safety/guard/security paths.
//!
//! These functions read the active project + task from `REPODESK_HOME`, so each
//! test runs against an isolated temporary home. `REPODESK_HOME` is a process-global
//! env var, so every test here is `#[serial]` to prevent cross-test interference.

use std::path::PathBuf;

use repodesk_core::api_clients::{ProviderSettings, ThinkingLevel};
use repodesk_core::guard::{GuardLevel, preflight};
use repodesk_core::judge::{JudgementDecision, judge_agent};
use repodesk_core::orchestrator::{
    OrchestrationPlan, OrchestrationRun, RunOptions, RunStatus, SubAgentResult, SubAgentStatus,
    SubAgentTask, list_runs, run_plan,
};
use repodesk_core::persistence::event_journal::{LogEventInput, log_event, read_task_events};
use repodesk_core::persistence::{count_action_runs, recent_action_runs, record_action_run};
use repodesk_core::projects::{AddProjectInput, add_project, use_project};
use repodesk_core::repopilot::{load_history, parse_review_json, record_report};
use repodesk_core::routing::types::TaskKind;
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
    })
    .expect("create_task");

    Fixture {
        _home: home,
        run_dir: task.config.run_dir,
        project_path,
    }
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
fn list_tasks_orders_newest_first_and_use_task_switches_active() {
    use repodesk_core::tasks::{list_tasks, use_task};

    let _fx = setup(); // creates "demo task", set active.

    // Second task becomes the newly active one.
    create_task(NewTaskInput {
        title: "second task".to_string(),
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
        model: None,
        thinking: ThinkingLevel::None,
        instruction: format!("do {id}"),
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
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
