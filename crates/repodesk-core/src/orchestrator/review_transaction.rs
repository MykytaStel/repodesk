//! Transaction boundary for human Accept.
//!
//! The review engine contains the path-, symlink-, rename-, and isolated-worktree
//! apply logic. This module wraps that engine so an Accept is all-or-nothing from
//! the active checkout's point of view: the exact pre-Accept Git index is saved,
//! and any apply error or skipped path rolls the index back. Isolated-worktree
//! Accept also restores the touched working-tree paths because that flow copies
//! or applies bytes into the active checkout.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::review::{self, ReviewAction, RunReview};
use super::runner::load_run;
use super::types::OrchestrationRun;

#[derive(Debug)]
struct PathSnapshot {
    path: String,
    tracked_before: bool,
    existed_before: bool,
}

#[derive(Debug)]
struct AcceptTransaction {
    project_path: PathBuf,
    index_tree_before: String,
    paths: Vec<PathSnapshot>,
    restore_worktree: bool,
}

impl AcceptTransaction {
    fn begin(
        project_path: &Path,
        touched_paths: &[String],
        restore_worktree: bool,
    ) -> RepoDeskResult<Self> {
        let index_tree_before = git_capture(project_path, &["write-tree"])?;
        if index_tree_before.trim().is_empty() {
            return Err(routing_error(
                "accept blocked: could not snapshot the current Git index tree",
            ));
        }

        let mut paths = Vec::with_capacity(touched_paths.len());
        for path in touched_paths {
            paths.push(PathSnapshot {
                path: path.clone(),
                tracked_before: git_success(
                    project_path,
                    &["ls-files", "--error-unmatch", "--", path],
                ),
                existed_before: fs::symlink_metadata(project_path.join(path)).is_ok(),
            });
        }

        Ok(Self {
            project_path: project_path.to_path_buf(),
            index_tree_before,
            paths,
            restore_worktree,
        })
    }

    fn rollback(&self) -> RepoDeskResult<()> {
        // Restore the entire index object, not just the reviewed paths. This
        // preserves unrelated staged changes exactly as they were before Accept.
        git_run(
            &self.project_path,
            &["read-tree", self.index_tree_before.as_str()],
        )?;

        if self.restore_worktree {
            // Tracked paths are restored from the now-restored index. This is
            // intentionally path-bounded; never reset the whole worktree.
            for snapshot in self.paths.iter().filter(|path| path.tracked_before) {
                git_run(
                    &self.project_path,
                    &["checkout-index", "--force", "--", snapshot.path.as_str()],
                )?;
            }

            // Isolated Accept may copy previously absent files into the active
            // checkout. Remove only files proven absent before this transaction.
            for snapshot in self
                .paths
                .iter()
                .filter(|path| !path.tracked_before && !path.existed_before)
            {
                remove_transaction_created_path(&self.project_path, &snapshot.path)?;
            }
        }

        let restored_tree = git_capture(&self.project_path, &["write-tree"])?;
        if restored_tree != self.index_tree_before {
            return Err(routing_error(format!(
                "accept rollback did not restore the original index tree (expected {}, got {})",
                self.index_tree_before, restored_tree
            )));
        }
        Ok(())
    }
}

/// Apply a human review action. Reject keeps the existing bounded behavior.
/// Accept is wrapped in an explicit transaction so a later file failure cannot
/// leave an earlier subset staged/applied while Review remains open.
pub fn review_run(run_id: &str, action: ReviewAction) -> RepoDeskResult<RunReview> {
    if action == ReviewAction::Reject {
        return review::review_run(run_id, action);
    }

    let run = load_run(run_id)?.ok_or_else(|| {
        routing_error(format!(
            "no orchestration run '{run_id}' for the active task"
        ))
    })?;
    let project = crate::projects::get_active_project()?;
    let touched_paths = transaction_paths(&run);
    let restore_worktree = run
        .results
        .iter()
        .any(|result| result.workspace.is_some() && !result.changed_files.is_empty());
    let transaction =
        AcceptTransaction::begin(&project.path, &touched_paths, restore_worktree)?;

    match review::review_run(run_id, action) {
        Ok(review) => {
            let skipped = skipped_paths(&review);
            if skipped.is_empty() {
                Ok(review)
            } else {
                let primary = format!(
                    "accept blocked: {} of the run's files were not applied ({})",
                    skipped.len(),
                    skipped.join(", ")
                );
                rollback_error(transaction.rollback(), primary)
            }
        }
        Err(error) => {
            let primary = error.to_string();
            match transaction.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(routing_error(format!(
                    "accept failed ({primary}); rollback also failed ({rollback}). Review is blocked because the active checkout may require manual recovery"
                ))),
            }
        }
    }
}

fn rollback_error(rollback: RepoDeskResult<()>, primary: String) -> RepoDeskResult<RunReview> {
    match rollback {
        Ok(()) => Err(routing_error(primary)),
        Err(error) => Err(routing_error(format!(
            "{primary}; rollback also failed ({error}). Review is blocked because the active checkout may require manual recovery"
        ))),
    }
}

fn skipped_paths(review: &RunReview) -> Vec<String> {
    review
        .processed
        .iter()
        .filter(|file| file.outcome.starts_with("skipped"))
        .map(|file| file.path.clone())
        .collect()
}

fn transaction_paths(run: &OrchestrationRun) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for raw in run
        .results
        .iter()
        .flat_map(|result| result.changed_files.iter())
    {
        if let Some((old, new)) = split_rename(raw) {
            for path in [old, new] {
                if is_safe_relative(&path) && seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
            continue;
        }

        let path = unquote_path(raw);
        if is_safe_relative(&path) && seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    paths
}

fn split_rename(path: &str) -> Option<(String, String)> {
    let (old, new) = path.split_once(" -> ")?;
    Some((unquote_path(old), unquote_path(new)))
}

fn unquote_path(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .to_string()
}

fn is_safe_relative(path: &str) -> bool {
    if path.trim().is_empty() {
        return false;
    }
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn remove_transaction_created_path(project_path: &Path, relative: &str) -> RepoDeskResult<()> {
    assert_safe_parent_chain(project_path, relative)?;
    let target = project_path.join(relative);
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    if metadata.is_file() || metadata.file_type().is_symlink() {
        fs::remove_file(&target)?;
    } else {
        return Err(routing_error(format!(
            "accept rollback refused to remove '{}' because it is not a file or symlink",
            relative
        )));
    }

    cleanup_empty_parents(project_path, target.parent());
    Ok(())
}

fn assert_safe_parent_chain(project_path: &Path, relative: &str) -> RepoDeskResult<()> {
    if !is_safe_relative(relative) {
        return Err(routing_error(format!(
            "accept rollback refused unsafe path '{relative}'"
        )));
    }

    let mut cursor = project_path.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        let Component::Normal(part) = component else {
            continue;
        };
        cursor.push(part);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(routing_error(format!(
                    "accept rollback refused '{}' because a parent path is a symlink",
                    relative
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(routing_error(format!(
                    "accept rollback refused '{}' because a parent path is not a directory",
                    relative
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn cleanup_empty_parents(project_path: &Path, mut parent: Option<&Path>) {
    while let Some(path) = parent {
        if path == project_path || !path.starts_with(project_path) {
            break;
        }
        match fs::remove_dir(path) {
            Ok(()) => parent = path.parent(),
            Err(_) => break,
        }
    }
}

fn git_success(project_path: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_capture(project_path: &Path, args: &[&str]) -> RepoDeskResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(git_error(args, &output.stderr))
    }
}

fn git_run(project_path: &Path, args: &[&str]) -> RepoDeskResult<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_error(args, &output.stderr))
    }
}

fn git_error(args: &[&str], stderr: &[u8]) -> RepoDeskError {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    routing_error(format!(
        "git {} failed{}",
        args.join(" "),
        if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        }
    ))
}

fn routing_error(detail: impl Into<String>) -> RepoDeskError {
    RepoDeskError::RoutingFailed {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "RepoDesk Test"]);
        git(
            dir.path(),
            &["config", "user.email", "repodesk-test@example.invalid"],
        );
        for (path, content) in [("a.txt", "a0\n"), ("b.txt", "b0\n"), ("c.txt", "c0\n")] {
            fs::write(dir.path().join(path), content).expect("seed file");
        }
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "base"]);
        dir
    }

    #[test]
    fn rollback_restores_partial_in_place_staging_but_keeps_worktree_edits() {
        let dir = repo();
        fs::write(dir.path().join("a.txt"), "a1\n").unwrap();
        fs::write(dir.path().join("b.txt"), "b1\n").unwrap();
        let transaction = AcceptTransaction::begin(
            dir.path(),
            &["a.txt".to_string(), "b.txt".to_string()],
            false,
        )
        .unwrap();

        git(dir.path(), &["add", "--", "a.txt"]);
        assert_eq!(git(dir.path(), &["diff", "--cached", "--name-only"]), "a.txt");

        transaction.rollback().unwrap();
        assert!(git(dir.path(), &["diff", "--cached", "--name-only"]).is_empty());
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "a1\n");
        assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "b1\n");
    }

    #[test]
    fn rollback_restores_isolated_worktree_bytes_and_removes_new_files() {
        let dir = repo();
        let original_tree = git(dir.path(), &["write-tree"]);
        let transaction = AcceptTransaction::begin(
            dir.path(),
            &["a.txt".to_string(), "new.txt".to_string()],
            true,
        )
        .unwrap();

        fs::write(dir.path().join("a.txt"), "agent change\n").unwrap();
        fs::write(dir.path().join("new.txt"), "agent new\n").unwrap();
        git(dir.path(), &["add", "--", "a.txt", "new.txt"]);

        transaction.rollback().unwrap();
        assert_eq!(git(dir.path(), &["write-tree"]), original_tree);
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "a0\n");
        assert!(!dir.path().join("new.txt").exists());
    }

    #[test]
    fn rollback_preserves_unrelated_staged_changes_exactly() {
        let dir = repo();
        fs::write(dir.path().join("c.txt"), "c staged before accept\n").unwrap();
        git(dir.path(), &["add", "--", "c.txt"]);
        let original_tree = git(dir.path(), &["write-tree"]);
        let transaction =
            AcceptTransaction::begin(dir.path(), &["a.txt".to_string()], false).unwrap();

        fs::write(dir.path().join("a.txt"), "partial accept\n").unwrap();
        git(dir.path(), &["add", "--", "a.txt"]);
        transaction.rollback().unwrap();

        assert_eq!(git(dir.path(), &["write-tree"]), original_tree);
        assert_eq!(git(dir.path(), &["diff", "--cached", "--name-only"]), "c.txt");
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "partial accept\n");
    }

    #[test]
    fn rename_transaction_paths_expand_both_sides_and_reject_escape() {
        use crate::orchestrator::types::{RunStatus, SubAgentResult, SubAgentStatus};

        let result = SubAgentResult {
            task_id: "impl".into(),
            agent: "codex".into(),
            provider: "codex".into(),
            model: String::new(),
            status: SubAgentStatus::Ok,
            output: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: vec![
                "src/old.rs -> src/new.rs".into(),
                "../escape -> src/safe.rs".into(),
            ],
            diff_path: None,
            workspace: None,
            notes: vec![],
        };
        let run = OrchestrationRun {
            run_id: "run".into(),
            project: "project".into(),
            task_id: "task".into(),
            goal: "goal".into(),
            status: RunStatus::Completed,
            dry_run: false,
            started_at: String::new(),
            finished_at: String::new(),
            results: vec![result],
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_units: 0.0,
        };

        assert_eq!(
            transaction_paths(&run),
            vec!["src/old.rs", "src/new.rs", "src/safe.rs"]
        );
    }
}
