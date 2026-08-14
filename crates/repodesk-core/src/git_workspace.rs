use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

use crate::projects;

mod process;
pub use process::run_git_captured;
pub(crate) use process::run_git_captured_bounded;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitFileChange {
    pub path: String,
    pub status_code: String,
    pub status_label: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub deleted: bool,
    pub renamed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorkspaceSnapshot {
    pub ok: bool,
    pub project_name: Option<String>,
    pub project_path: Option<String>,
    pub is_git_repo: bool,
    pub branch: Option<String>,
    pub last_commit: Option<String>,
    pub is_dirty: bool,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub changed_files: Vec<GitFileChange>,
    pub diff_stat: String,
    pub cached_diff_stat: String,
    pub raw_status: String,
    pub warnings: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

pub fn build_git_workspace_snapshot() -> GitWorkspaceSnapshot {
    let generated_at = Utc::now();

    let project = match projects::get_active_project() {
        Ok(project) => project,
        Err(error) => {
            return GitWorkspaceSnapshot {
                ok: false,
                project_name: None,
                project_path: None,
                is_git_repo: false,
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
                warnings: vec![format!("No active project configured: {error}")],
                generated_at,
            };
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
    let mut warnings = Vec::new();

    if !project_path.exists() {
        return GitWorkspaceSnapshot {
            ok: false,
            project_name: Some(project_name.to_string()),
            project_path: Some(project_path_display),
            is_git_repo: false,
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
            warnings: vec!["Project path does not exist on this machine.".to_string()],
            generated_at,
        };
    }

    let is_repo = run_git(project_path, &["rev-parse", "--is-inside-work-tree"])
        .map(|output| output.stdout.trim() == "true" && output.ok)
        .unwrap_or(false);

    if !is_repo {
        return GitWorkspaceSnapshot {
            ok: false,
            project_name: Some(project_name.to_string()),
            project_path: Some(project_path_display),
            is_git_repo: false,
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
            warnings: vec!["Active project is not inside a Git repository.".to_string()],
            generated_at,
        };
    }

    let branch = run_git(project_path, &["branch", "--show-current"])
        .ok()
        .and_then(|output| non_empty(output.stdout.trim()))
        .or_else(|| {
            run_git(project_path, &["rev-parse", "--short", "HEAD"])
                .ok()
                .and_then(|output| non_empty(output.stdout.trim()))
                .map(|head| format!("detached@{head}"))
        });

    let last_commit = run_git(project_path, &["log", "-1", "--oneline"])
        .ok()
        .and_then(|output| non_empty(output.stdout.trim()));

    let raw_status = run_git(project_path, &["status", "--porcelain=v1"])
        .map(|output| output.stdout)
        .unwrap_or_else(|error| format!("Failed to read git status: {error}"));

    let changed_files = parse_porcelain_status(&raw_status);
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

    let diff_stat = run_git(project_path, &["diff", "--stat"])
        .map(|output| output.stdout)
        .unwrap_or_default();
    let cached_diff_stat = run_git(project_path, &["diff", "--cached", "--stat"])
        .map(|output| output.stdout)
        .unwrap_or_default();

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

#[derive(Debug)]
struct GitOutput {
    ok: bool,
    stdout: String,
}

fn run_git(project_path: &Path, args: &[&str]) -> std::io::Result<GitOutput> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;

    Ok(GitOutput {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    })
}

/// The unified diff for a single file in the working tree. `cached` selects the
/// staged diff; otherwise the unstaged diff. `file` must be a repo-relative path
/// (it comes from the changed-files list) — absolute or traversal (`..`) paths
/// are rejected so a diff can never reach outside the project.
pub fn file_diff(project_path: &Path, file: &str, cached: bool) -> String {
    let trimmed = file.trim();
    if trimmed.is_empty()
        || Path::new(trimmed).is_absolute()
        || Path::new(trimmed)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return String::new();
    }
    let mut args = vec!["diff", "--no-color"];
    if cached {
        args.push("--cached");
    }
    args.push("--");
    args.push(trimmed);
    run_git_captured(project_path, &args)
}

/// The unified diff for a file in the *active project*. Returns an empty string
/// when there is no active project or no diff for the path.
pub fn active_file_diff(file: &str, cached: bool) -> String {
    match projects::get_active_project() {
        Ok(project) => file_diff(project.path.as_path(), file, cached),
        Err(_) => String::new(),
    }
}

pub fn git_lines(project_path: &Path, args: &[&str]) -> Vec<String> {
    let output = run_git_captured(project_path, args);
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn parse_porcelain_status(raw_status: &str) -> Vec<GitFileChange> {
    raw_status
        .lines()
        .filter_map(parse_porcelain_line)
        .collect()
}

pub fn parse_porcelain_line(line: &str) -> Option<GitFileChange> {
    if line.trim().is_empty() || line.len() < 3 {
        return None;
    }

    let mut chars = line.chars();
    let x = chars.next().unwrap_or(' ');
    let y = chars.next().unwrap_or(' ');
    let path = line.get(3..).unwrap_or_default().trim().to_string();

    if path.is_empty() {
        return None;
    }

    let untracked = x == '?' && y == '?';
    let staged = !untracked && x != ' ';
    let unstaged = !untracked && y != ' ';
    let deleted = x == 'D' || y == 'D';
    let renamed = x == 'R' || y == 'R';
    let status_code = format!("{x}{y}");

    Some(GitFileChange {
        path,
        status_label: status_label(x, y).to_string(),
        status_code,
        staged,
        unstaged,
        untracked,
        deleted,
        renamed,
    })
}

fn status_label(x: char, y: char) -> &'static str {
    match (x, y) {
        ('?', '?') => "untracked",
        ('M', ' ') => "staged modified",
        (' ', 'M') => "modified",
        ('M', 'M') => "staged + unstaged modified",
        ('A', ' ') => "staged added",
        (' ', 'D') => "deleted",
        ('D', ' ') => "staged deleted",
        ('R', _) => "renamed",
        ('C', _) => "copied",
        ('U', _) | (_, 'U') => "conflict",
        _ => "changed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_status_groups() {
        let raw = " M src/main.rs\nM  Cargo.toml\n?? notes.md\nR  old.rs -> new.rs\n";
        let changes = parse_porcelain_status(raw);
        assert_eq!(changes.len(), 4);
        assert!(changes[0].unstaged);
        assert!(changes[1].staged);
        assert!(changes[2].untracked);
        assert!(changes[3].renamed);
    }

    #[test]
    fn ignores_empty_lines() {
        assert!(parse_porcelain_status("\n\n").is_empty());
    }
}
