use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::checks::{CheckCommandResult, is_allowed_check_command, run_validated_check};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::{ProjectConfig, get_active_project};

pub const TASK_RUNNER_MAX_TASKS: usize = 64;
pub const TASK_RUNNER_TIMEOUT_SECS: u64 = 120;
pub const TASK_RUNNER_MAX_OUTPUT_CHARS: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectTaskKind {
    Format,
    Lint,
    Typecheck,
    Test,
    Security,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTask {
    pub id: String,
    pub label: String,
    pub command: String,
    pub kind: ProjectTaskKind,
    pub runnable: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunnerSnapshot {
    pub project: String,
    pub tasks: Vec<ProjectTask>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunStatus {
    Passed,
    Failed,
    Timeout,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunResult {
    pub project: String,
    pub task_id: String,
    pub label: String,
    pub kind: ProjectTaskKind,
    pub command: String,
    pub status: TaskRunStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunBatch {
    pub project: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub success: bool,
    pub passed: usize,
    pub failed: usize,
    pub timed_out: usize,
    pub blocked: usize,
    pub results: Vec<TaskRunResult>,
}

pub fn active_task_runner_snapshot() -> RepoDeskResult<TaskRunnerSnapshot> {
    let project = get_active_project()?;
    Ok(snapshot_for_project(&project))
}

pub fn run_active_task(task_id: &str) -> RepoDeskResult<TaskRunResult> {
    let project = get_active_project()?;
    let snapshot = snapshot_for_project(&project);
    let task = snapshot
        .tasks
        .iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| {
            RepoDeskError::InvalidCheckCommand(format!(
                "Unknown or stale project task '{task_id}'. Refresh Tasks before running it."
            ))
        })?;

    Ok(run_task(&project, task))
}

pub fn run_all_active_tasks() -> RepoDeskResult<TaskRunBatch> {
    let project = get_active_project()?;
    let snapshot = snapshot_for_project(&project);
    let started_at = Utc::now();
    let results = snapshot
        .tasks
        .iter()
        .map(|task| run_task(&project, task))
        .collect::<Vec<_>>();
    let finished_at = Utc::now();

    let passed = results
        .iter()
        .filter(|result| result.status == TaskRunStatus::Passed)
        .count();
    let failed = results
        .iter()
        .filter(|result| result.status == TaskRunStatus::Failed)
        .count();
    let timed_out = results
        .iter()
        .filter(|result| result.status == TaskRunStatus::Timeout)
        .count();
    let blocked = results
        .iter()
        .filter(|result| result.status == TaskRunStatus::Blocked)
        .count();

    Ok(TaskRunBatch {
        project: project.name,
        started_at,
        finished_at,
        success: !results.is_empty() && passed == results.len(),
        passed,
        failed,
        timed_out,
        blocked,
        results,
    })
}

fn snapshot_for_project(project: &ProjectConfig) -> TaskRunnerSnapshot {
    let tasks = project
        .checks
        .iter()
        .take(TASK_RUNNER_MAX_TASKS)
        .enumerate()
        .map(|(index, command)| {
            let validation_error = is_allowed_check_command(command).err();
            ProjectTask {
                id: task_id(index, command),
                label: task_label(command),
                command: command.clone(),
                kind: task_kind(command),
                runnable: validation_error.is_none(),
                validation_error,
            }
        })
        .collect::<Vec<_>>();

    TaskRunnerSnapshot {
        project: project.name.clone(),
        tasks,
        truncated: project.checks.len() > TASK_RUNNER_MAX_TASKS,
    }
}

fn run_task(project: &ProjectConfig, task: &ProjectTask) -> TaskRunResult {
    let started_at = Utc::now();

    if let Some(reason) = &task.validation_error {
        let finished_at = Utc::now();
        return TaskRunResult {
            project: project.name.clone(),
            task_id: task.id.clone(),
            label: task.label.clone(),
            kind: task.kind,
            command: task.command.clone(),
            status: TaskRunStatus::Blocked,
            exit_code: None,
            duration_ms: 0,
            started_at,
            finished_at,
            stdout: String::new(),
            stderr: reason.clone(),
            stdout_truncated: false,
            stderr_truncated: false,
        };
    }

    let command_result = run_validated_check(&task.command, &project.path, TASK_RUNNER_TIMEOUT_SECS);
    let finished_at = Utc::now();
    from_check_result(project, task, started_at, finished_at, command_result)
}

fn from_check_result(
    project: &ProjectConfig,
    task: &ProjectTask,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    result: CheckCommandResult,
) -> TaskRunResult {
    let status = match result.status.as_str() {
        "passed" => TaskRunStatus::Passed,
        "timeout" => TaskRunStatus::Timeout,
        _ => TaskRunStatus::Failed,
    };
    let (stdout, stdout_truncated) = bounded_tail(&result.stdout, TASK_RUNNER_MAX_OUTPUT_CHARS);
    let (stderr, stderr_truncated) = bounded_tail(&result.stderr, TASK_RUNNER_MAX_OUTPUT_CHARS);

    TaskRunResult {
        project: project.name.clone(),
        task_id: task.id.clone(),
        label: task.label.clone(),
        kind: task.kind,
        command: task.command.clone(),
        status,
        exit_code: result.exit_code,
        duration_ms: u64::try_from(result.duration_ms).unwrap_or(u64::MAX),
        started_at,
        finished_at,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    }
}

fn bounded_tail(value: &str, max_chars: usize) -> (String, bool) {
    let count = value.chars().count();
    if count <= max_chars {
        return (value.to_string(), false);
    }

    let start = count.saturating_sub(max_chars);
    let tail = value.chars().skip(start).collect::<String>();
    (
        format!("[RepoDesk omitted {} earlier characters]\n{}", start, tail),
        true,
    )
}

fn task_id(index: usize, command: &str) -> String {
    // Stable FNV-1a is enough here: this is a stale-UI identity guard, not a
    // cryptographic integrity primitive. Including the index preserves unique
    // IDs even if a manually-edited project config contains duplicate checks.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in command.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("check-{index}-{hash:016x}")
}

fn task_kind(command: &str) -> ProjectTaskKind {
    let lower = command.to_ascii_lowercase();
    if lower.contains("fmt") || lower.contains("prettier") || lower.contains("black") {
        ProjectTaskKind::Format
    } else if lower.contains("clippy") || lower.contains("eslint") || lower.contains("flake8") {
        ProjectTaskKind::Lint
    } else if lower.contains("typecheck") || lower.contains("mypy") || lower.contains("tsc") {
        ProjectTaskKind::Typecheck
    } else if lower.contains("test") || lower.contains("pytest") || lower.contains("jest") || lower.contains("vitest") {
        ProjectTaskKind::Test
    } else if lower.contains("snyk")
        || lower.contains("trivy")
        || lower.contains("sonar")
        || lower.contains("checkmarx")
    {
        ProjectTaskKind::Security
    } else {
        ProjectTaskKind::Check
    }
}

fn task_label(command: &str) -> String {
    match task_kind(command) {
        ProjectTaskKind::Format => "Format check".to_string(),
        ProjectTaskKind::Lint => "Lint".to_string(),
        ProjectTaskKind::Typecheck => "Typecheck".to_string(),
        ProjectTaskKind::Test => "Tests".to_string(),
        ProjectTaskKind::Security => "Security scan".to_string(),
        ProjectTaskKind::Check => command
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_project_tasks() {
        assert_eq!(task_kind("cargo fmt --all -- --check"), ProjectTaskKind::Format);
        assert_eq!(task_kind("cargo clippy --all-targets"), ProjectTaskKind::Lint);
        assert_eq!(task_kind("pnpm typecheck"), ProjectTaskKind::Typecheck);
        assert_eq!(task_kind("cargo test --all"), ProjectTaskKind::Test);
        assert_eq!(task_kind("trivy fs ."), ProjectTaskKind::Security);
    }

    #[test]
    fn task_identity_changes_with_command_or_position() {
        let first = task_id(0, "cargo test --all");
        assert_eq!(first, task_id(0, "cargo test --all"));
        assert_ne!(first, task_id(1, "cargo test --all"));
        assert_ne!(first, task_id(0, "cargo test -p repodesk-core"));
    }

    #[test]
    fn bounded_tail_keeps_recent_diagnostics() {
        let (value, truncated) = bounded_tail("abcdefgh", 4);
        assert!(truncated);
        assert!(value.ends_with("efgh"));
        assert!(value.contains("omitted 4 earlier characters"));
    }
}
