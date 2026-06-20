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

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::git_workspace::parse_porcelain_status;

/// A linked worktree created for one orchestration run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunWorktree {
    /// Collision-resistant identity for this specific run step workspace.
    pub workspace_id: String,
    pub run_id: String,
    pub step_id: String,
    /// Absolute path to the worktree root (where the agent runs).
    pub path: String,
    /// The commit the worktree was checked out at — its diff base.
    pub base_commit: String,
    pub created_at: String,
    /// Recovery metadata persisted before the agent process is launched.
    #[serde(default)]
    pub metadata_path: Option<String>,
}

/// UI/recovery view of one RepoDesk-managed run worktree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunWorktreeStatus {
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub step_id: Option<String>,
    /// Absolute path to the worktree root.
    pub path: String,
    pub base_commit: Option<String>,
    pub created_at: Option<String>,
    pub metadata_path: Option<String>,
    /// True when `git worktree list` still tracks this path.
    pub git_tracked: bool,
    pub exists: bool,
    pub dirty: bool,
    pub changed_files: Vec<String>,
    /// True when RepoDesk can safely remove this worktree by workspace id.
    pub removable: bool,
    pub warnings: Vec<String>,
}

/// Result of explicitly removing a run worktree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunWorktreeCleanup {
    pub workspace_id: String,
    pub path: String,
    pub removed: bool,
    pub metadata_removed: bool,
    pub warnings: Vec<String>,
}

static WORKTREE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create an isolated worktree for one `run_id` + `step_id`, checked out
/// detached at the project's current `HEAD`, rooted under `parent_dir`.
///
/// The workspace id includes the run id, step id, nanosecond timestamp, process
/// id, and a process-local counter. Existing paths are never removed or reused:
/// a collision or stale workspace is treated as recoverable user data and makes
/// creation fail closed.
pub fn create_run_worktree(
    project_path: &Path,
    parent_dir: &Path,
    run_id: &str,
    step_id: &str,
) -> RepoDeskResult<RunWorktree> {
    let workspace_id = unique_workspace_id(run_id, step_id);
    create_worktree_with_workspace_id(project_path, parent_dir, run_id, step_id, &workspace_id)
}

fn create_worktree_with_workspace_id(
    project_path: &Path,
    parent_dir: &Path,
    run_id: &str,
    step_id: &str,
    workspace_id: &str,
) -> RepoDeskResult<RunWorktree> {
    let base_commit = git_capture(project_path, &["rev-parse", "HEAD"]).map_err(|error| {
        RepoDeskError::RoutingFailed {
            detail: format!("cannot create a worktree without a base commit: {error}"),
        }
    })?;

    std::fs::create_dir_all(parent_dir)?;
    let path = parent_dir.join(workspace_id);
    if path.exists() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "refusing to reuse existing worktree path {}; inspect or remove it manually",
                path.display()
            ),
        });
    }

    let path_str = path.display().to_string();
    git_capture(
        project_path,
        &["worktree", "add", "--detach", &path_str, "HEAD"],
    )?;

    let mut worktree = RunWorktree {
        workspace_id: workspace_id.to_string(),
        run_id: run_id.to_string(),
        step_id: step_id.to_string(),
        path: path_str,
        base_commit: base_commit.trim().to_string(),
        created_at: Utc::now().to_rfc3339(),
        metadata_path: None,
    };
    persist_recovery_metadata(parent_dir, &mut worktree)?;
    Ok(worktree)
}

/// Remove a previously created run worktree (forced — it may hold the agent's
/// uncommitted edits, which is the caller's decision to discard).
pub fn remove_run_worktree(project_path: &Path, worktree: &RunWorktree) -> RepoDeskResult<()> {
    remove_worktree_path(project_path, Path::new(&worktree.path)).and_then(|_| {
        remove_metadata_file(worktree.metadata_path.as_deref().map(Path::new)).map(|_| ())
    })
}

fn remove_worktree_path(project_path: &Path, path: &Path) -> RepoDeskResult<()> {
    let path_str = path.display().to_string();
    git_capture(project_path, &["worktree", "remove", "--force", &path_str])?;
    Ok(())
}

fn unique_workspace_id(run_id: &str, step_id: &str) -> String {
    let nanos = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Utc::now().timestamp_micros().saturating_mul(1_000));
    let counter = WORKTREE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{}-{}-{nanos}-{}-{counter}",
        sanitize_id(run_id),
        sanitize_id(step_id),
        std::process::id()
    )
}

fn sanitize_id(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "workspace".to_string()
    } else {
        out
    }
}

fn persist_recovery_metadata(parent_dir: &Path, worktree: &mut RunWorktree) -> RepoDeskResult<()> {
    let metadata_path = parent_dir.join(format!("{}.json", worktree.workspace_id));
    worktree.metadata_path = Some(metadata_path.display().to_string());
    let json = serde_json::to_string_pretty(worktree)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

/// Absolute worktree paths git currently tracks for the project (excluding the
/// main working tree). Useful for recovering or cleaning up interrupted runs.
pub fn list_run_worktrees(project_path: &Path) -> RepoDeskResult<Vec<String>> {
    let all = git_worktree_paths(project_path)?;
    let main = project_path
        .canonicalize()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| project_path.display().to_string());
    Ok(all
        .into_iter()
        .filter(|candidate| canonical_display(candidate) != main)
        .map(|path| path.display().to_string())
        .collect())
}

/// RepoDesk-managed run worktrees for the active task parent, including dirty
/// state and metadata needed for recovery/cleanup UI.
pub fn list_run_worktree_statuses(
    project_path: &Path,
    parent_dir: &Path,
) -> RepoDeskResult<Vec<RunWorktreeStatus>> {
    if !parent_dir.exists() {
        return Ok(Vec::new());
    }

    let mut statuses = Vec::new();
    for path in git_worktree_paths(project_path)? {
        if canonical_display(&path) == canonical_display(project_path) {
            continue;
        }
        if !is_managed_worktree_path(parent_dir, &path) {
            continue;
        }
        statuses.push(inspect_run_worktree(parent_dir, &path, true));
    }

    statuses.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.workspace_id.cmp(&b.workspace_id))
    });
    Ok(statuses)
}

/// Explicitly remove a RepoDesk-managed run worktree by workspace id. The id is
/// resolved under `parent_dir`; arbitrary paths and user-created git worktrees
/// are rejected.
pub fn cleanup_run_worktree(
    project_path: &Path,
    parent_dir: &Path,
    workspace_id: &str,
) -> RepoDeskResult<RunWorktreeCleanup> {
    let workspace_id = validate_workspace_id(workspace_id)?.to_string();
    if !parent_dir.exists() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("worktree parent does not exist: {}", parent_dir.display()),
        });
    }

    let metadata_path = parent_dir.join(format!("{workspace_id}.json"));
    let metadata = read_metadata(&metadata_path, &mut Vec::new());
    let path = metadata
        .as_ref()
        .map(|worktree| PathBuf::from(&worktree.path))
        .unwrap_or_else(|| parent_dir.join(&workspace_id));

    if !is_managed_worktree_path(parent_dir, &path) {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "refusing to remove worktree outside RepoDesk's managed parent: {}",
                path.display()
            ),
        });
    }

    let tracked = git_worktree_paths(project_path)?;
    if !tracked.iter().any(|candidate| same_path(candidate, &path)) {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "worktree is not tracked by git for the active project: {}",
                path.display()
            ),
        });
    }

    let path_display = path.display().to_string();
    git_capture(
        project_path,
        &["worktree", "remove", "--force", &path_display],
    )?;
    let metadata_removed = remove_metadata_file(Some(&metadata_path))?;

    Ok(RunWorktreeCleanup {
        workspace_id,
        path: path_display,
        removed: true,
        metadata_removed,
        warnings: Vec::new(),
    })
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

fn git_worktree_paths(project_path: &Path) -> RepoDeskResult<Vec<PathBuf>> {
    let output = git_capture(project_path, &["worktree", "list", "--porcelain"])?;
    Ok(output
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn inspect_run_worktree(parent_dir: &Path, path: &Path, git_tracked: bool) -> RunWorktreeStatus {
    let mut warnings = Vec::new();
    let workspace_id = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let metadata_path = parent_dir.join(format!("{workspace_id}.json"));
    let metadata = read_metadata(&metadata_path, &mut warnings);
    if let Some(worktree) = &metadata {
        if !same_path(Path::new(&worktree.path), path) {
            warnings.push(format!(
                "metadata path points to {}, but git tracks {}",
                worktree.path,
                path.display()
            ));
        }
        if worktree.workspace_id != workspace_id {
            warnings.push(format!(
                "metadata workspace id is {}, but path is {}",
                worktree.workspace_id, workspace_id
            ));
        }
    }

    let exists = path.exists();
    let mut changed_files = Vec::new();
    if exists {
        match git_capture(path, &["status", "--porcelain=v1"]) {
            Ok(raw_status) => {
                changed_files = parse_porcelain_status(&raw_status)
                    .into_iter()
                    .map(|change| change.path)
                    .collect();
            }
            Err(error) => warnings.push(format!("failed to read worktree status: {error}")),
        }
    } else {
        warnings.push("worktree path no longer exists".to_string());
    }

    let removable = git_tracked
        && exists
        && is_managed_worktree_path(parent_dir, path)
        && validate_workspace_id(&workspace_id).is_ok();
    RunWorktreeStatus {
        workspace_id,
        run_id: metadata.as_ref().map(|worktree| worktree.run_id.clone()),
        step_id: metadata.as_ref().map(|worktree| worktree.step_id.clone()),
        path: path.display().to_string(),
        base_commit: metadata
            .as_ref()
            .map(|worktree| worktree.base_commit.clone()),
        created_at: metadata
            .as_ref()
            .map(|worktree| worktree.created_at.clone()),
        metadata_path: if metadata_path.exists() {
            Some(metadata_path.display().to_string())
        } else {
            None
        },
        git_tracked,
        exists,
        dirty: !changed_files.is_empty(),
        changed_files,
        removable,
        warnings,
    }
}

fn read_metadata(metadata_path: &Path, warnings: &mut Vec<String>) -> Option<RunWorktree> {
    if !metadata_path.exists() {
        return None;
    }
    match fs::read_to_string(metadata_path)
        .ok()
        .and_then(|content| serde_json::from_str::<RunWorktree>(&content).ok())
    {
        Some(worktree) => Some(worktree),
        None => {
            warnings.push(format!(
                "failed to read worktree metadata: {}",
                metadata_path.display()
            ));
            None
        }
    }
}

fn remove_metadata_file(metadata_path: Option<&Path>) -> RepoDeskResult<bool> {
    let Some(metadata_path) = metadata_path else {
        return Ok(false);
    };
    if !metadata_path.exists() {
        return Ok(false);
    }
    fs::remove_file(metadata_path)?;
    Ok(true)
}

fn validate_workspace_id(value: &str) -> RepoDeskResult<&str> {
    let value = value.trim();
    let safe = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if safe {
        Ok(value)
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!("unsafe workspace id: {value}"),
        })
    }
}

fn is_managed_worktree_path(parent_dir: &Path, path: &Path) -> bool {
    let Ok(parent) = parent_dir.canonicalize() else {
        return false;
    };
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    candidate.starts_with(parent)
}

fn same_path(left: &Path, right: &Path) -> bool {
    canonical_display(left) == canonical_display(right)
}

fn canonical_display(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
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

        let wt = create_run_worktree(repo.path(), parent.path(), "run-1", "step-1").unwrap();
        assert!(Path::new(&wt.path).exists());
        assert!(Path::new(&wt.path).join("seed.txt").exists());
        assert!(!wt.base_commit.is_empty());
        assert_eq!(wt.run_id, "run-1");
        assert_eq!(wt.step_id, "step-1");
        assert!(wt.workspace_id.contains("run-1-step-1"));
        let metadata_path = wt.metadata_path.as_ref().expect("metadata path");
        assert!(Path::new(metadata_path).exists());
        let metadata =
            serde_json::from_str::<RunWorktree>(&std::fs::read_to_string(metadata_path).unwrap())
                .unwrap();
        assert_eq!(metadata.workspace_id, wt.workspace_id);

        let listed = list_run_worktrees(repo.path()).unwrap();
        assert!(listed.iter().any(|p| p.contains(&wt.workspace_id)));

        remove_run_worktree(repo.path(), &wt).unwrap();
        assert!(!Path::new(&wt.path).exists());
        assert!(!Path::new(metadata_path).exists());
    }

    #[test]
    fn agent_edits_in_worktree_do_not_touch_main_tree() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let wt = create_run_worktree(repo.path(), parent.path(), "run-2", "implement").unwrap();

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

    #[test]
    fn list_status_reports_dirty_managed_worktree() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let wt = create_run_worktree(repo.path(), parent.path(), "run-3", "implement").unwrap();
        std::fs::write(Path::new(&wt.path).join("seed.txt"), "seed\nagent\n").unwrap();
        std::fs::write(Path::new(&wt.path).join("new.txt"), "new\n").unwrap();

        let statuses = list_run_worktree_statuses(repo.path(), parent.path()).unwrap();
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.workspace_id, wt.workspace_id);
        assert_eq!(status.run_id.as_deref(), Some("run-3"));
        assert_eq!(status.step_id.as_deref(), Some("implement"));
        assert!(status.git_tracked);
        assert!(status.exists);
        assert!(status.dirty);
        assert!(status.removable);
        assert!(status.changed_files.iter().any(|path| path == "seed.txt"));
        assert!(status.changed_files.iter().any(|path| path == "new.txt"));

        remove_run_worktree(repo.path(), &wt).unwrap();
    }

    #[test]
    fn cleanup_run_worktree_removes_worktree_and_metadata() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let wt = create_run_worktree(repo.path(), parent.path(), "run-4", "implement").unwrap();
        let metadata_path = wt.metadata_path.clone().expect("metadata path");

        let cleanup = cleanup_run_worktree(repo.path(), parent.path(), &wt.workspace_id).unwrap();
        assert_eq!(cleanup.workspace_id, wt.workspace_id);
        assert!(cleanup.removed);
        assert!(cleanup.metadata_removed);
        assert!(!Path::new(&wt.path).exists());
        assert!(!Path::new(&metadata_path).exists());
        let listed = list_run_worktrees(repo.path()).unwrap();
        assert!(!listed.iter().any(|path| path.contains(&wt.workspace_id)));
    }

    #[test]
    fn cleanup_rejects_unsafe_workspace_id() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();

        let error = cleanup_run_worktree(repo.path(), parent.path(), "../outside").unwrap_err();
        assert!(error.to_string().contains("unsafe workspace id"));
    }

    #[test]
    fn worktree_ids_are_unique_per_step() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();

        let first = create_run_worktree(repo.path(), parent.path(), "run-3", "implement").unwrap();
        let second = create_run_worktree(repo.path(), parent.path(), "run-3", "review").unwrap();

        assert_ne!(first.workspace_id, second.workspace_id);
        assert_ne!(first.path, second.path);

        remove_run_worktree(repo.path(), &first).unwrap();
        remove_run_worktree(repo.path(), &second).unwrap();
    }

    #[test]
    fn existing_workspace_path_is_not_removed_or_reused() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let stale = parent.path().join("stale");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("keep.txt"), "recover me\n").unwrap();

        let result = create_worktree_with_workspace_id(
            repo.path(),
            parent.path(),
            "run-test",
            "step-test",
            "stale",
        );

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(stale.join("keep.txt")).unwrap(),
            "recover me\n"
        );
    }
}
