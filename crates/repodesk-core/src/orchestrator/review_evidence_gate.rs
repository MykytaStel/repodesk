//! Evidence gate in front of the transactional review boundary.
//!
//! Review is a human decision *and* a repository side effect. The decision must
//! be durable before RepoDesk stages or discards bytes, while final ReviewReceipt
//! persistence must stay bound to the side effect. A small intent record provides
//! that write-ahead boundary: Accept can roll back if final evidence fails;
//! Reject can be safely resumed without rerunning the agent when its final receipt
//! could not be persisted after the destructive side effect.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::tasks::show_active_task;
use crate::workflow::{changeset_digest, load_receipt_for_run};

use super::execution_evidence::require_review_evidence_ready;
use super::review::{self, ReviewAction, RunReview};
use super::review_transaction;

const MAX_REVIEW_INTENT_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ReviewIntentRecord {
    run_id: String,
    action: ReviewAction,
    changeset_digest: String,
    recorded_at: String,
}

/// Review one persisted run through a single evidence-bound public boundary.
///
/// The durable intent is written *before* any repository mutation. Accept keeps
/// the existing transactional rollback semantics and now extends that rollback
/// through final ReviewReceipt persistence. Reject is intentionally resumable:
/// once the human's Reject intent is durable, a failed final receipt leaves the
/// intent in place so repeating Reject can finish the idempotent cleanup without
/// launching the agent again.
pub fn review_run(run_id: &str, action: ReviewAction) -> RepoDeskResult<RunReview> {
    require_review_evidence_ready(run_id)?;
    let intent_path = ensure_review_intent(run_id, action)?;

    let result = review_transaction::review_run_with_persist(run_id, action, |review| {
        review::record_review(run_id, action, review).map_err(|error| {
            let detail = match action {
                ReviewAction::Accept => format!(
                    "accept evidence persistence failed: {error}; the staged Accept will be rolled back while the durable intent remains pending"
                ),
                ReviewAction::Reject => format!(
                    "reject side effects completed, but review evidence persistence failed: {error}; the durable Reject intent remains pending. Retry Reject to finish evidence without rerunning the agent"
                ),
            };
            routing_error(detail)
        })
    });

    if result.is_ok() {
        // The ReviewReceipt is already authoritative. Failure to remove a stale
        // intent must not turn a completed Review back into an apparent failure;
        // the next review preflight recognizes a matching finalized receipt and
        // cleans the stale intent before accepting another decision.
        let _ = remove_review_intent_file(&intent_path);
    }
    result
}

fn ensure_review_intent(run_id: &str, action: ReviewAction) -> RepoDeskResult<PathBuf> {
    validate_run_id(run_id)?;
    let receipt = load_receipt_for_run(run_id)?.ok_or_else(|| {
        routing_error(format!(
            "no workflow receipt for orchestration run '{run_id}'"
        ))
    })?;
    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| changeset_digest(&[]));
    let path = review_intent_path(run_id)?;

    if let Some(existing) = read_review_intent_file(&path)? {
        let finalized = receipt.review.as_ref().is_some_and(|review| {
            review.run_id == existing.run_id
                && review.changeset_digest == existing.changeset_digest
                && matches!(
                    (review.decision, existing.action),
                    (
                        crate::workflow::ReviewDecision::Accepted,
                        ReviewAction::Accept
                    ) | (
                        crate::workflow::ReviewDecision::Rejected,
                        ReviewAction::Reject
                    )
                )
        });
        if finalized {
            remove_review_intent_file(&path)?;
        } else if existing.run_id == run_id
            && existing.action == action
            && existing.changeset_digest == digest
        {
            // Resume the exact human decision after a prior persistence fault.
            return Ok(path);
        } else {
            return Err(routing_error(format!(
                "review blocked: a different durable review decision is still pending for run '{}'. Resume that decision before starting another review",
                existing.run_id
            )));
        }
    }

    let intent = ReviewIntentRecord {
        run_id: run_id.to_string(),
        action,
        changeset_digest: digest,
        recorded_at: Utc::now().to_rfc3339(),
    };
    write_review_intent_file(&path, &intent)?;
    let persisted = read_review_intent_file(&path)?.ok_or_else(|| {
        routing_error("review intent write completed but the durable record is missing")
    })?;
    if persisted != intent {
        return Err(routing_error(
            "review intent verification failed after persistence",
        ));
    }
    Ok(path)
}

fn review_intent_path(run_id: &str) -> RepoDeskResult<PathBuf> {
    validate_run_id(run_id)?;
    Ok(show_active_task()?
        .config
        .run_dir
        .join("orchestrate")
        .join("review-intents")
        .join(format!("{run_id}.json")))
}

fn validate_run_id(run_id: &str) -> RepoDeskResult<()> {
    let valid = !run_id.is_empty()
        && run_id.len() <= 120
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(routing_error(
            "invalid orchestration run id for review intent",
        ))
    }
}

fn read_review_intent_file(path: &Path) -> RepoDeskResult<Option<ReviewIntentRecord>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(routing_error(format!(
            "review intent path is a symlink: {}",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(routing_error(format!(
            "review intent path is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_REVIEW_INTENT_BYTES {
        return Err(routing_error(format!(
            "review intent exceeds the {MAX_REVIEW_INTENT_BYTES} byte limit"
        )));
    }

    let bytes = fs::read(path)?;
    let intent: ReviewIntentRecord = serde_json::from_slice(&bytes).map_err(|error| {
        routing_error(format!(
            "review intent is corrupt or invalid JSON at {}: {error}",
            path.display()
        ))
    })?;
    validate_run_id(&intent.run_id)?;
    if intent.changeset_digest.trim().is_empty() {
        return Err(routing_error("review intent has an empty changeset digest"));
    }
    Ok(Some(intent))
}

fn write_review_intent_file(path: &Path, intent: &ReviewIntentRecord) -> RepoDeskResult<()> {
    let bytes = serde_json::to_vec_pretty(intent)?;
    if bytes.len() as u64 > MAX_REVIEW_INTENT_BYTES {
        return Err(routing_error(format!(
            "review intent exceeds the {MAX_REVIEW_INTENT_BYTES} byte limit"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| routing_error("review intent path has no parent directory"))?;
    fs::create_dir_all(parent)?;

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(routing_error(format!(
                "refusing to replace symlinked review intent: {}",
                path.display()
            )));
        }
        if !metadata.is_file() {
            return Err(routing_error(format!(
                "refusing to replace non-file review intent: {}",
                path.display()
            )));
        }
    }

    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| RepoDeskError::Io(error.error))?;
    sync_parent_directory(parent)?;
    Ok(())
}

fn remove_review_intent_file(path: &Path) -> RepoDeskResult<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !(metadata.is_file() || metadata.file_type().is_symlink()) {
        return Err(routing_error(format!(
            "refusing to remove non-file review intent: {}",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> RepoDeskResult<()> {
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> RepoDeskResult<()> {
    Ok(())
}

fn routing_error(detail: impl Into<String>) -> RepoDeskError {
    RepoDeskError::RoutingFailed {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn review_intent_round_trips_through_atomic_storage() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("run1.json");
        let intent = ReviewIntentRecord {
            run_id: "run1".into(),
            action: ReviewAction::Accept,
            changeset_digest: "digest".into(),
            recorded_at: "now".into(),
        };

        write_review_intent_file(&path, &intent).unwrap();
        assert_eq!(read_review_intent_file(&path).unwrap(), Some(intent));
        remove_review_intent_file(&path).unwrap();
        assert!(read_review_intent_file(&path).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_review_intent_is_rejected() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.json");
        fs::write(&target, b"{}").unwrap();
        let link = dir.path().join("run1.json");
        symlink(&target, &link).unwrap();

        assert!(read_review_intent_file(&link).is_err());
        let intent = ReviewIntentRecord {
            run_id: "run1".into(),
            action: ReviewAction::Reject,
            changeset_digest: "digest".into(),
            recorded_at: "now".into(),
        };
        assert!(write_review_intent_file(&link, &intent).is_err());
    }
}
