//! Canonical pre-commit evidence contract for the active ChangeSet.
//!
//! This is a derived read model, not a new source of truth. It composes the
//! current TaskRunReceipt, exact Git tree, Engineering Contract scope decision,
//! and acceptance evidence into the one contract both Changes and Finish use.
//! A commit is safe only when this manifest is `ready`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::workflow::{
    CheckReceipt, ReviewDecision, TaskRunReceipt, commit_exists, head_sha, index_tree_sha,
    load_receipt, staged_paths,
};

use super::{
    AcceptanceEvidenceReport, CommitScopePolicyDecision, ScopeComplianceStatus,
    load_active_acceptance_evidence, load_active_commit_scope_policy,
};

pub const SAFE_COMMIT_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeCommitState {
    Blocked,
    Ready,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeCommitManifest {
    pub version: u32,
    pub work_item_id: String,
    pub run_id: Option<String>,
    pub changeset_digest: Option<String>,
    /// Parent HEAD against which VerificationReceipt ran.
    pub parent_head_sha: Option<String>,
    /// Live repository HEAD. After Finish this is the resulting commit SHA.
    pub current_head_sha: Option<String>,
    pub reviewed_tree_sha: Option<String>,
    pub verification_tree_sha: Option<String>,
    pub verification_verified_at: Option<String>,
    pub reviewed_paths: Vec<String>,
    /// Exact staged paths pre-commit; committed paths after a proven Finish.
    pub staged_paths: Vec<String>,
    pub verification_commands: Vec<CheckReceipt>,
    pub scope: CommitScopePolicyDecision,
    pub acceptance: AcceptanceEvidenceReport,
    pub commit_sha: Option<String>,
    pub state: SafeCommitState,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    /// Deterministic fingerprint of every evidence field above except itself.
    pub manifest_digest: String,
}

impl SafeCommitManifest {
    pub fn blocker_message(&self) -> Option<String> {
        self.blockers.first().cloned().or_else(|| {
            (!self.ready && self.state != SafeCommitState::Committed)
                .then(|| "commit blocked by Safe Commit Manifest".to_string())
        })
    }
}

#[derive(Serialize)]
struct ManifestDigestPayload<'a> {
    version: u32,
    work_item_id: &'a str,
    run_id: &'a Option<String>,
    changeset_digest: &'a Option<String>,
    parent_head_sha: &'a Option<String>,
    current_head_sha: &'a Option<String>,
    reviewed_tree_sha: &'a Option<String>,
    verification_tree_sha: &'a Option<String>,
    verification_verified_at: &'a Option<String>,
    reviewed_paths: &'a [String],
    staged_paths: &'a [String],
    verification_commands: &'a [CheckReceipt],
    scope: &'a CommitScopePolicyDecision,
    acceptance: &'a AcceptanceEvidenceReport,
    commit_sha: &'a Option<String>,
    state: SafeCommitState,
    ready: bool,
    blockers: &'a [String],
    warnings: &'a [String],
}

#[allow(clippy::too_many_arguments)]
pub fn derive_safe_commit_manifest(
    work_item_id: &str,
    receipt: Option<&TaskRunReceipt>,
    current_head_sha: Option<String>,
    current_index_tree: Option<String>,
    live_staged_paths: Vec<String>,
    committed_tree_sha: Option<String>,
    scope: CommitScopePolicyDecision,
    acceptance: AcceptanceEvidenceReport,
) -> RepoDeskResult<SafeCommitManifest> {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();

    if scope.status == ScopeComplianceStatus::Unconfigured {
        warnings.push(
            "Engineering Contract scope is not configured; legacy scope policy applies.".into(),
        );
    }
    if !acceptance.configured {
        warnings.push(
            "Acceptance criteria are not configured; legacy acceptance policy applies.".into(),
        );
    }

    let run_id = receipt.map(|value| value.run_id.clone());
    let changeset_digest = receipt.and_then(|value| value.execution.changeset_digest.clone());
    let review = receipt.and_then(|value| value.review.as_ref());
    let verification = receipt.and_then(|value| value.verification.as_ref());
    let finish = receipt.and_then(|value| value.finish.as_ref());

    let parent_head_sha = verification.map(|value| value.head_sha.clone());
    let reviewed_tree_sha = review.and_then(|value| value.index_tree_after_accept.clone());
    let verification_tree_sha = verification.map(|value| value.index_tree_sha.clone());
    let verification_verified_at = verification.map(|value| value.verified_at.clone());
    let verification_commands = verification
        .map(|value| value.commands.clone())
        .unwrap_or_default();
    let commit_sha = finish.map(|value| value.commit_sha.clone());

    let mut reviewed_paths = review
        .map(|value| value.reviewed_paths.clone())
        .unwrap_or_default();
    normalize_paths(&mut reviewed_paths);

    let mut staged_paths = if let Some(finish) = finish {
        finish.committed_paths.clone()
    } else {
        live_staged_paths
    };
    normalize_paths(&mut staged_paths);

    let committed = finish.is_some();
    if receipt.is_none() {
        blockers.push("No canonical run receipt exists for the active Work Item.".into());
    }

    if let Some(receipt) = receipt {
        let Some(digest) = changeset_digest.as_deref() else {
            blockers.push("No recorded ChangeSet exists for this run.".into());
            return finalize_manifest(
                work_item_id,
                run_id,
                changeset_digest,
                parent_head_sha,
                current_head_sha,
                reviewed_tree_sha,
                verification_tree_sha,
                verification_verified_at,
                reviewed_paths,
                staged_paths,
                verification_commands,
                scope,
                acceptance,
                commit_sha,
                committed,
                blockers,
                warnings,
            );
        };

        let review_valid = review.is_some_and(|value| {
            value.decision == ReviewDecision::Accepted
                && value.run_id == receipt.run_id
                && value.changeset_digest == digest
                && value
                    .index_tree_after_accept
                    .as_deref()
                    .is_some_and(|tree| !tree.is_empty())
        });
        if !review_valid {
            blockers.push("The exact current ChangeSet has not been reviewed and accepted.".into());
        }

        if committed {
            let finish_valid = finish.is_some_and(|value| value.run_id == receipt.run_id);
            if !finish_valid {
                blockers.push("Finish evidence belongs to a different run.".into());
            }
            if committed_tree_sha.as_deref() != reviewed_tree_sha.as_deref()
                || committed_tree_sha.as_deref() != verification_tree_sha.as_deref()
            {
                blockers.push(
                    "Committed tree does not match the exact reviewed and verified tree.".into(),
                );
            }
            if !scope.allowed {
                warnings.push(
                    "The historical commit no longer satisfies the current scope policy.".into(),
                );
            }
            if acceptance.configured && (acceptance.failed > 0 || acceptance.unproven > 0) {
                warnings.push(
                    "The historical commit does not have complete current acceptance evidence."
                        .into(),
                );
            }
        } else {
            match (current_head_sha.as_deref(), current_index_tree.as_deref()) {
                (Some(head), Some(tree)) => {
                    if reviewed_tree_sha.as_deref() != Some(tree) {
                        blockers.push(
                            "The staged index no longer matches the exact tree accepted in Review."
                                .into(),
                        );
                    }
                    let verified = verification.is_some_and(|value| {
                        value.run_id == receipt.run_id
                            && value.index_tree_sha == tree
                            && value.valid_for(head, tree, digest)
                    });
                    if !verified {
                        blockers.push(
                            "Verification is missing, failed, stale, or belongs to a different reviewed tree."
                                .into(),
                        );
                    }
                }
                _ => blockers.push(
                    "RepoDesk cannot resolve the current Git HEAD and staged index tree.".into(),
                ),
            }

            if !scope.allowed {
                blockers.push(
                    scope.blocker_message().unwrap_or_else(|| {
                        "Engineering Contract scope policy blocks commit.".into()
                    }),
                );
            }

            if staged_paths.is_empty() {
                blockers.push("Nothing is staged for the reviewed ChangeSet.".into());
            } else if path_set(&staged_paths) != path_set(&reviewed_paths) {
                blockers.push(
                    "The staged path set is not exactly the path set accepted in Review.".into(),
                );
            }

            if acceptance.configured {
                if acceptance.failed > 0 {
                    blockers.push(format!(
                        "{} acceptance criterion{} failed current verification.",
                        acceptance.failed,
                        if acceptance.failed == 1 { "" } else { "s" }
                    ));
                }
                if acceptance.unproven > 0 {
                    blockers.push(format!(
                        "{} acceptance criterion{} remain unproven or stale.",
                        acceptance.unproven,
                        if acceptance.unproven == 1 { "" } else { "s" }
                    ));
                }
            }
        }
    }

    finalize_manifest(
        work_item_id,
        run_id,
        changeset_digest,
        parent_head_sha,
        current_head_sha,
        reviewed_tree_sha,
        verification_tree_sha,
        verification_verified_at,
        reviewed_paths,
        staged_paths,
        verification_commands,
        scope,
        acceptance,
        commit_sha,
        committed,
        blockers,
        warnings,
    )
}

pub fn load_active_safe_commit_manifest() -> RepoDeskResult<SafeCommitManifest> {
    let task = crate::tasks::show_active_task()?;
    let project = crate::projects::get_active_project()?;
    let receipt = load_receipt()?;
    let acceptance = load_active_acceptance_evidence()?;

    let review = receipt.as_ref().and_then(|value| value.review.as_ref());
    let scope = if let (Some(receipt), Some(review)) = (receipt.as_ref(), review)
        && review.decision == ReviewDecision::Accepted
    {
        load_active_commit_scope_policy(&receipt.run_id, &review.reviewed_paths)?
    } else {
        CommitScopePolicyDecision {
            status: ScopeComplianceStatus::NotEvaluated,
            allowed: false,
            overridden: false,
            override_event_id: None,
            out_of_scope_files: Vec::new(),
            protected_files: Vec::new(),
        }
    };

    let current_head = head_sha(&project.path);
    let current_tree = index_tree_sha(&project.path);
    let staged = staged_paths(&project.path);
    let committed_tree = receipt
        .as_ref()
        .and_then(|value| value.finish.as_ref())
        .filter(|finish| commit_exists(&project.path, &finish.commit_sha))
        .and_then(|finish| {
            crate::workflow::receipt::commit_tree_sha(&project.path, &finish.commit_sha)
        });

    derive_safe_commit_manifest(
        &task.config.id,
        receipt.as_ref(),
        current_head,
        current_tree,
        staged,
        committed_tree,
        scope,
        acceptance,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_manifest(
    work_item_id: &str,
    run_id: Option<String>,
    changeset_digest: Option<String>,
    parent_head_sha: Option<String>,
    current_head_sha: Option<String>,
    reviewed_tree_sha: Option<String>,
    verification_tree_sha: Option<String>,
    verification_verified_at: Option<String>,
    reviewed_paths: Vec<String>,
    staged_paths: Vec<String>,
    verification_commands: Vec<CheckReceipt>,
    scope: CommitScopePolicyDecision,
    acceptance: AcceptanceEvidenceReport,
    commit_sha: Option<String>,
    committed: bool,
    blockers: Vec<String>,
    warnings: Vec<String>,
) -> RepoDeskResult<SafeCommitManifest> {
    let state = if committed && blockers.is_empty() {
        SafeCommitState::Committed
    } else if !committed && blockers.is_empty() {
        SafeCommitState::Ready
    } else {
        SafeCommitState::Blocked
    };
    let ready = state == SafeCommitState::Ready;

    let mut manifest = SafeCommitManifest {
        version: SAFE_COMMIT_MANIFEST_VERSION,
        work_item_id: work_item_id.to_string(),
        run_id,
        changeset_digest,
        parent_head_sha,
        current_head_sha,
        reviewed_tree_sha,
        verification_tree_sha,
        verification_verified_at,
        reviewed_paths,
        staged_paths,
        verification_commands,
        scope,
        acceptance,
        commit_sha,
        state,
        ready,
        blockers,
        warnings,
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = digest_manifest(&manifest)?;
    Ok(manifest)
}

fn digest_manifest(manifest: &SafeCommitManifest) -> RepoDeskResult<String> {
    let payload = ManifestDigestPayload {
        version: manifest.version,
        work_item_id: &manifest.work_item_id,
        run_id: &manifest.run_id,
        changeset_digest: &manifest.changeset_digest,
        parent_head_sha: &manifest.parent_head_sha,
        current_head_sha: &manifest.current_head_sha,
        reviewed_tree_sha: &manifest.reviewed_tree_sha,
        verification_tree_sha: &manifest.verification_tree_sha,
        verification_verified_at: &manifest.verification_verified_at,
        reviewed_paths: &manifest.reviewed_paths,
        staged_paths: &manifest.staged_paths,
        verification_commands: &manifest.verification_commands,
        scope: &manifest.scope,
        acceptance: &manifest.acceptance,
        commit_sha: &manifest.commit_sha,
        state: manifest.state,
        ready: manifest.ready,
        blockers: &manifest.blockers,
        warnings: &manifest.warnings,
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|error| RepoDeskError::Api(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn normalize_paths(paths: &mut Vec<String>) {
    paths.sort();
    paths.dedup();
}

fn path_set(paths: &[String]) -> BTreeSet<&str> {
    paths.iter().map(String::as_str).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::engineering::{AcceptanceCriterionEvidence, AcceptanceCriterionStatus};
    use crate::orchestrator::{RunStatus, SubAgentStatus};
    use crate::workflow::{
        ExecutionMode, ExecutionReceipt, ReviewReceipt, StepReceipt, VerificationReceipt,
    };

    fn receipt() -> TaskRunReceipt {
        TaskRunReceipt {
            task_id: "task-1".into(),
            run_id: "run-1".into(),
            execution_mode: ExecutionMode::AgentRun,
            base_commit: Some("base".into()),
            execution: ExecutionReceipt {
                status: RunStatus::Completed,
                required_steps: vec![StepReceipt {
                    task_id: "implement".into(),
                    status: SubAgentStatus::Ok,
                    allow_write: true,
                    changed_files: vec!["src/lib.rs".into()],
                }],
                changeset_digest: Some("digest-1".into()),
            },
            review: Some(ReviewReceipt {
                run_id: "run-1".into(),
                decision: ReviewDecision::Accepted,
                reviewed_paths: vec!["src/lib.rs".into()],
                changeset_digest: "digest-1".into(),
                index_tree_after_accept: Some("tree-1".into()),
            }),
            verification: Some(VerificationReceipt {
                run_id: "run-1".into(),
                head_sha: "head-1".into(),
                index_tree_sha: "tree-1".into(),
                changeset_digest: "digest-1".into(),
                commands: vec![CheckReceipt {
                    command: "cargo test".into(),
                    success: true,
                }],
                success: true,
                verified_at: "2026-08-14T00:00:00Z".into(),
            }),
            finish: None,
        }
    }

    fn scope() -> CommitScopePolicyDecision {
        CommitScopePolicyDecision {
            status: ScopeComplianceStatus::Compliant,
            allowed: true,
            overridden: false,
            override_event_id: None,
            out_of_scope_files: Vec::new(),
            protected_files: Vec::new(),
        }
    }

    fn acceptance(status: AcceptanceCriterionStatus) -> AcceptanceEvidenceReport {
        let (proven, failed, unproven) = match status {
            AcceptanceCriterionStatus::Proven => (1, 0, 0),
            AcceptanceCriterionStatus::Failed => (0, 1, 0),
            AcceptanceCriterionStatus::Unproven => (0, 0, 1),
        };
        AcceptanceEvidenceReport {
            configured: true,
            work_item_id: "task-1".into(),
            current_run_id: Some("run-1".into()),
            criteria: vec![AcceptanceCriterionEvidence {
                criterion_id: "criterion-1".into(),
                criterion: "Tests demonstrate the behavior".into(),
                status,
                command: Some("cargo test".into()),
                run_id: Some("run-1".into()),
                linked_at: Some(Utc::now()),
                stale: status == AcceptanceCriterionStatus::Unproven,
                stale_reason: (status == AcceptanceCriterionStatus::Unproven)
                    .then(|| "verification changed".into()),
            }],
            proven,
            failed,
            unproven,
        }
    }

    #[test]
    fn exact_review_verification_scope_and_acceptance_are_ready() {
        let manifest = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-1".into()),
            vec!["src/lib.rs".into()],
            None,
            scope(),
            acceptance(AcceptanceCriterionStatus::Proven),
        )
        .unwrap();
        assert_eq!(manifest.state, SafeCommitState::Ready);
        assert!(manifest.ready);
        assert!(manifest.blockers.is_empty());
    }

    #[test]
    fn unproven_acceptance_blocks_commit() {
        let manifest = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-1".into()),
            vec!["src/lib.rs".into()],
            None,
            scope(),
            acceptance(AcceptanceCriterionStatus::Unproven),
        )
        .unwrap();
        assert!(!manifest.ready);
        assert!(
            manifest
                .blockers
                .iter()
                .any(|value| value.contains("unproven"))
        );
    }

    #[test]
    fn stale_tree_and_stray_path_block_commit() {
        let manifest = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-2".into()),
            vec!["README.md".into(), "src/lib.rs".into()],
            None,
            scope(),
            acceptance(AcceptanceCriterionStatus::Proven),
        )
        .unwrap();
        assert!(!manifest.ready);
        assert!(
            manifest
                .blockers
                .iter()
                .any(|value| value.contains("staged index"))
        );
        assert!(
            manifest
                .blockers
                .iter()
                .any(|value| value.contains("path set"))
        );
    }

    #[test]
    fn unconfigured_acceptance_is_backward_compatible_warning() {
        let mut report = acceptance(AcceptanceCriterionStatus::Unproven);
        report.configured = false;
        report.criteria.clear();
        report.proven = 0;
        report.unproven = 0;
        let manifest = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-1".into()),
            vec!["src/lib.rs".into()],
            None,
            scope(),
            report,
        )
        .unwrap();
        assert!(manifest.ready);
        assert!(
            manifest
                .warnings
                .iter()
                .any(|value| value.contains("Acceptance criteria"))
        );
    }

    #[test]
    fn digest_changes_when_acceptance_evidence_changes() {
        let a = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-1".into()),
            vec!["src/lib.rs".into()],
            None,
            scope(),
            acceptance(AcceptanceCriterionStatus::Proven),
        )
        .unwrap();
        let b = derive_safe_commit_manifest(
            "task-1",
            Some(&receipt()),
            Some("head-1".into()),
            Some("tree-1".into()),
            vec!["src/lib.rs".into()],
            None,
            scope(),
            acceptance(AcceptanceCriterionStatus::Failed),
        )
        .unwrap();
        assert_ne!(a.manifest_digest, b.manifest_digest);
    }
}
