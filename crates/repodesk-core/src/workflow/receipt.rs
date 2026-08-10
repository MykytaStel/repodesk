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

use crate::errors::{RepoDeskError, RepoDeskResult};
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
    /// Digest of the run's path set. This is an identity for *which paths* the
    /// run reported, not proof of their bytes. Exact accepted content is bound
    /// separately by [`ReviewReceipt::index_tree_after_accept`].
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
/// run, the path-set digest, and — critically — the exact Git index tree after
/// Accept. The path digest says *which files*; the tree SHA says *which bytes*.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReceipt {
    pub run_id: String,
    pub decision: ReviewDecision,
    pub reviewed_paths: Vec<String>,
    pub changeset_digest: String,
    /// The exact index tree SHA after the accepted changeset was staged.
    /// Accepted reviews without this proof are invalid and may not be saved.
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

impl TaskRunReceipt {
    /// Invalidate current review/verification evidence when the staged index no
    /// longer equals the exact tree the human accepted. A completed run is
    /// historical evidence and is not reopened merely because the repository
    /// later moves on to another commit.
    fn invalidate_stale_review_tree(&mut self, current_tree: Option<&str>) {
        if self.finish.is_some() {
            return;
        }
        let stale = self
            .review
            .as_ref()
            .filter(|review| review.decision == ReviewDecision::Accepted)
            .map(|review| {
                let run_matches = review.run_id == self.run_id;
                let digest_matches = self.execution.changeset_digest.as_deref()
                    == Some(review.changeset_digest.as_str());
                let tree_matches =
                    match (review.index_tree_after_accept.as_deref(), current_tree) {
                        (Some(reviewed), Some(current)) => reviewed == current,
                        _ => false,
                    };
                !run_matches || !digest_matches || !tree_matches
            })
            .unwrap_or(false);

        if stale {
            self.review = None;
            self.verification = None;
            self.finish = None;
        }
    }
}

// ── Persistence ──────────────────────────────────────────────────────────────

fn receipt_path() -> RepoDeskResult<PathBuf> {
    Ok(show_active_task()?
        .config
        .run_dir
        .join("task-run-receipt.json"))
}

fn validate_receipt(receipt: &TaskRunReceipt) -> RepoDeskResult<()> {
    if let Some(review) = receipt.review.as_ref() {
        if review.run_id != receipt.run_id {
            return Err(RepoDeskError::RoutingFailed {
                detail: "review receipt does not belong to the current run".to_string(),
            });
        }
        if review.decision == ReviewDecision::Accepted {
            let execution_digest = receipt.execution.changeset_digest.as_deref();
            if execution_digest != Some(review.changeset_digest.as_str()) {
                return Err(RepoDeskError::RoutingFailed {
                    detail: "accepted review does not match the run changeset".to_string(),
                });
            }
            if review
                .index_tree_after_accept
                .as_deref()
                .map(str::trim)
                .filter(|tree| !tree.is_empty())
                .is_none()
            {
                return Err(RepoDeskError::RoutingFailed {
                    detail: "accept blocked: could not bind the reviewed changes to an exact index tree"
                        .to_string(),
                });
            }
        }
    }

    if let Some(verification) = receipt.verification.as_ref() {
        if verification.run_id != receipt.run_id {
            return Err(RepoDeskError::RoutingFailed {
                detail: "verification receipt does not belong to the current run".to_string(),
            });
        }
        if let Some(run_digest) = receipt.execution.changeset_digest.as_deref() {
            let review = receipt
                .review
                .as_ref()
                .filter(|review| review.decision == ReviewDecision::Accepted)
                .ok_or_else(|| RepoDeskError::RoutingFailed {
                    detail: "verification cannot be saved without an accepted review".to_string(),
                })?;
            let reviewed_tree = review
                .index_tree_after_accept
                .as_deref()
                .ok_or_else(|| RepoDeskError::RoutingFailed {
                    detail: "verification cannot be saved without an exact reviewed tree".to_string(),
                })?;
            if review.changeset_digest != run_digest
                || verification.changeset_digest != run_digest
                || verification.index_tree_sha != reviewed_tree
            {
                return Err(RepoDeskError::RoutingFailed {
                    detail: "verification receipt is not bound to the exact reviewed tree".to_string(),
                });
            }
        }
    }

    if let Some(finish) = receipt.finish.as_ref()
        && finish.run_id != receipt.run_id
    {
        return Err(RepoDeskError::RoutingFailed {
            detail: "finish receipt does not belong to the current run".to_string(),
        });
    }

    Ok(())
}

/// Load the active task's current run receipt, or `None` when absent/unreadable
/// (a corrupt file must never break the Work surface). Before returning live
/// evidence, an unfinished Accepted review is compared with the current index
/// tree; a changed tree logically invalidates Review + Verification so the Work
/// phase returns to Review instead of trusting a path-only digest.
pub fn load_receipt() -> RepoDeskResult<Option<TaskRunReceipt>> {
    let path = receipt_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let mut receipt: TaskRunReceipt = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
    {
        Some(receipt) => receipt,
        None => return Ok(None),
    };

    if receipt.finish.is_none() && receipt.review.is_some() {
        let current_tree = crate::projects::get_active_project()
            .ok()
            .and_then(|project| index_tree_sha(&project.path));
        receipt.invalidate_stale_review_tree(current_tree.as_deref());
    }

    Ok(Some(receipt))
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

/// Persist the run receipt for the active task. Receipt invariants are checked
/// before any write so Accepted/Verified cannot be stored without exact tree
/// evidence.
pub fn save_receipt(receipt: &TaskRunReceipt) -> RepoDeskResult<()> {
    validate_receipt(receipt)?;
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

/// Return the exact accepted tree for a run that has changes, and prove it still
/// equals the current index tree. Runs with no changes return `Ok(None)` because
/// Review is intentionally vacuous for them.
pub fn reviewed_tree_sha_for(
    receipt: &TaskRunReceipt,
    current_index_tree: &str,
) -> RepoDeskResult<Option<String>> {
    let Some(run_digest) = receipt.execution.changeset_digest.as_deref() else {
        return Ok(None);
    };
    let review = receipt
        .review
        .as_ref()
        .filter(|review| review.decision == ReviewDecision::Accepted)
        .filter(|review| review.run_id == receipt.run_id)
        .filter(|review| review.changeset_digest == run_digest)
        .ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: "review is missing or stale — accept the exact changeset again before verification"
                .to_string(),
        })?;
    let reviewed_tree = review
        .index_tree_after_accept
        .as_deref()
        .map(str::trim)
        .filter(|tree| !tree.is_empty())
        .ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: "review is missing exact tree evidence — accept the changeset again".to_string(),
        })?;
    if reviewed_tree != current_index_tree {
        return Err(RepoDeskError::RoutingFailed {
            detail: "review is stale: the staged tree changed after Accept — review and accept the changes again"
                .to_string(),
        });
    }
    Ok(Some(reviewed_tree.to_string()))
}

// ── Digest + git facts (argv-only, no shell) ─────────────────────────────────

/// Stable digest of a changeset's path set: SHA-256 over sorted-unique
/// repo-relative paths. Equal path sets always hash equal; this deliberately
/// does **not** prove content identity — the accepted index tree SHA does that.
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

/// Tree SHA stored by a specific commit object.
pub fn commit_tree_sha(project_path: &Path, commit_sha: &str) -> Option<String> {
    if commit_sha.trim().is_empty() {
        return None;
    }
    let spec = format!("{commit_sha}^{{tree}}");
    let output = Command::new("git")
        .args(["rev-parse", &spec])
        .current_dir(project_path)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
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

    fn reviewed_receipt(tree: Option<&str>) -> TaskRunReceipt {
        let paths = vec!["src/a.rs".to_string()];
        let digest = changeset_digest(&paths);
        TaskRunReceipt {
            task_id: "t1".into(),
            run_id: "r1".into(),
            execution_mode: ExecutionMode::AgentRun,
            base_commit: Some("base".into()),
            execution: ExecutionReceipt {
                status: RunStatus::Completed,
                required_steps: vec![step("impl", SubAgentStatus::Ok, true)],
                changeset_digest: Some(digest.clone()),
            },
            review: Some(ReviewReceipt {
                run_id: "r1".into(),
                decision: ReviewDecision::Accepted,
                reviewed_paths: paths,
                changeset_digest: digest.clone(),
                index_tree_after_accept: tree.map(str::to_string),
            }),
            verification: Some(VerificationReceipt {
                run_id: "r1".into(),
                head_sha: "head".into(),
                index_tree_sha: tree.unwrap_or_default().into(),
                changeset_digest: digest,
                commands: vec![],
                success: true,
                verified_at: "now".into(),
            }),
            finish: None,
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
    fn digest_is_order_independent_and_path_sensitive() {
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

    #[test]
    fn accepted_review_is_bound_to_exact_tree_not_only_paths() {
        let receipt = reviewed_receipt(Some("tree-t1"));
        assert_eq!(
            reviewed_tree_sha_for(&receipt, "tree-t1")
                .unwrap()
                .as_deref(),
            Some("tree-t1")
        );
        // Same run + same path digest, but different staged bytes => different
        // tree. Verification/commit must not inherit the old Accept.
        assert!(reviewed_tree_sha_for(&receipt, "tree-t2").is_err());
    }

    #[test]
    fn staged_tree_change_invalidates_review_and_verification() {
        let mut receipt = reviewed_receipt(Some("tree-t1"));
        receipt.invalidate_stale_review_tree(Some("tree-t2"));
        assert!(receipt.review.is_none());
        assert!(receipt.verification.is_none());
        assert!(receipt.finish.is_none());
    }

    #[test]
    fn accepted_review_without_tree_proof_is_rejected() {
        let receipt = reviewed_receipt(None);
        assert!(validate_receipt(&receipt).is_err());
    }
}
