use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::brain::{format_brain_status, read_brain_status};
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::workflow_doctor::{diagnose_workflow, DoctorLevel};

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub project: UiProject,
    pub task: UiTask,
    pub brain: UiBrain,
    pub files: UiFiles,
    pub safe_routing: UiSafeRouting,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiProject {
    pub name: String,
    pub path: String,
    pub project_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub run_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiBrain {
    pub level: String,
    pub summary: String,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiFiles {
    pub context_md: bool,
    pub prompt_codex_md: bool,
    pub prompt_chatgpt_md: bool,
    pub prompt_review_md: bool,
    pub checks_summary_md: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSafeRouting {
    pub codex: bool,
    pub chatgpt: bool,
}

pub fn build_ui_snapshot() -> RepoDeskResult<UiSnapshot> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let doctor = diagnose_workflow()?;
    let brain = read_brain_status()?;

    let run_dir = &task.config.run_dir;

    Ok(UiSnapshot {
        project: UiProject {
            name: project.name,
            path: project.path.display().to_string(),
            project_type: project.project_type,
        },
        task: UiTask {
            id: task.config.id,
            title: task.config.title,
            status: format!("{:?}", task.config.status),
            run_dir: run_dir.display().to_string(),
        },
        brain: UiBrain {
            level: doctor.level.as_label().to_string(),
            summary: format_brain_status(&brain),
            next_actions: doctor.next_actions,
        },
        files: UiFiles {
            context_md: run_dir.join("context.md").exists(),
            prompt_codex_md: run_dir.join("prompt.codex.md").exists(),
            prompt_chatgpt_md: run_dir.join("prompt.chatgpt.md").exists(),
            prompt_review_md: run_dir.join("prompt.review.md").exists(),
            checks_summary_md: run_dir.join("checks-summary.md").exists(),
        },
        safe_routing: UiSafeRouting {
            codex: doctor.safe_for_codex,
            chatgpt: doctor.safe_for_chatgpt,
        },
    })
}

pub fn write_ui_snapshot() -> RepoDeskResult<PathBuf> {
    let snapshot = build_ui_snapshot()?;
    let task = show_active_task()?;
    let file = task.config.run_dir.join("ui-snapshot.json");
    let json = serde_json::to_string_pretty(&snapshot)?;
    fs::write(&file, json)?;
    Ok(file)
}

pub fn read_ui_snapshot_json() -> RepoDeskResult<String> {
    let snapshot = build_ui_snapshot()?;
    Ok(serde_json::to_string_pretty(&snapshot)?)
}

pub fn ui_level_label(level: &DoctorLevel) -> &'static str {
    level.as_label()
}

pub fn ui_routes_text() -> String {
    r#"UI routes planned for Tauri desktop:

- /dashboard
  Brain status, active project, active task, warnings.

- /projects
  Registered projects, active project switcher.

- /task
  Current task, context pack, prompt files.

- /tokens
  Token estimate, breakdown, ledger, budget warnings.

- /agents
  AI adapters, capabilities, guard preflight.

- /checks
  Check runner, latest summary, log-safe output.

- /security
  Security policy, blocked paths, risk explanation.
"#
    .to_string()
}
