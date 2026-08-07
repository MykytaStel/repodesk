//! Evidence-backed acceptance criteria for RepoDesk Work Items.
//!
//! A criterion is never marked proven from agent prose or a heuristic. v0 only
//! accepts an explicit link to a command from the canonical VerificationReceipt.
//! That link becomes stale when the run, reviewed changeset, verification
//! receipt, or verified code tree changes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engineering::work_item_contract::{WorkItemContract, read_work_item_contract};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;
use crate::tasks::{TaskInfo, show_active_task};
use crate::workflow::{TaskRunReceipt, changeset_digest, head_sha, index_tree_sha, load_receipt};

pub const ACCEPTANCE_EVIDENCE_FILE: &str = "acceptance-evidence.json";
pub const ACCEPTANCE_EVIDENCE_VERSION: u32 = 1;
const MAX_BINDINGS: usize = 256;
const MAX_COMMAND_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceCriterionStatus {
    Unproven,
    Proven,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceEvidenceBinding {
    pub criterion_id: String,
    pub criterion: String,
    pub run_id: String,
    pub changeset_digest: String,
    pub verification_verified_at: String,
    pub command: String,
    pub success: bool,
    pub linked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceEvidenceStore {
    pub version: u32,
    pub project: String,
    pub work_item_id: String,
    #[serde(default)]
    pub bindings: Vec<AcceptanceEvidenceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCriterionEvidence {
    pub criterion_id: String,
    pub criterion: String,
    pub status: AcceptanceCriterionStatus,
    pub command: Option<String>,
    pub run_id: Option<String>,
    pub linked_at: Option<DateTime<Utc>>,
    pub stale: bool,
    pub stale_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceEvidenceReport {
    pub configured: bool,
    pub work_item_id: String,
    pub current_run_id: Option<String>,
    pub criteria: Vec<AcceptanceCriterionEvidence>,
    pub proven: usize,
    pub failed: usize,
    pub unproven: usize,
}

pub fn acceptance_evidence_path(run_dir: &Path) -> PathBuf {
    run_dir.join(ACCEPTANCE_EVIDENCE_FILE)
}

pub fn criterion_id(criterion: &str) -> String {
    let normalized = criterion
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)[..16].to_string()
}

pub fn read_acceptance_evidence(run_dir: &Path) -> RepoDeskResult<Option<AcceptanceEvidenceStore>> {
    let path = acceptance_evidence_path(run_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

/// True when the verification receipt still describes the exact code tree it
/// proved. Before commit this is the current HEAD + staged index tree. After a
/// RepoDesk bounded commit, HEAD necessarily changes, so the proof follows the
/// resulting commit only when that commit's tree is exactly the verified index
/// tree. This deliberately does not require overall verification success: a
/// failed command is still valid negative evidence for an acceptance criterion.
pub fn active_verification_is_fresh(receipt: &TaskRunReceipt) -> RepoDeskResult<bool> {
    let Some(verification) = receipt.verification.as_ref() else {
        return Ok(false);
    };
    if verification.run_id != receipt.run_id {
        return Ok(false);
    }

    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| changeset_digest(&[]));
    if verification.changeset_digest != digest {
        return Ok(false);
    }

    let project_path = get_active_project()?.path;
    if let Some(finish) = receipt.finish.as_ref() {
        if finish.run_id != receipt.run_id {
            return Ok(false);
        }
        let committed_tree = commit_tree_sha(&project_path, &finish.commit_sha);
        return Ok(committed_tree.as_deref() == Some(verification.index_tree_sha.as_str()));
    }

    let Some(head) = head_sha(&project_path) else {
        return Ok(false);
    };
    let Some(tree) = index_tree_sha(&project_path) else {
        return Ok(false);
    };
    Ok(verification.head_sha == head && verification.index_tree_sha == tree)
}

fn commit_tree_sha(project_path: &Path, commit_sha: &str) -> Option<String> {
    let revision = format!("{commit_sha}^{{tree}}");
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(["rev-parse", "--verify", revision.as_str()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn load_active_acceptance_evidence() -> RepoDeskResult<AcceptanceEvidenceReport> {
    let task = show_active_task()?;
    let contract = read_work_item_contract(&task.config.run_dir)?;
    let receipt = load_receipt()?;
    let fresh = match receipt.as_ref() {
        Some(receipt) => active_verification_is_fresh(receipt)?,
        None => false,
    };
    let store = read_acceptance_evidence(&task.config.run_dir)?;
    Ok(derive_acceptance_evidence(
        &task,
        contract.as_ref(),
        receipt.as_ref(),
        store.as_ref(),
        fresh,
    ))
}

pub fn link_active_acceptance_evidence(
    requested_criterion_id: &str,
    requested_command: &str,
) -> RepoDeskResult<AcceptanceEvidenceReport> {
    let task = show_active_task()?;
    let contract = read_work_item_contract(&task.config.run_dir)?.ok_or_else(|| {
        RepoDeskError::Api(
            "Configure an Engineering Contract before linking acceptance evidence".into(),
        )
    })?;
    let receipt = load_receipt()?.ok_or_else(|| {
        RepoDeskError::Api("No canonical run receipt exists for the active Work Item".into())
    })?;
    if !active_verification_is_fresh(&receipt)? {
        return Err(RepoDeskError::Api(
            "Verification is missing or stale; verify the current reviewed changeset before linking acceptance evidence".into(),
        ));
    }
    let verification = receipt.verification.as_ref().ok_or_else(|| {
        RepoDeskError::Api("Run verification before linking acceptance evidence".into())
    })?;

    let criterion = contract
        .acceptance_criteria
        .iter()
        .find(|criterion| criterion_id(criterion) == requested_criterion_id)
        .cloned()
        .ok_or_else(|| RepoDeskError::Api("Acceptance criterion no longer exists".into()))?;

    let command = requested_command.trim();
    if command.is_empty() || command.chars().count() > MAX_COMMAND_CHARS || command.contains('\0') {
        return Err(RepoDeskError::Api(
            "Verification command is empty or invalid".into(),
        ));
    }
    let check = verification
        .commands
        .iter()
        .find(|check| check.command == command)
        .ok_or_else(|| {
            RepoDeskError::Api(
                "The selected command is not part of the current VerificationReceipt".into(),
            )
        })?;

    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| changeset_digest(&[]));

    let mut store = read_acceptance_evidence(&task.config.run_dir)?.unwrap_or_else(|| {
        AcceptanceEvidenceStore {
            version: ACCEPTANCE_EVIDENCE_VERSION,
            project: task.config.project_name.clone(),
            work_item_id: task.config.id.clone(),
            bindings: Vec::new(),
        }
    });
    if store.version != ACCEPTANCE_EVIDENCE_VERSION
        || store.project != task.config.project_name
        || store.work_item_id != task.config.id
    {
        return Err(RepoDeskError::Api(
            "Acceptance evidence artifact does not match the active Work Item".into(),
        ));
    }

    store.bindings.retain(|binding| {
        !(binding.criterion_id == requested_criterion_id && binding.run_id == receipt.run_id)
    });
    store.bindings.push(AcceptanceEvidenceBinding {
        criterion_id: requested_criterion_id.to_string(),
        criterion,
        run_id: receipt.run_id.clone(),
        changeset_digest: digest,
        verification_verified_at: verification.verified_at.clone(),
        command: command.to_string(),
        success: check.success,
        linked_at: Utc::now(),
    });
    if store.bindings.len() > MAX_BINDINGS {
        let remove_count = store.bindings.len() - MAX_BINDINGS;
        store.bindings.drain(0..remove_count);
    }

    write_store(&task.config.run_dir, &store)?;
    Ok(derive_acceptance_evidence(
        &task,
        Some(&contract),
        Some(&receipt),
        Some(&store),
        true,
    ))
}

pub fn derive_acceptance_evidence(
    task: &TaskInfo,
    contract: Option<&WorkItemContract>,
    receipt: Option<&TaskRunReceipt>,
    store: Option<&AcceptanceEvidenceStore>,
    verification_fresh: bool,
) -> AcceptanceEvidenceReport {
    let Some(contract) = contract else {
        return AcceptanceEvidenceReport {
            configured: false,
            work_item_id: task.config.id.clone(),
            current_run_id: receipt.map(|receipt| receipt.run_id.clone()),
            criteria: Vec::new(),
            proven: 0,
            failed: 0,
            unproven: 0,
        };
    };

    let criteria = contract
        .acceptance_criteria
        .iter()
        .map(|criterion| derive_criterion(criterion, receipt, store, verification_fresh))
        .collect::<Vec<_>>();
    let proven = criteria
        .iter()
        .filter(|criterion| criterion.status == AcceptanceCriterionStatus::Proven)
        .count();
    let failed = criteria
        .iter()
        .filter(|criterion| criterion.status == AcceptanceCriterionStatus::Failed)
        .count();
    let unproven = criteria.len().saturating_sub(proven + failed);

    AcceptanceEvidenceReport {
        configured: true,
        work_item_id: task.config.id.clone(),
        current_run_id: receipt.map(|receipt| receipt.run_id.clone()),
        criteria,
        proven,
        failed,
        unproven,
    }
}

fn derive_criterion(
    criterion: &str,
    receipt: Option<&TaskRunReceipt>,
    store: Option<&AcceptanceEvidenceStore>,
    verification_fresh: bool,
) -> AcceptanceCriterionEvidence {
    let id = criterion_id(criterion);
    let binding = store
        .into_iter()
        .flat_map(|store| store.bindings.iter())
        .filter(|binding| binding.criterion_id == id)
        .max_by_key(|binding| binding.linked_at);

    let Some(binding) = binding else {
        return unproven(id, criterion, None, None, false, None);
    };
    if binding.criterion.trim() != criterion.trim() {
        return stale_unproven(id, criterion, binding, "criterion text changed");
    }
    let Some(receipt) = receipt else {
        return stale_unproven(
            id,
            criterion,
            binding,
            "canonical run receipt is unavailable",
        );
    };
    if binding.run_id != receipt.run_id {
        return stale_unproven(id, criterion, binding, "evidence belongs to another run");
    }
    let digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| changeset_digest(&[]));
    if binding.changeset_digest != digest {
        return stale_unproven(id, criterion, binding, "reviewed changeset changed");
    }
    let Some(verification) = receipt.verification.as_ref() else {
        return stale_unproven(
            id,
            criterion,
            binding,
            "verification receipt is unavailable",
        );
    };
    if binding.verification_verified_at != verification.verified_at {
        return stale_unproven(id, criterion, binding, "verification was replaced or rerun");
    }
    if !verification_fresh {
        return stale_unproven(
            id,
            criterion,
            binding,
            "verified code tree no longer matches",
        );
    }
    let Some(check) = verification
        .commands
        .iter()
        .find(|check| check.command == binding.command)
    else {
        return stale_unproven(
            id,
            criterion,
            binding,
            "linked command is absent from verification",
        );
    };

    AcceptanceCriterionEvidence {
        criterion_id: id,
        criterion: criterion.to_string(),
        status: if check.success {
            AcceptanceCriterionStatus::Proven
        } else {
            AcceptanceCriterionStatus::Failed
        },
        command: Some(binding.command.clone()),
        run_id: Some(binding.run_id.clone()),
        linked_at: Some(binding.linked_at),
        stale: false,
        stale_reason: None,
    }
}

fn stale_unproven(
    id: String,
    criterion: &str,
    binding: &AcceptanceEvidenceBinding,
    reason: &str,
) -> AcceptanceCriterionEvidence {
    AcceptanceCriterionEvidence {
        criterion_id: id,
        criterion: criterion.to_string(),
        status: AcceptanceCriterionStatus::Unproven,
        command: Some(binding.command.clone()),
        run_id: Some(binding.run_id.clone()),
        linked_at: Some(binding.linked_at),
        stale: true,
        stale_reason: Some(reason.to_string()),
    }
}

fn unproven(
    id: String,
    criterion: &str,
    command: Option<String>,
    run_id: Option<String>,
    stale: bool,
    stale_reason: Option<String>,
) -> AcceptanceCriterionEvidence {
    AcceptanceCriterionEvidence {
        criterion_id: id,
        criterion: criterion.to_string(),
        status: AcceptanceCriterionStatus::Unproven,
        command,
        run_id,
        linked_at: None,
        stale,
        stale_reason,
    }
}

fn write_store(run_dir: &Path, store: &AcceptanceEvidenceStore) -> RepoDeskResult<()> {
    let content = serde_json::to_string_pretty(store)?;
    fs::write(acceptance_evidence_path(run_dir), format!("{content}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::RunStatus;
    use crate::tasks::{TaskConfig, TaskStatus};
    use crate::workflow::{CheckReceipt, ExecutionMode, ExecutionReceipt, VerificationReceipt};

    fn task() -> TaskInfo {
        let now = Utc::now();
        TaskInfo {
            config: TaskConfig {
                id: "task-1".into(),
                project_name: "repodesk".into(),
                title: "Evidence".into(),
                status: TaskStatus::Open,
                verify_command: None,
                run_dir: PathBuf::from("/tmp/task-1"),
                created_at: now,
                updated_at: now,
            },
            task_file: PathBuf::from("/tmp/task-1/task.toml"),
            task_markdown_file: PathBuf::from("/tmp/task-1/task.md"),
        }
    }

    fn receipt(success: bool) -> TaskRunReceipt {
        TaskRunReceipt {
            task_id: "task-1".into(),
            run_id: "run-1".into(),
            execution_mode: ExecutionMode::AgentRun,
            base_commit: Some("base".into()),
            execution: ExecutionReceipt {
                status: RunStatus::Completed,
                required_steps: Vec::new(),
                changeset_digest: Some("digest-1".into()),
            },
            review: None,
            verification: Some(VerificationReceipt {
                run_id: "run-1".into(),
                head_sha: "head".into(),
                index_tree_sha: "tree".into(),
                changeset_digest: "digest-1".into(),
                commands: vec![CheckReceipt {
                    command: "cargo test".into(),
                    success,
                }],
                success,
                verified_at: "2026-08-07T18:00:00Z".into(),
            }),
            finish: None,
        }
    }

    fn contract() -> WorkItemContract {
        WorkItemContract {
            version: 1,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            goal: "Ship evidence".into(),
            allowed_paths: vec!["src".into()],
            protected_paths: Vec::new(),
            acceptance_criteria: vec!["Tests pass".into()],
            updated_at: Utc::now(),
        }
    }

    fn store() -> AcceptanceEvidenceStore {
        AcceptanceEvidenceStore {
            version: 1,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            bindings: vec![AcceptanceEvidenceBinding {
                criterion_id: criterion_id("Tests pass"),
                criterion: "Tests pass".into(),
                run_id: "run-1".into(),
                changeset_digest: "digest-1".into(),
                verification_verified_at: "2026-08-07T18:00:00Z".into(),
                command: "cargo test".into(),
                success: true,
                linked_at: Utc::now(),
            }],
        }
    }

    #[test]
    fn criterion_without_binding_is_unproven() {
        let report = derive_acceptance_evidence(
            &task(),
            Some(&contract()),
            Some(&receipt(true)),
            None,
            true,
        );
        assert_eq!(report.unproven, 1);
    }

    #[test]
    fn matching_fresh_passed_command_is_proven() {
        let report = derive_acceptance_evidence(
            &task(),
            Some(&contract()),
            Some(&receipt(true)),
            Some(&store()),
            true,
        );
        assert_eq!(report.proven, 1);
        assert_eq!(report.criteria[0].status, AcceptanceCriterionStatus::Proven);
    }

    #[test]
    fn stale_tree_makes_prior_proof_unproven() {
        let report = derive_acceptance_evidence(
            &task(),
            Some(&contract()),
            Some(&receipt(true)),
            Some(&store()),
            false,
        );
        assert_eq!(report.unproven, 1);
        assert!(report.criteria[0].stale);
    }

    #[test]
    fn failed_linked_command_is_failed_acceptance() {
        let mut receipt = receipt(false);
        let mut store = store();
        store.bindings[0].success = false;
        receipt.verification.as_mut().unwrap().success = false;
        let report = derive_acceptance_evidence(
            &task(),
            Some(&contract()),
            Some(&receipt),
            Some(&store),
            true,
        );
        assert_eq!(report.failed, 1);
    }
}
