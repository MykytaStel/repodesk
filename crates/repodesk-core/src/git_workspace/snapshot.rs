use chrono::{DateTime, Utc};
use std::path::Path;

use crate::projects;

use super::GitWorkspaceSnapshot;
use super::diff::capture_diff_stat;
use super::process::{run_git_captured_bounded, truncate_with_marker};
use super::status::read_git_status;

const MAX_BRANCH_BYTES: usize = 512;
const MAX_COMMIT_BYTES: usize = 8 * 1024;
const MAX_RAW_STATUS_BYTES: usize = 32 * 1024;
const STATUS_TRUNCATION_MARKER: &str = "\n[status projection truncated]";

pub fn build_git_workspace_snapshot() -> GitWorkspaceSnapshot {
    let generated_at = Utc::now();

    let project = match projects::get_active_project() {
        Ok(project) => project,
        Err(error) => {
            return unavailable_snapshot(
                None,
                None,
                false,
                format!("No active project configured: {error}"),
                generated_at,
            );
        }
    };

    build_git_workspace_snapshot_for_path(&project.name, project.path.as_path(), generated_at)
}

pub fn build_git_workspace_snapshot_for_path(
    project_name: &str,
    project_path: &Path,
    generated_at: DateTime<Utc>,
) -> GitWorkspaceSnapshot {
    let project_path_display = project_path.display().to_string();
    if !project_path.exists() {
        return unavailable_snapshot(
            Some(project_name.to_string()),
            Some(project_path_display),
            false,
            "Project path does not exist on this machine.".to_string(),
            generated_at,
        );
    }

    let status = match read_git_status(project_path) {
        Ok(Some(status)) => status,
        Ok(None) => {
            return unavailable_snapshot(
                Some(project_name.to_string()),
                Some(project_path_display),
                false,
                "Active project is not inside a Git repository.".to_string(),
                generated_at,
            );
        }
        Err(error) => {
            return unavailable_snapshot(
                Some(project_name.to_string()),
                Some(project_path_display),
                true,
                format!("Failed to capture complete Git status: {error}"),
                generated_at,
            );
        }
    };

    let mut warnings = Vec::new();
    let changed_files = status.changes();
    let staged_count = changed_files.iter().filter(|item| item.staged).count();
    let unstaged_count = changed_files.iter().filter(|item| item.unstaged).count();
    let untracked_count = changed_files.iter().filter(|item| item.untracked).count();
    let is_dirty = !changed_files.is_empty();

    if is_dirty {
        warnings.push(format!(
            "Workspace has {} changed file(s). Review before running patch agents or committing.",
            changed_files.len()
        ));
    }
    if untracked_count > 0 {
        warnings.push(format!(
            "Workspace has {untracked_count} untracked file(s). Check whether generated files should be committed or ignored."
        ));
    }

    let branch = read_branch(project_path, &mut warnings);
    let last_commit = read_last_commit(project_path, &mut warnings);

    let diff_stat = read_diff_stat(project_path, false, &mut warnings);
    let cached_diff_stat = read_diff_stat(project_path, true, &mut warnings);

    let (raw_projection, raw_truncated) = status.diagnostic_porcelain(MAX_RAW_STATUS_BYTES);
    let raw_status = truncate_with_marker(
        &raw_projection,
        MAX_RAW_STATUS_BYTES,
        raw_truncated,
        STATUS_TRUNCATION_MARKER,
    );
    if raw_truncated {
        warnings.push(format!(
            "Git status diagnostic projection exceeded {MAX_RAW_STATUS_BYTES} bytes and was truncated; changed_files remains canonical."
        ));
    }

    GitWorkspaceSnapshot {
        ok: true,
        project_name: Some(project_name.to_string()),
        project_path: Some(project_path_display),
        is_git_repo: true,
        branch,
        last_commit,
        is_dirty,
        staged_count,
        unstaged_count,
        untracked_count,
        changed_files,
        diff_stat,
        cached_diff_stat,
        raw_status,
        warnings,
        generated_at,
    }
}

fn read_branch(project_path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    let branch = run_git_captured_bounded(
        project_path,
        &["branch", "--show-current"],
        MAX_BRANCH_BYTES,
    );
    if branch.success && !branch.truncated {
        if let Some(branch) = non_empty(branch.text.trim()) {
            return Some(branch);
        }
    } else if !branch.success {
        warnings.push("Failed to read the current Git branch.".to_string());
        return None;
    } else {
        warnings.push("Current Git branch name exceeded the metadata budget.".to_string());
        return None;
    }

    let head = run_git_captured_bounded(
        project_path,
        &["rev-parse", "--short", "HEAD"],
        MAX_BRANCH_BYTES,
    );
    if !head.success {
        warnings.push("Workspace is detached but HEAD could not be resolved.".to_string());
        return None;
    }
    if head.truncated {
        warnings.push("Detached HEAD metadata exceeded the configured budget.".to_string());
        return None;
    }
    non_empty(head.text.trim()).map(|head| format!("detached@{head}"))
}

fn read_last_commit(project_path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    let commit =
        run_git_captured_bounded(project_path, &["log", "-1", "--oneline"], MAX_COMMIT_BYTES);
    if commit.truncated {
        warnings.push("Last commit metadata exceeded the configured budget.".to_string());
        return None;
    }
    if !commit.success {
        // A repository with no commits is valid and should not look unhealthy.
        return None;
    }
    non_empty(commit.text.trim())
}

fn read_diff_stat(project_path: &Path, cached: bool, warnings: &mut Vec<String>) -> String {
    let capture = capture_diff_stat(project_path, cached);
    let label = if cached { "staged" } else { "unstaged" };
    if !capture.success {
        warnings.push(format!("Failed to capture {label} Git diff stat."));
        return String::new();
    }
    if capture.truncated {
        warnings.push(format!(
            "{label} Git diff stat was truncated to its hard budget."
        ));
    }
    capture.text
}

fn unavailable_snapshot(
    project_name: Option<String>,
    project_path: Option<String>,
    is_git_repo: bool,
    warning: String,
    generated_at: DateTime<Utc>,
) -> GitWorkspaceSnapshot {
    GitWorkspaceSnapshot {
        ok: false,
        project_name,
        project_path,
        is_git_repo,
        branch: None,
        last_commit: None,
        is_dirty: false,
        staged_count: 0,
        unstaged_count: 0,
        untracked_count: 0,
        changed_files: Vec::new(),
        diff_stat: String::new(),
        cached_diff_stat: String::new(),
        raw_status: String::new(),
        warnings: vec![warning],
        generated_at,
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
