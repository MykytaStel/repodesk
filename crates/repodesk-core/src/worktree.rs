//! Isolated git worktrees for write-capable coding-agent runs.
//!
//! Running an agent inside a fresh worktree checked out at the project's current
//! `HEAD` means its changes are attributable **even when the main working tree is
//! dirty**: the diff is computed against a clean base, not tangled with the
//! developer's in-progress edits. RepoDesk creates the worktree, the agent edits
//! only there, and the resulting diff is captured for review.
//!
//! Lifecycle is explicit and non-destructive by default: [`create_run_worktree`]
//! makes one and records recovery metadata; [`list_run_worktrees`] enumerates
//! what git knows about; [`remove_run_worktree`] tears one down only when asked.
//! Nothing here commits, pushes, merges, or auto-cleans — the human stays the
//! operator. Lifecycle code lives here, never inside a provider/executor client.

use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

/// A linked worktree created for one orchestration run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunWorktree {
    pub run_id: String,
    /// Absolute path to the worktree root (where the agent runs).
    pub path: String,
    /// The commit the worktree was checked out at — its diff base.
    pub base_commit: String,
    pub created_at: String,
}

/// Create an isolated worktree for `run_id`, checked out detached at the
/// project's current `HEAD`, rooted at `parent_dir/<run_id>`. Fails if the
/// project has no commit yet (a worktree needs a base) or if git refuses.
pub fn create_run_worktree(
    project_path: &Path,
    parent_dir: &Path,
    run_id: &str,
) -> RepoDeskResult<RunWorktree> {
    let base_commit = git_capture(project_path, &["rev-parse", "HEAD"]).map_err(|error| {
        RepoDeskError::RoutingFailed {
            detail: format!("cannot create a worktree without a base commit: {error}"),
        }
    })?;

    let path = parent_dir.join(run_id);
    // A stale path from a previous run would make `worktree add` fail; clear it.
    if path.exists() {
        let _ = remove_worktree_path(project_path, &path);
    }
    std::fs::create_dir_all(parent_dir)?;

    let path_str = path.display().to_string();
    git_capture(
        project_path,
        &["worktree", "add", "--detach", &path_str, "HEAD"],
    )?;

    Ok(RunWorktree {
        run_id: run_id.to_string(),
        path: path_str,
        base_commit: base_commit.trim().to_string(),
        created_at: Utc::now().to_rfc3339(),
    })
}

/// Remove a previously created run worktree (forced — it may hold the agent's
/// uncommitted edits, which is the caller's decision to discard).
pub fn remove_run_worktree(project_path: &Path, worktree: &RunWorktree) -> RepoDeskResult<()> {
    remove_worktree_path(project_path, Path::new(&worktree.path))
}

fn remove_worktree_path(project_path: &Path, path: &Path) -> RepoDeskResult<()> {
    let path_str = path.display().to_string();
    git_capture(project_path, &["worktree", "remove", "--force", &path_str])?;
    Ok(())
}

/// Absolute worktree paths git currently tracks for the project (excluding the
/// main working tree). Useful for recovering or cleaning up interrupted runs.
pub fn list_run_worktrees(project_path: &Path) -> RepoDeskResult<Vec<String>> {
    let output = git_capture(project_path, &["worktree", "list", "--porcelain"])?;
    let main = project_path
        .canonicalize()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| project_path.display().to_string());
    let mut paths = Vec::new();
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            let candidate = rest.trim().to_string();
            let canonical = Path::new(&candidate)
                .canonicalize()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| candidate.clone());
            if canonical != main {
                paths.push(candidate);
            }
        }
    }
    Ok(paths)
}

/// Run a git subcommand (argv-only, no shell) in `project_path`, returning stdout
/// on success or a sanitized error on failure.
fn git_capture(project_path: &Path, args: &[&str]) -> RepoDeskResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .map_err(|error| RepoDeskError::RoutingFailed {
            detail: format!("failed to run git {}: {error}", args.join(" ")),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

/// Convenience for callers that root run worktrees under a task's run dir.
pub fn worktrees_parent(run_dir: &Path) -> PathBuf {
    run_dir.join("worktrees")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo() -> tempfile::TempDir {
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(repo.path())
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "git {args:?} failed"
            );
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "init"]);
        repo
    }

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    #[test]
    fn create_list_and_remove_worktree() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();

        let wt = create_run_worktree(repo.path(), parent.path(), "run-1").unwrap();
        assert!(Path::new(&wt.path).exists());
        assert!(Path::new(&wt.path).join("seed.txt").exists());
        assert!(!wt.base_commit.is_empty());

        let listed = list_run_worktrees(repo.path()).unwrap();
        assert!(listed.iter().any(|p| p.contains("run-1")));

        remove_run_worktree(repo.path(), &wt).unwrap();
        assert!(!Path::new(&wt.path).exists());
    }

    #[test]
    fn agent_edits_in_worktree_do_not_touch_main_tree() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let wt = create_run_worktree(repo.path(), parent.path(), "run-2").unwrap();

        // Simulate the agent editing only inside the worktree.
        std::fs::write(Path::new(&wt.path).join("seed.txt"), "seed\nagent\n").unwrap();
        std::fs::write(Path::new(&wt.path).join("new.txt"), "new\n").unwrap();

        // The main working tree is untouched.
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\n"
        );
        assert!(!repo.path().join("new.txt").exists());

        remove_run_worktree(repo.path(), &wt).unwrap();
    }
}
