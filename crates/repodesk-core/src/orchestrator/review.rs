//! Human review of a coding-agent run's changeset: **accept** (stage the files
//! the agent changed, ready for a commit the human makes) or **reject** (discard
//! them — restore tracked files to HEAD, remove untracked ones).
//!
//! This is the actionable half of agent-run diff capture
//! ([`crate::executors`]): the run records *which* files it changed, and this
//! module lets the operator keep or drop exactly those files. It is bounded by
//! construction — it only ever touches paths the recorded run reported, never the
//! whole tree — and it never commits, pushes, or merges. RepoDesk stays the
//! control plane; the human stays the operator.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::runner::load_run;
use super::types::OrchestrationRun;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    /// Stage the agent-changed files (`git add`) so the human can commit them.
    Accept,
    /// Discard the agent's changes: restore tracked files to HEAD, remove
    /// untracked ones.
    Reject,
}

impl ReviewAction {
    pub fn from_label(value: &str) -> RepoDeskResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "accept" => Ok(Self::Accept),
            "reject" => Ok(Self::Reject),
            other => Err(RepoDeskError::RoutingFailed {
                detail: format!("unknown review action '{other}' (expected accept|reject)"),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedFile {
    pub path: String,
    /// What happened: `staged`, `restored`, `deleted`, or `skipped: <reason>`.
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReview {
    pub run_id: String,
    pub action: ReviewAction,
    pub project: String,
    pub processed: Vec<ReviewedFile>,
    pub warnings: Vec<String>,
}

/// Apply `action` to every file the persisted run `run_id` reported changing,
/// against the active project's working tree.
pub fn review_run(run_id: &str, action: ReviewAction) -> RepoDeskResult<RunReview> {
    let run = load_run(run_id)?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: format!("no orchestration run '{run_id}' for the active task"),
    })?;
    let project = crate::projects::get_active_project()?;
    let paths = collect_changed_files(&run);
    let isolated_workspaces = collect_isolated_workspaces(&run);

    let mut review = RunReview {
        run_id: run_id.to_string(),
        action,
        project: project.name.clone(),
        processed: Vec::new(),
        warnings: Vec::new(),
    };

    if paths.is_empty() {
        review
            .warnings
            .push("run recorded no changed files to review".to_string());
        return Ok(review);
    }
    if !isolated_workspaces.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "run '{run_id}' produced changes in isolated worktree(s); accept/reject apply-back is not implemented yet, so the active checkout was left untouched. Inspect manually: {}",
                isolated_workspaces.join(", ")
            ),
        });
    }
    if !is_git_repo(project.path.as_path()) {
        review
            .warnings
            .push("active project is not a git repository; nothing to review".to_string());
        return Ok(review);
    }

    for path in paths {
        review
            .processed
            .push(review_one(project.path.as_path(), &path, action));
    }
    Ok(review)
}

/// Distinct repo-relative paths the run's steps reported changing, in first-seen
/// order.
fn collect_changed_files(run: &OrchestrationRun) -> Vec<String> {
    let mut seen = Vec::new();
    for result in &run.results {
        for path in &result.changed_files {
            if !seen.iter().any(|existing| existing == path) {
                seen.push(path.clone());
            }
        }
    }
    seen
}

fn collect_isolated_workspaces(run: &OrchestrationRun) -> Vec<String> {
    let mut seen = Vec::new();
    for result in &run.results {
        if let Some(worktree) = &result.workspace {
            let label = format!(
                "{} ({})",
                worktree.path,
                worktree
                    .metadata_path
                    .as_deref()
                    .unwrap_or("metadata not recorded")
            );
            if !seen.iter().any(|existing| existing == &label) {
                seen.push(label);
            }
        }
    }
    seen
}

fn review_one(project_path: &Path, path: &str, action: ReviewAction) -> ReviewedFile {
    if !is_safe_relative(path) {
        return ReviewedFile {
            path: path.to_string(),
            outcome: "skipped: path is absolute or escapes the project".to_string(),
        };
    }

    match action {
        ReviewAction::Accept => {
            // Stage the change; the human commits. Works for tracked edits and
            // newly added files alike.
            if git_ok(project_path, &["add", "--", path]) {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "staged".to_string(),
                }
            } else {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "skipped: git add failed".to_string(),
                }
            }
        }
        ReviewAction::Reject => {
            if is_tracked(project_path, path) {
                // Reset both the index and working tree for this path to HEAD.
                if git_ok(
                    project_path,
                    &[
                        "restore",
                        "--source=HEAD",
                        "--staged",
                        "--worktree",
                        "--",
                        path,
                    ],
                ) {
                    ReviewedFile {
                        path: path.to_string(),
                        outcome: "restored".to_string(),
                    }
                } else {
                    ReviewedFile {
                        path: path.to_string(),
                        outcome: "skipped: git restore failed".to_string(),
                    }
                }
            } else if git_ok(project_path, &["clean", "-f", "--", path]) {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "deleted".to_string(),
                }
            } else {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "skipped: untracked cleanup failed".to_string(),
                }
            }
        }
    }
}

/// A path is safe to act on only if it is non-empty, relative, and contains no
/// `..` component — so review can never reach outside the project root.
fn is_safe_relative(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() || Path::new(trimmed).is_absolute() {
        return false;
    }
    !Path::new(trimmed)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn is_git_repo(project_path: &Path) -> bool {
    git_ok(project_path, &["rev-parse", "--is-inside-work-tree"])
}

fn is_tracked(project_path: &Path, path: &str) -> bool {
    git_ok(project_path, &["ls-files", "--error-unmatch", "--", path])
}

/// Run a git subcommand (argv-only, no shell) in `project_path`, returning
/// whether it exited successfully. Output is discarded; callers only need the
/// status. Paths are passed after `--` so they cannot be read as flags.
fn git_ok(project_path: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths() {
        assert!(!is_safe_relative(""));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("../outside.txt"));
        assert!(!is_safe_relative("src/../../escape.rs"));
        assert!(is_safe_relative("src/main.rs"));
        assert!(is_safe_relative("added.txt"));
    }

    #[test]
    fn action_parsing() {
        assert_eq!(
            ReviewAction::from_label("accept").unwrap(),
            ReviewAction::Accept
        );
        assert_eq!(
            ReviewAction::from_label("REJECT").unwrap(),
            ReviewAction::Reject
        );
        assert!(ReviewAction::from_label("merge").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reject_restores_tracked_and_deletes_untracked() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // git not installed
        }
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(git_ok(repo.path(), args), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "init"]);

        // Simulate an agent: modify the tracked file, add a new untracked one.
        std::fs::write(repo.path().join("seed.txt"), "seed\nchanged\n").unwrap();
        std::fs::write(repo.path().join("added.txt"), "new\n").unwrap();

        let restored = review_one(repo.path(), "seed.txt", ReviewAction::Reject);
        assert_eq!(restored.outcome, "restored");
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\n"
        );

        let deleted = review_one(repo.path(), "added.txt", ReviewAction::Reject);
        assert_eq!(deleted.outcome, "deleted");
        assert!(!repo.path().join("added.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn accept_stages_changed_file() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(git_ok(repo.path(), args), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("added.txt"), "new\n").unwrap();

        let staged = review_one(repo.path(), "added.txt", ReviewAction::Accept);
        assert_eq!(staged.outcome, "staged");
        // The file is now in the index.
        assert!(is_tracked(repo.path(), "added.txt"));
    }
}
