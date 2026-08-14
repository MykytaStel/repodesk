use std::path::{Component, Path};

use crate::projects;

use super::process::{run_git_captured_bounded, truncate_with_marker};

const MAX_FILE_DIFF_BYTES: usize = 200 * 1024;
const MAX_DIFF_STAT_BYTES: usize = 64 * 1024;
const MAX_GIT_LINES_BYTES: usize = 512 * 1024;
const DIFF_TRUNCATION_MARKER: &str = "\n[diff truncated]";
const STAT_TRUNCATION_MARKER: &str = "\n[diff stat truncated]";
const LINES_TRUNCATION_MARKER: &str = "\n[git lines truncated]";

pub(super) struct DiffStatCapture {
    pub(super) text: String,
    pub(super) success: bool,
    pub(super) truncated: bool,
}

pub(super) fn capture_diff_stat(project_path: &Path, cached: bool) -> DiffStatCapture {
    let capture = if cached {
        run_git_captured_bounded(
            project_path,
            &["diff", "--cached", "--stat"],
            MAX_DIFF_STAT_BYTES,
        )
    } else {
        run_git_captured_bounded(project_path, &["diff", "--stat"], MAX_DIFF_STAT_BYTES)
    };
    DiffStatCapture {
        text: truncate_with_marker(
            &capture.text,
            MAX_DIFF_STAT_BYTES,
            capture.truncated,
            STAT_TRUNCATION_MARKER,
        ),
        success: capture.success,
        truncated: capture.truncated,
    }
}

/// The unified diff for a single file in the working tree. `cached` selects the
/// staged diff; otherwise the unstaged diff. `file` must be a repo-relative path
/// (it comes from the changed-files list) — absolute or traversal (`..`) paths
/// are rejected so a diff can never reach outside the project.
pub fn file_diff(project_path: &Path, file: &str, cached: bool) -> String {
    let trimmed = file.trim();
    if !is_safe_repo_relative_path(trimmed) {
        return String::new();
    }

    let mut args = vec!["diff", "--no-color"];
    if cached {
        args.push("--cached");
    }
    args.push("--");
    args.push(trimmed);

    let capture = run_git_captured_bounded(project_path, &args, MAX_FILE_DIFF_BYTES);
    truncate_with_marker(
        &capture.text,
        MAX_FILE_DIFF_BYTES,
        capture.truncated,
        DIFF_TRUNCATION_MARKER,
    )
}

/// The unified diff for a file in the *active project*. Returns an empty string
/// when there is no active project or no diff for the path.
pub fn active_file_diff(file: &str, cached: bool) -> String {
    match projects::get_active_project() {
        Ok(project) => file_diff(project.path.as_path(), file, cached),
        Err(_) => String::new(),
    }
}

/// Compatibility projection for line-oriented Git commands. Retained output is
/// capped so commands such as large logs cannot grow desktop memory without
/// bound. Callers that need typed data should prefer a domain-specific reader.
pub fn git_lines(project_path: &Path, args: &[&str]) -> Vec<String> {
    let capture = run_git_captured_bounded(project_path, args, MAX_GIT_LINES_BYTES);
    let output = truncate_with_marker(
        &capture.text,
        MAX_GIT_LINES_BYTES,
        capture.truncated,
        LINES_TRUNCATION_MARKER,
    );
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn is_safe_repo_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let path = Path::new(path);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absolute_and_traversal_diff_paths() {
        assert!(!is_safe_repo_relative_path(""));
        assert!(!is_safe_repo_relative_path("../secret"));
        assert!(!is_safe_repo_relative_path("nested/../../secret"));
        assert_eq!(
            is_safe_repo_relative_path("src/features/code/CodeTab.tsx"),
            !Path::new("src/features/code/CodeTab.tsx").is_absolute()
        );
    }
}
