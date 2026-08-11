//! Durable recovery boundary around the bounded Finish commit.
//!
//! `finish::commit_reviewed_index` already proves that Review, Verification, the
//! staged index, and the created commit all bind to one exact tree. This module
//! adds the missing crash/persistence boundary around that operation: a durable
//! intent is written before `git commit`, and a retry can recover Finish evidence
//! for the exact commit instead of creating a second commit.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::finish::{self, CommitOutcome};
use super::receipt::{
    FinishReceipt, ReviewDecision, TaskRunReceipt, commit_exists, commit_tree_sha, head_sha,
    index_tree_sha, load_receipt, reviewed_tree_sha_for, save_receipt, staged_paths,
};

const MAX_FINISH_INTENT_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FinishIntentRecord {
    run_id: String,
    parent_head_sha: String,
    reviewed_tree_sha: String,
    changeset_digest: String,
    committed_paths: Vec<String>,
    #[serde(default)]
    commit_sha: Option<String>,
    recorded_at: String,
}

/// Commit the reviewed index through a durable, retry-safe Finish boundary.
///
/// A pending intent is checked before any new commit is attempted. If the exact
/// reviewed commit already exists, FinishReceipt is repaired from that evidence;
/// RepoDesk never issues a second `git commit` for the same pending Finish.
pub fn commit_reviewed_index(message: &str) -> RepoDeskResult<CommitOutcome> {
    let project_path = crate::projects::get_active_project()?.path;
    let mut receipt =
        load_receipt()?.ok_or_else(|| routing_error("no run to commit — run the agent first"))?;
    let intent_path = finish_intent_path()?;

    if let Some(outcome) = existing_finish_outcome(&receipt, &project_path)? {
        let _ = remove_finish_intent_file(&intent_path);
        return Ok(outcome);
    }

    if let Some(existing) = read_finish_intent_file(&intent_path)? {
        validate_intent_against_receipt(&existing, &receipt)?;
        if let Some(commit_sha) = recoverable_commit_sha(&project_path, &intent_path, &existing)? {
            return persist_recovered_finish(
                &project_path,
                &intent_path,
                &mut receipt,
                &existing,
                &commit_sha,
            );
        }

        let current = prepare_finish_intent(&receipt, &project_path)?;
        if !same_precommit_boundary(&existing, &current) {
            return Err(routing_error(
                "finish blocked: the durable pending Finish intent no longer matches the current reviewed and verified boundary",
            ));
        }
    } else {
        let prepared = prepare_finish_intent(&receipt, &project_path)?;
        write_finish_intent_file(&intent_path, &prepared)?;
        let persisted = read_finish_intent_file(&intent_path)?.ok_or_else(|| {
            routing_error("finish intent write completed but the durable record is missing")
        })?;
        if persisted != prepared {
            return Err(routing_error(
                "finish intent verification failed after persistence",
            ));
        }
    }

    match finish::commit_reviewed_index(message) {
        Ok(outcome) => {
            // FinishReceipt is already authoritative. Intent cleanup is best
            // effort; a later idempotent Finish call can remove a stale intent.
            let _ = remove_finish_intent_file(&intent_path);
            Ok(outcome)
        }
        Err(primary) => {
            // The inner Finish may have failed before commit, or after a real
            // commit when evidence persistence failed. Inspect the durable
            // boundary before deciding whether a retry would be safe.
            let Some(intent) = read_finish_intent_file(&intent_path)? else {
                return Err(primary);
            };

            match recoverable_commit_sha(&project_path, &intent_path, &intent) {
                Ok(Some(commit_sha)) => {
                    let mut latest_receipt = load_receipt()?.ok_or_else(|| {
                        routing_error(format!(
                            "commit {commit_sha} exists but the workflow receipt is missing; Finish evidence cannot be repaired automatically"
                        ))
                    })?;
                    match persist_recovered_finish(
                        &project_path,
                        &intent_path,
                        &mut latest_receipt,
                        &intent,
                        &commit_sha,
                    ) {
                        Ok(outcome) => Ok(outcome),
                        Err(recovery) => Err(routing_error(format!(
                            "commit {commit_sha} already exists, but Finish evidence persistence failed ({recovery}). Retry Finish to recover evidence without creating another commit"
                        ))),
                    }
                }
                Ok(None) => {
                    // HEAD never left the pre-commit parent, so no repository
                    // side effect needs recovery. Do not let a harmless failed
                    // attempt strand the workflow behind a stale intent.
                    let _ = remove_finish_intent_file(&intent_path);
                    Err(primary)
                }
                Err(recovery) => Err(routing_error(format!(
                    "Finish failed ({primary}); recovery also failed ({recovery}). A durable Finish intent remains pending and RepoDesk will not create another commit until this boundary is resolved"
                ))),
            }
        }
    }
}

fn prepare_finish_intent(
    receipt: &TaskRunReceipt,
    project_path: &Path,
) -> RepoDeskResult<FinishIntentRecord> {
    let review = receipt
        .review
        .as_ref()
        .filter(|review| review.decision == ReviewDecision::Accepted)
        .filter(|review| review.run_id == receipt.run_id)
        .ok_or_else(|| {
            routing_error(
                "commit blocked: the run's exact changes have not been reviewed and accepted",
            )
        })?;
    let digest = receipt
        .execution
        .changeset_digest
        .as_deref()
        .filter(|digest| *digest == review.changeset_digest)
        .ok_or_else(|| {
            routing_error("commit blocked: accepted review does not match the run changeset")
        })?
        .to_string();

    let parent_head_sha = head_sha(project_path)
        .ok_or_else(|| routing_error("active project is not a git repository with a commit"))?;
    let current_tree = index_tree_sha(project_path)
        .ok_or_else(|| routing_error("could not read the staged index tree"))?;
    let reviewed_tree_sha = reviewed_tree_sha_for(receipt, &current_tree)?.ok_or_else(|| {
        routing_error("commit blocked: no exact reviewed tree exists for this changeset")
    })?;

    let verified = receipt
        .verification
        .as_ref()
        .map(|verification| {
            verification.run_id == receipt.run_id
                && verification.index_tree_sha == reviewed_tree_sha
                && verification.valid_for(&parent_head_sha, &current_tree, &digest)
        })
        .unwrap_or(false);
    if !verified {
        return Err(routing_error(
            "commit blocked: verification is missing, stale, or belongs to a different reviewed tree — run verification again",
        ));
    }

    let mut committed_paths = staged_paths(project_path);
    if committed_paths.is_empty() {
        return Err(routing_error(
            "commit blocked: nothing is staged — accept the reviewed changes first",
        ));
    }
    committed_paths.sort();
    committed_paths.dedup();

    let reviewed: HashSet<&str> = review.reviewed_paths.iter().map(String::as_str).collect();
    let stray: Vec<String> = committed_paths
        .iter()
        .filter(|path| !reviewed.contains(path.as_str()))
        .cloned()
        .collect();
    if !stray.is_empty() {
        return Err(routing_error(format!(
            "commit blocked: the index holds files outside the reviewed changeset: {}",
            stray.join(", ")
        )));
    }

    Ok(FinishIntentRecord {
        run_id: receipt.run_id.clone(),
        parent_head_sha,
        reviewed_tree_sha,
        changeset_digest: digest,
        committed_paths,
        commit_sha: None,
        recorded_at: Utc::now().to_rfc3339(),
    })
}

fn validate_intent_against_receipt(
    intent: &FinishIntentRecord,
    receipt: &TaskRunReceipt,
) -> RepoDeskResult<()> {
    if intent.run_id != receipt.run_id {
        return Err(routing_error(format!(
            "finish blocked: unresolved Finish intent belongs to prior run '{}'; resolve that boundary before committing run '{}'",
            intent.run_id, receipt.run_id
        )));
    }
    if receipt.execution.changeset_digest.as_deref() != Some(intent.changeset_digest.as_str()) {
        return Err(routing_error(
            "finish blocked: pending Finish intent does not match the current run changeset",
        ));
    }

    let review = receipt
        .review
        .as_ref()
        .filter(|review| review.decision == ReviewDecision::Accepted)
        .filter(|review| review.run_id == receipt.run_id)
        .filter(|review| review.changeset_digest == intent.changeset_digest)
        .filter(|review| {
            review.index_tree_after_accept.as_deref() == Some(intent.reviewed_tree_sha.as_str())
        })
        .ok_or_else(|| {
            routing_error(
                "finish blocked: pending Finish intent no longer has matching Accepted review evidence",
            )
        })?;

    let intent_paths: HashSet<&str> = intent.committed_paths.iter().map(String::as_str).collect();
    let reviewed_paths: HashSet<&str> = review.reviewed_paths.iter().map(String::as_str).collect();
    if intent_paths != reviewed_paths {
        return Err(routing_error(
            "finish blocked: pending Finish intent path set does not match the reviewed changeset",
        ));
    }

    let verified = receipt
        .verification
        .as_ref()
        .map(|verification| {
            verification.run_id == receipt.run_id
                && verification.valid_for(
                    &intent.parent_head_sha,
                    &intent.reviewed_tree_sha,
                    &intent.changeset_digest,
                )
        })
        .unwrap_or(false);
    if !verified {
        return Err(routing_error(
            "finish blocked: pending Finish intent is not backed by the original successful verification",
        ));
    }

    Ok(())
}

fn same_precommit_boundary(a: &FinishIntentRecord, b: &FinishIntentRecord) -> bool {
    a.run_id == b.run_id
        && a.parent_head_sha == b.parent_head_sha
        && a.reviewed_tree_sha == b.reviewed_tree_sha
        && a.changeset_digest == b.changeset_digest
        && a.committed_paths == b.committed_paths
        && a.commit_sha.is_none()
}

fn recoverable_commit_sha(
    project_path: &Path,
    intent_path: &Path,
    intent: &FinishIntentRecord,
) -> RepoDeskResult<Option<String>> {
    if let Some(commit_sha) = intent.commit_sha.as_deref() {
        validate_recovery_candidate(project_path, intent, commit_sha)?;
        return Ok(Some(commit_sha.to_string()));
    }

    let current_head = head_sha(project_path)
        .ok_or_else(|| routing_error("could not read HEAD while recovering pending Finish"))?;
    if current_head == intent.parent_head_sha {
        return Ok(None);
    }

    validate_recovery_candidate(project_path, intent, &current_head)?;

    // Promote Prepared → Committed before attempting FinishReceipt persistence.
    // If that later write fails, the next retry is bound to one exact commit SHA.
    let mut committed = intent.clone();
    committed.commit_sha = Some(current_head.clone());
    write_finish_intent_file(intent_path, &committed)?;
    let persisted = read_finish_intent_file(intent_path)?
        .ok_or_else(|| routing_error("committed Finish intent disappeared after persistence"))?;
    if persisted != committed {
        return Err(routing_error(
            "committed Finish intent verification failed after persistence",
        ));
    }

    Ok(Some(current_head))
}

fn validate_recovery_candidate(
    project_path: &Path,
    intent: &FinishIntentRecord,
    commit_sha: &str,
) -> RepoDeskResult<()> {
    if !commit_exists(project_path, commit_sha) {
        return Err(routing_error(format!(
            "pending Finish references commit {commit_sha}, but that commit object does not exist"
        )));
    }

    let committed_tree = commit_tree_sha(project_path, commit_sha).ok_or_else(|| {
        routing_error(format!(
            "commit {commit_sha} exists but its tree could not be resolved during Finish recovery"
        ))
    })?;
    if committed_tree != intent.reviewed_tree_sha {
        return Err(routing_error(format!(
            "Finish recovery blocked: commit {commit_sha} has tree {committed_tree}, expected reviewed tree {}",
            intent.reviewed_tree_sha
        )));
    }

    let parents = commit_parent_shas(project_path, commit_sha)?;
    if parents.as_slice() != [intent.parent_head_sha.as_str()] {
        return Err(routing_error(format!(
            "Finish recovery blocked: commit {commit_sha} is not the single child of verified parent {}",
            intent.parent_head_sha
        )));
    }

    Ok(())
}

fn persist_recovered_finish(
    project_path: &Path,
    intent_path: &Path,
    receipt: &mut TaskRunReceipt,
    intent: &FinishIntentRecord,
    commit_sha: &str,
) -> RepoDeskResult<CommitOutcome> {
    validate_intent_against_receipt(intent, receipt)?;
    validate_recovery_candidate(project_path, intent, commit_sha)?;

    receipt.finish = Some(FinishReceipt {
        run_id: receipt.run_id.clone(),
        commit_sha: commit_sha.to_string(),
        committed_paths: intent.committed_paths.clone(),
        finished_at: Utc::now().to_rfc3339(),
    });
    save_receipt(receipt)?;

    if let Ok(task) = crate::tasks::show_active_task() {
        let _ = crate::engineering::instrumentation::record_commit_created(
            &task.config,
            &receipt.run_id,
            commit_sha,
            &intent.committed_paths,
        );
    }

    let _ = remove_finish_intent_file(intent_path);
    Ok(CommitOutcome {
        commit_sha: commit_sha.to_string(),
        committed_paths: intent.committed_paths.clone(),
    })
}

fn existing_finish_outcome(
    receipt: &TaskRunReceipt,
    project_path: &Path,
) -> RepoDeskResult<Option<CommitOutcome>> {
    let Some(finish) = receipt.finish.as_ref() else {
        return Ok(None);
    };
    if finish.run_id != receipt.run_id {
        return Err(routing_error(
            "finish receipt does not belong to the current run",
        ));
    }
    if !commit_exists(project_path, &finish.commit_sha) {
        return Err(routing_error(format!(
            "Finish receipt references missing commit {}; workflow completion cannot be trusted",
            finish.commit_sha
        )));
    }
    Ok(Some(CommitOutcome {
        commit_sha: finish.commit_sha.clone(),
        committed_paths: finish.committed_paths.clone(),
    }))
}

fn finish_intent_path() -> RepoDeskResult<PathBuf> {
    Ok(crate::tasks::show_active_task()?
        .config
        .run_dir
        .join("finish-intent.json"))
}

fn validate_finish_intent(intent: &FinishIntentRecord) -> RepoDeskResult<()> {
    validate_run_id(&intent.run_id)?;
    for (label, value) in [
        ("parent HEAD", intent.parent_head_sha.as_str()),
        ("reviewed tree", intent.reviewed_tree_sha.as_str()),
        ("changeset digest", intent.changeset_digest.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(routing_error(format!("finish intent has an empty {label}")));
        }
    }
    if intent.committed_paths.is_empty()
        || intent
            .committed_paths
            .iter()
            .any(|path| path.trim().is_empty())
    {
        return Err(routing_error(
            "finish intent must contain a non-empty committed path set",
        ));
    }
    let mut normalized = intent.committed_paths.clone();
    normalized.sort();
    normalized.dedup();
    if normalized != intent.committed_paths {
        return Err(routing_error(
            "finish intent committed paths must be sorted and unique",
        ));
    }
    if intent
        .commit_sha
        .as_deref()
        .is_some_and(|sha| sha.trim().is_empty())
    {
        return Err(routing_error("finish intent has an empty commit SHA"));
    }
    Ok(())
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
            "invalid orchestration run id for Finish intent",
        ))
    }
}

fn read_finish_intent_file(path: &Path) -> RepoDeskResult<Option<FinishIntentRecord>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(routing_error(format!(
            "finish intent path is a symlink: {}",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(routing_error(format!(
            "finish intent path is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_FINISH_INTENT_BYTES {
        return Err(routing_error(format!(
            "finish intent exceeds the {MAX_FINISH_INTENT_BYTES} byte limit"
        )));
    }

    let bytes = fs::read(path)?;
    let intent: FinishIntentRecord = serde_json::from_slice(&bytes).map_err(|error| {
        routing_error(format!(
            "finish intent is corrupt or invalid JSON at {}: {error}",
            path.display()
        ))
    })?;
    validate_finish_intent(&intent)?;
    Ok(Some(intent))
}

fn write_finish_intent_file(path: &Path, intent: &FinishIntentRecord) -> RepoDeskResult<()> {
    validate_finish_intent(intent)?;
    let bytes = serde_json::to_vec_pretty(intent)?;
    if bytes.len() as u64 > MAX_FINISH_INTENT_BYTES {
        return Err(routing_error(format!(
            "finish intent exceeds the {MAX_FINISH_INTENT_BYTES} byte limit"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| routing_error("finish intent path has no parent directory"))?;
    fs::create_dir_all(parent)?;

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(routing_error(format!(
                "refusing to replace symlinked finish intent: {}",
                path.display()
            )));
        }
        if !metadata.is_file() {
            return Err(routing_error(format!(
                "refusing to replace non-file finish intent: {}",
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

fn remove_finish_intent_file(path: &Path) -> RepoDeskResult<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !(metadata.is_file() || metadata.file_type().is_symlink()) {
        return Err(routing_error(format!(
            "refusing to remove non-file finish intent: {}",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

fn commit_parent_shas(project_path: &Path, commit_sha: &str) -> RepoDeskResult<Vec<String>> {
    let output = Command::new("git")
        .args(["show", "-s", "--format=%P", commit_sha])
        .current_dir(project_path)
        .output()
        .map_err(|error| routing_error(format!("could not inspect commit parents: {error}")))?;
    if !output.status.success() {
        return Err(routing_error(format!(
            "could not inspect parents for commit {commit_sha}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect())
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

    fn intent() -> FinishIntentRecord {
        FinishIntentRecord {
            run_id: "run1".into(),
            parent_head_sha: "parent".into(),
            reviewed_tree_sha: "tree".into(),
            changeset_digest: "digest".into(),
            committed_paths: vec!["src/a.rs".into()],
            commit_sha: None,
            recorded_at: "now".into(),
        }
    }

    #[test]
    fn finish_intent_round_trips_through_atomic_storage() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("finish-intent.json");
        let intent = intent();

        write_finish_intent_file(&path, &intent).unwrap();
        assert_eq!(read_finish_intent_file(&path).unwrap(), Some(intent));
        remove_finish_intent_file(&path).unwrap();
        assert!(read_finish_intent_file(&path).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_finish_intent_is_rejected() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.json");
        fs::write(&target, b"{}").unwrap();
        let link = dir.path().join("finish-intent.json");
        symlink(&target, &link).unwrap();

        assert!(read_finish_intent_file(&link).is_err());
        assert!(write_finish_intent_file(&link, &intent()).is_err());
    }
}
