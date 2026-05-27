use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::usage::budget::{evaluate_context, load_budget_config};
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::repo_map::build_repo_map;
use crate::tasks::show_active_task;
use crate::tokens::estimate_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSnapshot {
    pub project: String,
    pub task: String,
    pub context_exists: bool,
    pub smart_context_exists: bool,
    pub checks_summary_exists: bool,
    pub prompt_files_count: usize,
    pub repo_files_scanned: usize,
    pub repo_hotspots_count: usize,
    pub context_tokens: Option<usize>,
    pub budget_level: Option<String>,
    pub next_action: String,
}

pub async fn build_dashboard_snapshot() -> RepoDeskResult<DashboardSnapshot> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let repo_map = build_repo_map().await?;

    let context_file = task.config.run_dir.join("context.md");
    let smart_context_file = task.config.run_dir.join("smart-context.md");
    let checks_summary_file = task.config.run_dir.join("checks-summary.md");

    let prompt_files_count = count_existing(&[
        task.config.run_dir.join("prompt.codex.md"),
        task.config.run_dir.join("prompt.chatgpt.md"),
        task.config.run_dir.join("prompt.review.md"),
    ]);

    let context_exists = context_file.exists();
    let smart_context_exists = smart_context_file.exists();
    let checks_summary_exists = checks_summary_file.exists();

    let (context_tokens, budget_level) = if context_exists {
        let estimate = estimate_file(&context_file)?;
        let budget = load_budget_config()?;
        let verdict = evaluate_context(&estimate, &budget);
        (
            Some(estimate.estimated_tokens),
            Some(verdict.level.as_label().to_string()),
        )
    } else {
        (None, None)
    };

    let next_action = next_action(
        context_exists,
        smart_context_exists,
        prompt_files_count,
        checks_summary_exists,
    );

    Ok(DashboardSnapshot {
        project: project.name,
        task: task.config.title,
        context_exists,
        smart_context_exists,
        checks_summary_exists,
        prompt_files_count,
        repo_files_scanned: repo_map.files_scanned,
        repo_hotspots_count: repo_map.hotspots.len(),
        context_tokens,
        budget_level,
        next_action,
    })
}

pub async fn dashboard_json() -> RepoDeskResult<String> {
    let snapshot = build_dashboard_snapshot().await?;
    Ok(serde_json::to_string_pretty(&snapshot)?)
}

pub async fn dashboard_summary() -> RepoDeskResult<String> {
    let snapshot = build_dashboard_snapshot().await?;

    Ok(format!(
        r#"RepoDesk dashboard:

Project: {}
Task: {}

State:
  context.md: {}
  smart-context.md: {}
  checks-summary.md: {}
  prompt files: {}

Repository:
  files scanned: {}
  hotspots: {}

Budget:
  context tokens: {}
  budget level: {}

Next action:
  {}
"#,
        snapshot.project,
        snapshot.task,
        yes_no(snapshot.context_exists),
        yes_no(snapshot.smart_context_exists),
        yes_no(snapshot.checks_summary_exists),
        snapshot.prompt_files_count,
        snapshot.repo_files_scanned,
        snapshot.repo_hotspots_count,
        snapshot
            .context_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        snapshot
            .budget_level
            .unwrap_or_else(|| "unknown".to_string()),
        snapshot.next_action
    ))
}

fn count_existing(paths: &[std::path::PathBuf]) -> usize {
    paths.iter().filter(|path| Path::new(path).exists()).count()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn next_action(
    context_exists: bool,
    smart_context_exists: bool,
    prompt_files_count: usize,
    checks_summary_exists: bool,
) -> String {
    if !context_exists {
        "Run `repodesk context build`.".to_string()
    } else if !smart_context_exists {
        "Run `repodesk smart-context build` for a smaller paid-agent context.".to_string()
    } else if prompt_files_count < 3 {
        "Run `repodesk prompt all`.".to_string()
    } else if !checks_summary_exists {
        "Run `repodesk checks run` and `repodesk checks summarize`.".to_string()
    } else {
        "Run `repodesk judge agent --agent codex` before giving work to a patch agent.".to_string()
    }
}
