use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub id: String,
    pub project_name: String,
    pub title: String,
    pub status: TaskStatus,
    pub run_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone)]
pub struct NewTaskInput {
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub config: TaskConfig,
    pub task_file: PathBuf,
    pub task_markdown_file: PathBuf,
}

/// A task in the active project's run history, flagged if it is the active one.
/// Mirrors `projects::ProjectInfo` so the desktop can render a task switcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub config: TaskConfig,
    pub is_active: bool,
}

pub fn create_task(input: NewTaskInput) -> RepoDeskResult<TaskInfo> {
    init::init_home()?;

    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;

    let title = input.title.trim();

    let now = Utc::now();
    let id = build_task_id(&now, title);

    let run_dir = paths.runs_dir.join(&project.name).join(&id);
    fs::create_dir_all(&run_dir)?;

    let config = TaskConfig {
        id: id.clone(),
        project_name: project.name.clone(),
        title: title.to_string(),
        status: TaskStatus::Open,
        run_dir: run_dir.clone(),
        created_at: now,
        updated_at: now,
    };

    let task_file = run_dir.join("task.toml");
    let task_markdown_file = run_dir.join("task.md");

    write_task_config(&task_file, &config)?;
    write_task_markdown(&task_markdown_file, &config)?;

    let active_task_file = active_task_file_for_project(&paths, &project.name);
    fs::write(active_task_file, format!("{id}\n"))?;

    Ok(TaskInfo {
        config,
        task_file,
        task_markdown_file,
    })
}

pub fn show_active_task() -> RepoDeskResult<TaskInfo> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;

    let active_task_file = active_task_file_for_project(&paths, &project.name);

    if !active_task_file.exists() {
        return Err(RepoDeskError::ActiveTaskNotSet(project.name));
    }

    let task_id = fs::read_to_string(active_task_file)?.trim().to_string();

    if task_id.is_empty() {
        return Err(RepoDeskError::ActiveTaskNotSet(project.name));
    }

    let run_dir = paths.runs_dir.join(&project.name).join(&task_id);
    let task_file = run_dir.join("task.toml");
    let task_markdown_file = run_dir.join("task.md");

    if !task_file.exists() {
        return Err(RepoDeskError::TaskNotFound(task_id));
    }

    let config = read_task_config(&task_file)?;

    Ok(TaskInfo {
        config,
        task_file,
        task_markdown_file,
    })
}

pub fn task_status() -> RepoDeskResult<TaskInfo> {
    show_active_task()
}

/// List every task recorded for the active project, newest first, flagging the
/// active one. Dirs without a readable `task.toml` are skipped (not fatal) so a
/// half-written or foreign directory never breaks the switcher.
pub fn list_tasks() -> RepoDeskResult<Vec<TaskSummary>> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;

    let active_id = read_active_task_id(&paths, &project.name);
    let project_runs = paths.runs_dir.join(&project.name);

    let mut tasks = Vec::new();
    if project_runs.exists() {
        for entry in fs::read_dir(&project_runs)? {
            let entry = entry?;
            let task_file = entry.path().join("task.toml");
            if !task_file.exists() {
                continue;
            }
            if let Ok(config) = read_task_config(&task_file) {
                let is_active = active_id.as_deref() == Some(config.id.as_str());
                tasks.push(TaskSummary { config, is_active });
            }
        }
    }

    tasks.sort_by(|a, b| b.config.created_at.cmp(&a.config.created_at));
    Ok(tasks)
}

/// Switch the active task of the active project to `task_id`. The id is
/// validated and confirmed to exist before the pointer is rewritten, so a bad
/// id never leaves the project without an active task.
pub fn use_task(task_id: &str) -> RepoDeskResult<TaskInfo> {
    let task_id = task_id.trim();
    validate_task_id(task_id)?;

    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;

    let run_dir = paths.runs_dir.join(&project.name).join(task_id);
    let task_file = run_dir.join("task.toml");
    if !task_file.exists() {
        return Err(RepoDeskError::TaskNotFound(task_id.to_string()));
    }

    let config = read_task_config(&task_file)?;
    let task_markdown_file = run_dir.join("task.md");

    let active_task_file = active_task_file_for_project(&paths, &project.name);
    fs::write(active_task_file, format!("{task_id}\n"))?;

    Ok(TaskInfo {
        config,
        task_file,
        task_markdown_file,
    })
}

/// Read the active task id for a project, if one is set and non-empty.
fn read_active_task_id(paths: &RepoDeskPaths, project_name: &str) -> Option<String> {
    let active_task_file = active_task_file_for_project(paths, project_name);
    let id = fs::read_to_string(active_task_file)
        .ok()?
        .trim()
        .to_string();
    if id.is_empty() { None } else { Some(id) }
}

/// Task ids are generated as `<timestamp>[-<slug>]` (ascii alphanumerics and
/// dashes). Reject anything else so a caller-supplied id can never escape the
/// project's run directory via separators or `..`.
fn validate_task_id(task_id: &str) -> RepoDeskResult<()> {
    let valid = !task_id.is_empty()
        && task_id != "."
        && task_id != ".."
        && task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(RepoDeskError::TaskNotFound(task_id.to_string()))
    }
}

fn active_task_file_for_project(paths: &RepoDeskPaths, project_name: &str) -> PathBuf {
    paths.project_dir(project_name).join("active_task")
}

fn build_task_id(now: &DateTime<Utc>, title: &str) -> String {
    let timestamp = now.format("%Y-%m-%d-%H%M%S").to_string();
    let slug = slugify(title);

    if slug.is_empty() {
        timestamp
    } else {
        format!("{timestamp}-{slug}")
    }
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').chars().take(48).collect()
}

fn write_task_config(path: &Path, config: &TaskConfig) -> RepoDeskResult<()> {
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

fn read_task_config(path: &Path) -> RepoDeskResult<TaskConfig> {
    let content = fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}

fn write_task_markdown(path: &Path, config: &TaskConfig) -> RepoDeskResult<()> {
    let content = format!(
        r#"# Task

{title}

## Goal

Describe the goal of this task.

## Scope

- Add files or folders that are allowed to change.

## Do not change

- Add files, areas, or behavior that must stay untouched.

## Acceptance criteria

- Add concrete checks that prove the task is complete.

## Notes

Project: `{project}`
Task id: `{id}`
Status: `open`
"#,
        title = config.title,
        project = config.project_name,
        id = config.id,
    );

    fs::write(path, content)?;
    Ok(())
}
