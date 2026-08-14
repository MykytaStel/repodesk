//! The evidence-producing back half of the Work flow: **verification** and the
//! **bounded commit**. Both write receipts ([`super::receipt`]) so the Verify and
//! Finish phases turn green only on proof, and both refuse to act on anything
//! outside the reviewed changeset.

use std::process::Command;

use chrono::Utc;

use crate::engineering::instrumentation::VerificationFinishedTelemetry;
use crate::errors::{RepoDeskError, RepoDeskResult};

use super::receipt::{
    CheckReceipt, FinishReceipt, VerificationReceipt, commit_exists, commit_tree_sha, head_sha,
    index_tree_sha, load_receipt, reviewed_tree_sha_for, save_receipt,
};

/// Result of running final verification.
#[derive(Debug, Clone)]
pub struct VerificationOutcome {
    pub success: bool,
    pub commands: Vec<CheckReceipt>,
}

/// Result of a bounded commit.
#[derive(Debug, Clone)]
pub struct CommitOutcome {
    pub commit_sha: String,
    pub committed_paths: Vec<String>,
}

fn active_project_path() -> RepoDeskResult<std::path::PathBuf> {
    Ok(crate::projects::get_active_project()?.path)
}

/// Run the project's checks and record a [`VerificationReceipt`] bound to the
/// current run, HEAD, staged index tree, and reviewed changeset. When the run
/// has changes, the current staged tree must still be the *exact* tree captured
/// by Accept; a path-only digest is never sufficient.
pub fn run_verification() -> RepoDeskResult<VerificationOutcome> {
    let mut receipt = load_receipt()?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "no run to verify — run the agent first".to_string(),
    })?;
    let project_path = active_project_path()?;
    let task = crate::tasks::show_active_task()?.config;

    let head = head_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "active project is not a git repository with a commit".to_string(),
    })?;
    let tree = index_tree_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "could not read the staged index tree".to_string(),
    })?;

    // For runs that changed files, Verify is permitted only while the current
    // index is byte-for-byte the tree the human accepted. For no-change runs,
    // Review is intentionally vacuous and there is no accepted tree to require.
    let reviewed_tree = reviewed_tree_sha_for(&receipt, &tree)?;
    debug_assert!(
        reviewed_tree
            .as_deref()
            .is_none_or(|accepted| accepted == tree)
    );

    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| super::receipt::changeset_digest(&[]));

    // Only an invocation that reached the actual check runner is a verification
    // attempt. Preconditions above can block Verify without polluting attempt
    // metrics with an unmatched `VerificationStarted` event.
    let verification_id =
        crate::engineering::instrumentation::new_verification_id(&receipt.run_id).ok();
    if let Some(id) = verification_id.clone() {
        let _ = crate::engineering::instrumentation::record_verification_started(
            &task,
            &receipt.run_id,
            id,
        );
    }

    let result = match crate::checks::run_checks() {
        Ok(result) => result,
        Err(error) => {
            if let Some(id) = verification_id {
                let error_text = error.to_string();
                let _ = crate::engineering::instrumentation::record_verification_finished(
                    &task,
                    &receipt.run_id,
                    id,
                    VerificationFinishedTelemetry {
                        success: false,
                        command_count: 0,
                        summary_path: None,
                        log_path: None,
                        error: Some(&error_text),
                    },
                );
            }
            return Err(error);
        }
    };
    let commands: Vec<CheckReceipt> = result
        .commands
        .iter()
        .map(|check| CheckReceipt {
            command: check.command.clone(),
            success: check.exit_code == Some(0),
        })
        .collect();
    // Pass when the checks pass (a project with none configured passes
    // vacuously, but the receipt is still bound to this HEAD/index/changeset).
    let success = result.success;

    receipt.verification = Some(VerificationReceipt {
        run_id: receipt.run_id.clone(),
        head_sha: head,
        // `reviewed_tree_sha_for` proved this is still the accepted tree for a
        // changed run. Persist that exact SHA as the verification target.
        index_tree_sha: reviewed_tree.unwrap_or(tree),
        changeset_digest: digest,
        commands: commands.clone(),
        success,
        verified_at: Utc::now().to_rfc3339(),
    });
    // A re-verification supersedes any prior finish for this run.
    receipt.finish = None;
    save_receipt(&receipt)?;

    if let Some(id) = verification_id {
        let _ = crate::engineering::instrumentation::record_verification_finished(
            &task,
            &receipt.run_id,
            id,
            VerificationFinishedTelemetry {
                success,
                command_count: commands.len(),
                summary_path: result.summary_file.to_str(),
                log_path: result.log_file.to_str(),
                error: None,
            },
        );
    }

    Ok(VerificationOutcome { success, commands })
}

/// Commit **only** the exact ChangeSet authorized by the canonical Safe Commit
/// Manifest — never `git add -A`. The manifest is the same read model rendered
/// by Changes, so operator readiness and the actual Finish gate cannot drift.
/// After Git returns success, the resulting commit tree is independently
/// compared with the reviewed tree to catch hooks or unexpected index mutation.
pub fn commit_reviewed_index(message: &str) -> RepoDeskResult<CommitOutcome> {
    let message = message.trim();
    if message.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit message cannot be empty".to_string(),
        });
    }
    if message.contains('\0') || message.contains('\r') {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit message contains unsupported characters".to_string(),
        });
    }

    let manifest = crate::engineering::load_active_safe_commit_manifest()?;
    if let Some(detail) = manifest.blocker_message() {
        return Err(RepoDeskError::RoutingFailed { detail });
    }
    if !manifest.ready {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit blocked: Safe Commit Manifest is not ready".to_string(),
        });
    }

    let mut receipt = load_receipt()?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "no run to commit — run the agent first".to_string(),
    })?;
    if manifest.run_id.as_deref() != Some(receipt.run_id.as_str()) {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit blocked: Safe Commit Manifest belongs to a different run".to_string(),
        });
    }
    let reviewed_tree = manifest.reviewed_tree_sha.clone().ok_or_else(|| {
        RepoDeskError::RoutingFailed {
            detail: "commit blocked: Safe Commit Manifest has no exact reviewed tree".to_string(),
        }
    })?;
    let staged = manifest.staged_paths.clone();
    let project_path = active_project_path()?;

    // Commit the already-reviewed index (no `git add`). The manifest has proven
    // exact path/tree equality, current successful verification, scope policy,
    // and configured acceptance evidence immediately before this side effect.
    let output = Command::new("git")
        .arg("-C")
        .arg(&project_path)
        .args(["commit", "-m", message])
        .output()
        .map_err(|error| RepoDeskError::RoutingFailed {
            detail: format!("git commit failed: {error}"),
        })?;
    if !output.status.success() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }

    // A hook can mutate the index while `git commit` runs. Never mint Finish
    // evidence until the committed object itself proves the exact reviewed tree.
    let commit_sha = head_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "commit succeeded but HEAD could not be read".to_string(),
    })?;
    debug_assert!(commit_exists(&project_path, &commit_sha));
    let committed_tree = commit_tree_sha(&project_path, &commit_sha).ok_or_else(|| {
        RepoDeskError::RoutingFailed {
            detail: format!(
                "commit {commit_sha} was created, but RepoDesk could not verify its tree — Finish remains unproven"
            ),
        }
    })?;
    if committed_tree != reviewed_tree {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "commit integrity violation: commit {commit_sha} contains tree {committed_tree}, but Safe Commit Manifest {} authorized reviewed tree {reviewed_tree}. Finish was not recorded; inspect the commit before continuing.",
                manifest.manifest_digest
            ),
        });
    }

    receipt.finish = Some(FinishReceipt {
        run_id: receipt.run_id.clone(),
        commit_sha: commit_sha.clone(),
        committed_paths: staged.clone(),
        finished_at: Utc::now().to_rfc3339(),
    });
    save_receipt(&receipt)?;

    if let Ok(task) = crate::tasks::show_active_task() {
        let _ = crate::engineering::instrumentation::record_commit_created(
            &task.config,
            &receipt.run_id,
            &commit_sha,
            &staged,
        );
    }

    Ok(CommitOutcome {
        commit_sha,
        committed_paths: staged,
    })
}
