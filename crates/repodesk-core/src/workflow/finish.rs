//! The evidence-producing back half of the Work flow: **verification** and the
//! **bounded commit**. Both write receipts ([`super::receipt`]) so the Verify and
//! Finish phases turn green only on proof, and both refuse to act on anything
//! outside the reviewed changeset.

use std::collections::HashSet;
use std::process::Command;

use chrono::Utc;

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::receipt::{
    CheckReceipt, FinishReceipt, ReviewDecision, VerificationReceipt, commit_exists, head_sha,
    index_tree_sha, load_receipt, save_receipt, staged_paths,
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
/// current run, HEAD, staged index tree, and reviewed changeset. The Verify
/// phase only counts this as done while none of those have moved since.
pub fn run_verification() -> RepoDeskResult<VerificationOutcome> {
    let mut receipt = load_receipt()?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "no run to verify — run the agent first".to_string(),
    })?;
    let project_path = active_project_path()?;
    let task = crate::tasks::show_active_task()?.config;

    let verification_id =
        crate::engineering::instrumentation::new_verification_id(&receipt.run_id).ok();
    if let Some(id) = verification_id.clone() {
        let _ = crate::engineering::instrumentation::record_verification_started(
            &task,
            &receipt.run_id,
            id,
        );
    }

    let head = head_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "active project is not a git repository with a commit".to_string(),
    })?;
    let tree = index_tree_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "could not read the staged index tree".to_string(),
    })?;
    // Bind to the reviewed changeset (the run's recorded files), so re-reviewing
    // a different changeset invalidates this verification.
    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| super::receipt::changeset_digest(&[]));

    let result = match crate::checks::run_checks() {
        Ok(result) => result,
        Err(error) => {
            if let Some(id) = verification_id {
                let error_text = error.to_string();
                let _ = crate::engineering::instrumentation::record_verification_finished(
                    &task,
                    &receipt.run_id,
                    id,
                    false,
                    0,
                    None,
                    None,
                    Some(&error_text),
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
        index_tree_sha: tree,
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
            success,
            commands.len(),
            result.summary_file.to_str(),
            result.log_file.to_str(),
            None,
        );
    }

    Ok(VerificationOutcome { success, commands })
}

/// Commit **only** the already-staged, reviewed changeset — never `git add -A`.
/// Refuses unless the run was accepted and verification is still fresh, and
/// refuses if the index holds any path outside the reviewed set.
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

    let mut receipt = load_receipt()?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "no run to commit — run the agent first".to_string(),
    })?;
    let project_path = active_project_path()?;
    let run_digest = receipt.execution.changeset_digest.clone();

    // 1. The run's changeset must have been accepted (this run, this digest).
    let review = receipt
        .review
        .as_ref()
        .filter(|r| r.decision == ReviewDecision::Accepted && r.run_id == receipt.run_id)
        .filter(|r| Some(&r.changeset_digest) == run_digest.as_ref())
        .ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: "commit blocked: the run's changes have not been reviewed and accepted"
                .to_string(),
        })?;
    let reviewed: HashSet<&str> = review.reviewed_paths.iter().map(String::as_str).collect();

    // 2. Verification must still be valid against the current tree.
    let head = head_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "active project is not a git repository with a commit".to_string(),
    })?;
    let tree = index_tree_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "could not read the staged index tree".to_string(),
    })?;
    let digest = run_digest
        .clone()
        .unwrap_or_else(|| super::receipt::changeset_digest(&[]));
    let verified = receipt
        .verification
        .as_ref()
        .map(|v| v.run_id == receipt.run_id && v.valid_for(&head, &tree, &digest))
        .unwrap_or(false);
    if !verified {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit blocked: verification is missing or stale — run verification again"
                .to_string(),
        });
    }

    // 3. The index must hold exactly the reviewed changeset — no stray files.
    let staged = staged_paths(&project_path);
    if staged.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "commit blocked: nothing is staged — accept the reviewed changes first"
                .to_string(),
        });
    }
    let stray: Vec<String> = staged
        .iter()
        .filter(|path| !reviewed.contains(path.as_str()))
        .cloned()
        .collect();
    if !stray.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "commit blocked: the index holds files outside the reviewed changeset: {}. \
                 Unstage them so the commit stays bounded to this task.",
                stray.join(", ")
            ),
        });
    }

    // 4. Commit the existing index (no `git add`), then record the finish.
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

    let commit_sha = head_sha(&project_path).ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: "commit succeeded but HEAD could not be read".to_string(),
    })?;
    debug_assert!(commit_exists(&project_path, &commit_sha));

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
