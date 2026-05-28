use std::fs;
use std::path::PathBuf;

use crate::errors::RepoDeskResult;
use crate::guard::preflight;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::estimate_file;
use crate::usage::budget::{evaluate_context, load_budget_config};

#[derive(Debug, Clone)]
pub struct BrainStatus {
    pub project_name: String,
    pub project_path: PathBuf,
    pub task_id: String,
    pub task_title: String,
    pub run_dir: PathBuf,
    pub context_state: ArtifactState,
    pub prompt_state: PromptState,
    pub checks_state: ArtifactState,
    pub codex_preflight: String,
    pub chatgpt_preflight: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ArtifactState {
    pub exists: bool,
    pub path: PathBuf,
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct PromptState {
    pub codex: ArtifactState,
    pub chatgpt: ArtifactState,
    pub review: ArtifactState,
}

pub fn read_brain_status() -> RepoDeskResult<BrainStatus> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let budget = load_budget_config()?;

    let context_file = task.config.run_dir.join("context.md");
    let context_state = if context_file.exists() {
        let estimate = estimate_file(&context_file)?;
        let verdict = evaluate_context(&estimate, &budget);

        ArtifactState {
            exists: true,
            path: context_file.clone(),
            details: format!(
                "{} tokens, budget {}",
                estimate.estimated_tokens,
                verdict.level.as_label()
            ),
        }
    } else {
        ArtifactState {
            exists: false,
            path: context_file.clone(),
            details: "missing; run `repodesk context build`".to_string(),
        }
    };

    let checks_summary = task.config.run_dir.join("checks-summary.md");
    let checks_state = if checks_summary.exists() {
        ArtifactState {
            exists: true,
            path: checks_summary.clone(),
            details: summarize_checks_state(&checks_summary),
        }
    } else {
        ArtifactState {
            exists: false,
            path: checks_summary.clone(),
            details: "missing; run `repodesk checks run`".to_string(),
        }
    };

    let prompt_state = PromptState {
        codex: artifact(task.config.run_dir.join("prompt.codex.md"), "codex prompt"),
        chatgpt: artifact(
            task.config.run_dir.join("prompt.chatgpt.md"),
            "chatgpt prompt",
        ),
        review: artifact(
            task.config.run_dir.join("prompt.review.md"),
            "review prompt",
        ),
    };

    let codex_preflight = preflight("codex")
        .map(|result| result.level.as_label().to_string())
        .unwrap_or_else(|_| "UNKNOWN".to_string());

    let chatgpt_preflight = preflight("chatgpt")
        .map(|result| result.level.as_label().to_string())
        .unwrap_or_else(|_| "UNKNOWN".to_string());

    let mut recommendations = Vec::new();

    if !context_state.exists {
        recommendations.push("Build context before using any agent.".to_string());
    }

    if !prompt_state.codex.exists || !prompt_state.chatgpt.exists || !prompt_state.review.exists {
        recommendations.push("Generate prompts with `repodesk prompt all`.".to_string());
    }

    if !checks_state.exists {
        recommendations.push(
            "Run checks once and give agents only checks-summary.md, not full logs.".to_string(),
        );
    }

    if codex_preflight == "BLOCK" || chatgpt_preflight == "BLOCK" {
        recommendations.push(
            "A preflight guard blocks at least one agent. Fix that before continuing.".to_string(),
        );
    }

    if recommendations.is_empty() {
        recommendations
            .push("Brain state is healthy. Continue with bounded agent workflow.".to_string());
    }

    Ok(BrainStatus {
        project_name: project.name,
        project_path: project.path,
        task_id: task.config.id,
        task_title: task.config.title,
        run_dir: task.config.run_dir,
        context_state,
        prompt_state,
        checks_state,
        codex_preflight,
        chatgpt_preflight,
        recommendations,
    })
}

pub fn format_brain_status(status: &BrainStatus) -> String {
    format!(
        r#"RepoDesk Brain Status

Project:
  name: {}
  path: {}

Active task:
  id: {}
  title: {}
  run dir: {}

Artifacts:
  context: {} ({})
  checks summary: {} ({})
  prompt.codex.md: {} ({})
  prompt.chatgpt.md: {} ({})
  prompt.review.md: {} ({})

Preflight:
  codex: {}
  chatgpt: {}

Recommendations:
{}
"#,
        status.project_name,
        status.project_path.display(),
        status.task_id,
        status.task_title,
        status.run_dir.display(),
        yes_no(status.context_state.exists),
        status.context_state.details,
        yes_no(status.checks_state.exists),
        status.checks_state.details,
        yes_no(status.prompt_state.codex.exists),
        status.prompt_state.codex.details,
        yes_no(status.prompt_state.chatgpt.exists),
        status.prompt_state.chatgpt.details,
        yes_no(status.prompt_state.review.exists),
        status.prompt_state.review.details,
        status.codex_preflight,
        status.chatgpt_preflight,
        format_list(&status.recommendations)
    )
}

fn artifact(path: PathBuf, label: &str) -> ArtifactState {
    if path.exists() {
        ArtifactState {
            exists: true,
            path,
            details: format!("{label} ready"),
        }
    } else {
        ArtifactState {
            exists: false,
            path,
            details: format!("{label} missing"),
        }
    }
}

fn summarize_checks_state(path: &PathBuf) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();
    let lowered = content.to_ascii_lowercase();

    if lowered.contains("overall status: `passed`") {
        "last checks passed".to_string()
    } else if lowered.contains("overall status: `failed`") || lowered.contains("status: failed") {
        "last checks failed".to_string()
    } else {
        "summary exists".to_string()
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}
