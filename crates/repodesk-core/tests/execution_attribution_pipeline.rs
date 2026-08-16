#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::change_attribution::ChangeAttributionStrength;
use repodesk_core::orchestrator::{
    ExecutionAuthorization, OrchestrationPlan, RunOptions, SubAgentStatus, SubAgentTask, run_plan,
};
use repodesk_core::projects::{AddProjectInput, add_project, use_project};
use repodesk_core::routing::types::{ExecutorKind, TaskKind};
use repodesk_core::tasks::{NewTaskInput, create_task, show_active_task};
use serial_test::serial;
use tempfile::TempDir;

fn git_ok(path: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let home = TempDir::new().expect("tempdir");
    unsafe {
        std::env::set_var("REPODESK_HOME", home.path());
    }
    repodesk_core::init::init_home().expect("init home");

    let project = home.path().join("project");
    std::fs::create_dir_all(&project).expect("project dir");
    add_project(AddProjectInput {
        name: "demo".into(),
        path: project.clone(),
        project_type: "generic".into(),
        main_language: None,
    })
    .expect("add project");
    use_project("demo").expect("use project");
    create_task(NewTaskInput {
        title: "attribution".into(),
        verify_command: None,
    })
    .expect("create task");

    git_ok(&project, &["init", "-q"]);
    git_ok(&project, &["config", "user.email", "t@example.com"]);
    git_ok(&project, &["config", "user.name", "Test"]);
    std::fs::write(project.join("seed.txt"), "seed\n").expect("seed");
    git_ok(&project, &["add", "."]);
    git_ok(&project, &["commit", "-qm", "init"]);
    (home, project)
}

fn coding_agent_plan(task_id: &str) -> OrchestrationPlan {
    OrchestrationPlan {
        project: "demo".into(),
        task_id: task_id.into(),
        goal: "write one tracked file".into(),
        steps: vec![SubAgentTask {
            id: "implement".into(),
            title: "Implement".into(),
            kind: TaskKind::Patch,
            agent: "codex_cli".into(),
            provider: "codex_cli".into(),
            executor_kind: ExecutorKind::CodingAgent,
            executor_id: "codex_cli".into(),
            provider_id: None,
            model: None,
            thinking: Default::default(),
            instruction: "edit seed.txt".into(),
            depends_on: Vec::new(),
            budget_tokens: 500,
            allow_write: true,
            verify_command: None,
        }],
    }
}

#[tokio::test]
#[serial]
async fn isolated_coding_agent_result_carries_exact_attribution_evidence() {
    let (_home, _project) = setup();
    let task_id = show_active_task().expect("task").config.id;
    let bin_dir = TempDir::new().expect("bin dir");
    let fake_codex = bin_dir.path().join("codex");
    std::fs::write(
        &fake_codex,
        "#!/bin/sh\ncat >/dev/null\nprintf 'agent\\n' >> seed.txt\necho done\n",
    )
    .expect("fake codex");
    std::fs::set_permissions(&fake_codex, std::fs::Permissions::from_mode(0o755))
        .expect("chmod");

    let old_path = std::env::var_os("PATH");
    let mut paths = vec![bin_dir.path().to_path_buf()];
    if let Some(path) = old_path.as_ref() {
        paths.extend(std::env::split_paths(path));
    }
    unsafe {
        std::env::set_var("PATH", std::env::join_paths(paths).expect("join path"));
    }

    let result = run_plan(
        &coding_agent_plan(&task_id),
        &RunOptions {
            authorization: ExecutionAuthorization {
                allow_coding_agents: true,
                allow_workspace_writes: true,
                allow_paid_providers: false,
            },
            coding_agent_timeout_secs: 5,
            settings: ProviderSettings::default(),
            ..RunOptions::default()
        },
    )
    .await;

    unsafe {
        match old_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }

    let run = result.expect("run");
    let step = run.results.first().expect("result");
    assert_eq!(step.status, SubAgentStatus::Ok);
    assert_eq!(
        step.change_attribution.strength,
        ChangeAttributionStrength::ExactIsolated
    );
    let workspace = step.workspace.as_ref().expect("managed worktree");
    assert_eq!(
        step.change_attribution.workspace_id.as_deref(),
        Some(workspace.workspace_id.as_str())
    );
    assert_eq!(
        step.change_attribution.baseline_commit.as_deref(),
        Some(workspace.base_commit.as_str())
    );
    assert!(
        !step
            .change_attribution
            .reason
            .as_deref()
            .unwrap_or("")
            .contains(&workspace.path)
    );
}
