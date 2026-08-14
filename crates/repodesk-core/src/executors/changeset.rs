use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::git_workspace::{self, GitFileChange};

/// Maximum diff size kept on an executor run / written to a receipt.
const MAX_DIFF_BYTES: usize = 200 * 1024;
const DIFF_TRUNCATION_MARKER: &str = "\n[diff truncated]";

/// The working-tree changeset a run produced.
pub(super) struct Changeset {
    pub(super) changed_files: Vec<GitFileChange>,
    pub(super) diff: String,
    pub(super) diff_truncated: bool,
    pub(super) diff_path: Option<String>,
}

impl Changeset {
    fn empty() -> Self {
        Self {
            changed_files: Vec::new(),
            diff: String::new(),
            diff_truncated: false,
            diff_path: None,
        }
    }
}

/// Compare the post-run git status to the pre-run snapshot, capture the unified
/// diff of the tracked changes, and write it to a `.diff` receipt. Returns an
/// empty changeset when `cwd` is not a git repo or nothing changed.
pub(super) fn capture_changeset(
    cwd: &Path,
    output_dir: &Path,
    safe_id: &str,
    stamp: u128,
    pre_status: Option<&str>,
) -> Changeset {
    let Some(pre) = pre_status else {
        return Changeset::empty();
    };
    let post = git_workspace::run_git_captured(cwd, &["status", "--porcelain=v1"]);
    let changed_files = changed_since(pre, &post);
    if changed_files.is_empty() {
        return Changeset::empty();
    }

    // Staged + unstaged tracked diffs; neither needs an existing HEAD, so this
    // works in a brand-new repo too. Untracked files are listed in
    // `changed_files` but their content is not inlined here.
    //
    // The staged read receives only the budget left by the unstaged read. Both
    // commands still drain to EOF, but retained diff memory stays O(MAX_DIFF_BYTES)
    // rather than retaining two independent full-size captures.
    let raw_capture =
        git_workspace::run_git_captured_bounded(cwd, &["diff", "--no-color"], MAX_DIFF_BYTES);
    let needs_separator = !raw_capture.text.trim().is_empty();
    let used = raw_capture.text.len().min(MAX_DIFF_BYTES);
    let cached_budget = MAX_DIFF_BYTES
        .saturating_sub(used)
        .saturating_sub(usize::from(needs_separator));
    let cached_capture = git_workspace::run_git_captured_bounded(
        cwd,
        &["diff", "--cached", "--no-color"],
        cached_budget,
    );

    let mut raw_diff = raw_capture.text;
    if !cached_capture.text.trim().is_empty() {
        if needs_separator {
            raw_diff.push(char::from(10));
        }
        raw_diff.push_str(&cached_capture.text);
    }

    let (diff, final_truncated) = truncate_to_bytes(&raw_diff, MAX_DIFF_BYTES);
    let diff_truncated = raw_capture.truncated || cached_capture.truncated || final_truncated;
    let diff_path = {
        let path = output_dir.join(format!("{safe_id}-{stamp}.diff"));
        match fs::write(&path, diff.as_bytes()) {
            Ok(()) => Some(path.display().to_string()),
            Err(_) => None,
        }
    };

    Changeset {
        changed_files,
        diff,
        diff_truncated,
        diff_path,
    }
}

/// Porcelain status of `cwd`, or `None` when it is not inside a git work tree.
pub(super) fn git_porcelain(cwd: &Path) -> Option<String> {
    let inside = git_workspace::run_git_captured(cwd, &["rev-parse", "--is-inside-work-tree"]);
    if inside.trim() != "true" {
        return None;
    }
    Some(git_workspace::run_git_captured(
        cwd,
        &["status", "--porcelain=v1"],
    ))
}

/// Porcelain lines present after the run but not before it — the files the run
/// added or whose status it changed.
fn changed_since(pre: &str, post: &str) -> Vec<GitFileChange> {
    let pre_lines: HashSet<&str> = pre.lines().collect();
    post.lines()
        .filter(|line| !pre_lines.contains(*line))
        .filter_map(git_workspace::parse_porcelain_line)
        .collect()
}

/// Truncate `text` to at most `max` bytes on a char boundary. When the complete
/// marker fits, it is included *inside* the byte budget rather than appended
/// after it. Returns `(text, was_truncated)`.
fn truncate_to_bytes(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_string(), false);
    }

    let marker_bytes = DIFF_TRUNCATION_MARKER.len();
    let content_budget = max.saturating_sub(marker_bytes);
    let mut end = content_budget;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    if max < marker_bytes {
        let mut fallback_end = max;
        while fallback_end > 0 && !text.is_char_boundary(fallback_end) {
            fallback_end -= 1;
        }
        return (text[..fallback_end].to_string(), true);
    }

    let mut output = String::with_capacity(max);
    output.push_str(&text[..end]);
    output.push_str(DIFF_TRUNCATION_MARKER);
    debug_assert!(output.len() <= max);
    (output, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_to_bytes_honors_the_hard_budget() {
        assert_eq!(
            truncate_to_bytes("short", 100),
            ("short".to_string(), false)
        );

        let (text, truncated) = truncate_to_bytes(&"x".repeat(100), 32);
        assert!(truncated);
        assert!(text.ends_with("[diff truncated]"));
        assert!(text.len() <= 32);
    }

    #[test]
    fn tiny_budget_still_never_overflows() {
        let (text, truncated) = truncate_to_bytes("abcdefgh", 4);
        assert!(truncated);
        assert_eq!(text, "abcd");
        assert_eq!(text.len(), 4);
    }

    #[test]
    fn changed_since_reports_only_new_status_lines() {
        let pre = " M seed.txt\n";
        let post = " M seed.txt\n M other.rs\n?? added.txt\n";
        let changed = changed_since(pre, post);
        let paths: Vec<&str> = changed.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, vec!["other.rs", "added.txt"]);
    }
}
