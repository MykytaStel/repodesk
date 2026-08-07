//! Evidence receipts for the Work phase flow.
//!
//! Every phase past Prepare turns green only when a **receipt proves the event
//! happened** — not from an indirect proxy (a non-empty index, a shared checks
//! summary) or a manual acknowledgement. One [`TaskRunReceipt`] per task run is
//! persisted at `run_dir/task-run-receipt.json` and is **keyed to a `run_id`**:
//! a new orchestration run overwrites it, which automatically invalidates a
//! stale review or verification from an earlier run.
//!
//! The receipt is the single source of post-execution truth:
//! `run evidence → exact changeset review → fresh verification → bounded commit`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

use super::phase::ExecutionMode;
use crate::orchestrator::types::{RunStatus, SubAgentStatus};

/// One step's contribution to a run, as recorded for evidence. `allow_write`
/// marks the implementation steps whose success is *required* for the run to
/// count as a successful execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepReceipt {
    pub task_id: String,
    pub status: SubAgentStatus,
    pub allow_write: bool,
    #[serde(default)]
    pub changed_files: Vec<String>,
}

/// Proof of what the run actually did. `Partial` is **not** treated as success:
/// [`ExecutionReceipt::succeeded`] requires every required step to be `Ok`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub status: RunStatus,
    pub required_steps: Vec<StepReceipt>,
    /// Digest of the run's changeset (sorted-unique repo-relative paths).
    #[serde(default)]
    pub changeset_digest: Option<String>,
}

impl ExecutionReceipt {
    /// True only when every required (write-capable) step is `Ok`. When a plan
    /// has no write step (e.g. analysis-only), falls back to "all steps Ok".
    pub fn succeeded(&self) -> bool {
        if self.required_steps.is_empty() {
            return false;
        }
        let required: Vec<&StepReceipt> = self
            .required_steps
            .iter()
            .filter(|step| step.allow_write)
            .collect();
        if required.is_empty() {
            self.required_steps
                .iter()
                .all(|step| step.status == SubAgentStatus::Ok)
        } else {
            required
                .iter()
                .all(|step| step.status == SubAgentStatus::Ok)
        }
    }
}

/// Whether the human accepted or rejected the run's changeset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Accepted,
    Rejected,
}

/// Proof the exact changeset was reviewed and (on accept) staged. Bound to the
/// run and to the changeset digest, so a later run cannot inherit this review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReceipt {
    pub run_id: String,
    pub decision: ReviewDecision,
    pub reviewed_paths: Vec<String>,
    pub changeset_digest: String,
    /// The index tree sha after the accepted changeset was staged.
    #[serde(default)]
    pub index_tree_after_accept: Option<String>,
}

/// One verification command's outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReceipt {
    pub command: String,
    pub success: bool,
}

/// Proof that final verification ran against the **current** HEAD, index tree,
/// and reviewed changeset. Any of those changing afterwards invalidates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReceipt {
    pub run_id: String,
    pub head_sha: String,
    pub index_tree_sha: String,
    pub changeset_digest: String,
    pub commands: Vec<CheckReceipt>,
    pub success: bool,
    pub verified_at: String,
}

impl VerificationReceipt {
    /// Valid only when the checks passed *and* nothing has moved since: same
    /// HEAD, same staged index tree, same reviewed changeset.
    pub fn valid_for(&self, head_sha: &str, index_tree_sha: &str, changeset_digest: &str) -> bool {
        self.success
            && self.head_sha == head_sha
            && self.index_tree_sha == index_tree_sha
            && self.changeset_digest == changeset_digest
    }
}

/// Proof a real commit landed the reviewed changeset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishReceipt {
    pub run_id: String,
    pub commit_sha: String,
    pub committed_paths: Vec<String>,
    pub finished_at: String,
}

/// The evidence ledger for a task's current run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunReceipt {
    pub task_id: String,
    pub run_id: String,
    pub execution_mode: ExecutionMode,
    pub base_commit: Option<String>,
    pub execution: ExecutionReceipt,
    #[serde(default)]
    pub review: Option<ReviewReceipt>,
    #[serde(default)]
    pub verification: Option<VerificationReceipt>,
    #[serde(default)]
    pub finish: Option<FinishReceipt>,
}

// ── Persistence ──────────────────────────────────────────────────────────────

fn receipt_path() -> RepoDeskResult<PathBuf> {
    Ok(show_active_task()?
        .config
        .run_dir
        .join("task-run-receipt.json"))
}

/// Load the active task's run receipt, or `None` when absent/unreadable (a
/// corrupt file must never break the Work surface).
pub fn load_receipt() -> RepoDeskResult<Option<TaskRunReceipt>> {
    let path = receipt_path()?;
    if !path.exists() {
        return Ok(None);
    }
    Ok(std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok()))
}

fn review_changed(previous: Option<&TaskRunReceipt>, current: &TaskRunReceipt) -> bool {
    let previous_review = previous
        .filter(|receipt| receipt.run_id == current.run_id)
        .and_then(|receipt| receipt.review.as_ref());
    let current_review = current.review.as_ref();

    match (previous_review, current_review) {
        (None, Some(_)) => true,
        (Some(previous), Some(current)) => {
            previous.run_id != current.run_id
                || previous.decision != current.decision
                || previous.reviewed_paths != current.reviewed_paths
                || previous.changeset_digest != current.changeset_digest
                || previous.index_tree_after_accept != current.index_tree_after_accept
        }
        _ => false,
    }
}

/// Persist the run receipt for the active task.
pub fn save_receipt(receipt: &TaskRunReceipt) -> RepoDeskResult<()> {
    let previous = load_receipt().ok().flatten();
    let should_record_review = review_changed(previous.as_ref(), receipt);

    let path = receipt_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(receipt)?)?;

    if should_record_review
        && let Some(review) = receipt.review.as_ref()
        && let Ok(task) = show_active_task()
    {
        let decision = match review.decision {
            ReviewDecision::Accepted => "accepted",
            ReviewDecision::Rejected => "rejected",
        };
        let _ = crate::engineering::instrumentation::record_changeset_reviewed(
            &task.config,
            &review.run_id,
            decision,
            &review.reviewed_paths,
            &review.changeset_digest,
        );
    }

    Ok(())
}

/// Load the receipt only if it matches `run_id`; a mismatch means it belongs to
/// an older run and must be ignored.
pub fn load_receipt_for_run(run_id: &str) -> RepoDeskResult<Option<TaskRunReceipt>> {
    Ok(load_receipt()?.filter(|receipt| receipt.run_id == run_id))
}

// ── Digest + git facts (argv-only, no shell) ─────────────────────────────────

/// Stable digest of a changeset: SHA-256 over sorted-unique repo-relative paths.
/// Equal path sets always hash equal; any add/remove changes the digest.
pub fn changeset_digest(paths: &[String]) -> String {
    let mut unique: Vec<&str> = paths
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    unique.sort_unstable();
    unique.dedup();
    let mut hasher = Sha256::new();
    for path in unique {
        hasher.update(path.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn git_capture(project_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Current HEAD commit sha, if the path is a repo with at least one commit.
pub fn head_sha(project_path: &Path) -> Option<String> {
    git_capture(project_path, &["rev-parse", "HEAD"])
}

/// Sha of the tree the current index would commit (`git write-tree`). Read-only:
/// it writes a tree object but never mutates the index or HEAD.
pub fn index_tree_sha(project_path: &Path) -> Option<String> {
    git_capture(project_path, &["write-tree"])
}

/// Repo-relative paths currently staged in the index.
pub fn staged_paths(project_path: &Path) -> Vec<String> {
    git_capture(project_path, &["diff", "--cached", "--name-only"])
        .map(|out| {
            out.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a commit object exists in the repo.
pub fn commit_exists(project_path: &Path, sha: &str) -> bool {
    if sha.trim().is_empty() {
        return false;
    }
    Command::new("git")
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, status: SubAgentStatus, allow_write: bool) -> StepReceipt {
        StepReceipt {
            task_id: id.to_string(),
            status,
            allow_write,
            changed_files: Vec::new(),
        }
    }

    #[test]
    fn execution_requires_all_write_steps_ok() {
        // Prepare Ok + implementation Failed → not succeeded (Partial run).
        let exec = ExecutionReceipt {
            status: RunStatus::Partial,
            required_steps: vec![
                step("prepare", SubAgentStatus::Ok, false),
                step("implement", SubAgentStatus::Failed, true),
            ],
            changeset_digest: None,
        };
        assert!(!exec.succeeded());

        let exec_ok = ExecutionReceipt {
            status: RunStatus::Partial,
            required_steps: vec![
                step("prepare", SubAgentStatus::Skipped, false),
                step("implement", SubAgentStatus::Ok, true),
            ],
            changeset_digest: None,
        };
        assert!(exec_ok.succeeded());
    }

    #[test]
    fn execution_without_write_steps_needs_all_ok() {
        let analysis = ExecutionReceipt {
            status: RunStatus::Completed,
            required_steps: vec![
                step("analyze", SubAgentStatus::Ok, false),
                step("summarize", SubAgentStatus::Ok, false),
            ],
            changeset_digest: None,
        };
        assert!(analysis.succeeded());
        let blocked = ExecutionReceipt {
            status: RunStatus::Partial,
            required_steps: vec![
                step("analyze", SubAgentStatus::Ok, false),
                step("summarize", SubAgentStatus::Blocked, false),
            ],
            changeset_digest: None,
        };
        assert!(!blocked.succeeded());
    }

    #[test]
    fn empty_execution_is_not_success() {
        let exec = ExecutionReceipt {
            status: RunStatus::Failed,
            required_steps: vec![],
            changeset_digest: None,
        };
        assert!(!exec.succeeded());
    }

    #[test]
    fn digest_is_order_independent_and_sensitive() {
        let a = changeset_digest(&["src/b.rs".into(), "src/a.rs".into()]);
        let b = changeset_digest(&["src/a.rs".into(), "src/b.rs".into()]);
        assert_eq!(a, b);
        let c = changeset_digest(&["src/a.rs".into()]);
        assert_ne!(a, c);
    }

    #[test]
    fn verification_validity_tracks_head_index_and_digest() {
        let v = VerificationReceipt {
            run_id: "r1".into(),
            head_sha: "head".into(),
            index_tree_sha: "tree".into(),
            changeset_digest: "dig".into(),
            commands: vec![],
            success: true,
            verified_at: "now".into(),
        };
        assert!(v.valid_for("head", "tree", "dig"));
        assert!(!v.valid_for("head2", "tree", "dig")); // HEAD moved
        assert!(!v.valid_for("head", "tree2", "dig")); // staged tree changed
        assert!(!v.valid_for("head", "tree", "dig2")); // changeset changed
        let failed = VerificationReceipt {
            success: false,
            ..v
        };
        assert!(!failed.valid_for("head", "tree", "dig"));
    }
}