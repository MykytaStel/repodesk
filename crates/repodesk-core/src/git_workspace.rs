use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

mod diff;
mod process;
mod snapshot;
mod status;

pub use diff::{active_file_diff, file_diff, git_lines};
pub use process::run_git_captured;
pub use snapshot::{build_git_workspace_snapshot, build_git_workspace_snapshot_for_path};
pub(crate) use process::run_git_captured_bounded;
pub(crate) use status::{GitStatusSnapshot, read_git_status};

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
    /// Bounded human-readable compatibility projection of `changed_files`.
    /// `changed_files` is the canonical workspace status representation.
    pub raw_status: String,
    pub warnings: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

/// Legacy parser kept for callers/tests that already hold line-oriented
/// porcelain v1 text. New Git reads use the NUL-delimited streaming parser in
/// `git_workspace::status` so filenames cannot create line ambiguity.
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

pub(super) fn status_label(x: char, y: char) -> &'static str {
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
