//! Typed RepoDesk 2 engineering domain vocabulary.
//!
//! This module intentionally adapts the existing task/orchestrator models rather
//! than replacing them. The goal is to introduce stable identities and shared
//! engineering concepts that later UI, event-ledger, and intelligence slices can
//! depend on while legacy APIs keep working.

use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::orchestrator::types::{OrchestrationRun, RunStatus, SubAgentResult};
use crate::tasks::{TaskConfig, TaskStatus};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineeringDomainError {
    #[error("{kind} id must not be empty")]
    EmptyId { kind: &'static str },
    #[error("{kind} id must not contain leading/trailing whitespace or control characters")]
    InvalidId { kind: &'static str },
    #[error("evidence locator must not be empty")]
    EmptyEvidenceLocator,
}

fn validate_id(kind: &'static str, value: &str) -> Result<(), EngineeringDomainError> {
    if value.trim().is_empty() {
        return Err(EngineeringDomainError::EmptyId { kind });
    }
    if value.trim() != value || value.chars().any(char::is_control) {
        return Err(EngineeringDomainError::InvalidId { kind });
    }
    Ok(())
}

macro_rules! typed_id {
    ($name:ident, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, EngineeringDomainError> {
                let value = value.into();
                validate_id($kind, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

typed_id!(WorkItemId, "work item");
typed_id!(ExecutionId, "execution");
typed_id!(ChangeSetId, "changeset");
typed_id!(VerificationId, "verification");
typed_id!(EngineeringKnowledgeId, "engineering knowledge");
typed_id!(EngineeringEventId, "engineering event");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub project: String,
    pub title: String,
    pub state: WorkItemState,
    pub verify_command: Option<String>,
    pub run_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<&TaskConfig> for WorkItem {
    type Error = EngineeringDomainError;

    fn try_from(task: &TaskConfig) -> Result<Self, Self::Error> {
        let state = match &task.status {
            TaskStatus::Open => WorkItemState::Open,
            TaskStatus::Closed => WorkItemState::Closed,
        };

        Ok(Self {
            id: WorkItemId::try_new(task.id.clone())?,
            project: task.project_name.clone(),
            title: task.title.clone(),
            state,
            verify_command: task.verify_command.clone(),
            run_dir: task.run_dir.clone(),
            created_at: task.created_at,
            updated_at: task.updated_at,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerKind {
    Human,
    CodingAgent,
    Inference,
    CheckRunner,
    Script,
    Ci,
    Manual,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkerRef {
    pub kind: WorkerKind,
    pub id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl WorkerRef {
    pub fn from_legacy_result(result: &SubAgentResult) -> Self {
        let agent = result.agent.trim();
        let provider = result.provider.trim();
        let agent_lower = agent.to_ascii_lowercase();
        let provider_lower = provider.to_ascii_lowercase();

        let kind = match agent_lower.as_str() {
            "manual" => WorkerKind::Manual,
            "local_checks" | "check_runner" => WorkerKind::CheckRunner,
            "codex" | "codex_cli" | "claude" | "claude_code" | "claude_code_cli" => {
                WorkerKind::CodingAgent
            }
            _ if matches!(
                provider_lower.as_str(),
                "openai"
                    | "openai_api"
                    | "anthropic"
                    | "anthropic_api"
                    | "gemini"
                    | "gemini_api"
                    | "ollama"
                    | "lm_studio"
                    | "llamafile"
                    | "localai"
            ) =>
            {
                WorkerKind::Inference
            }
            _ => WorkerKind::Unknown,
        };

        let id = if !agent.is_empty() {
            agent.to_string()
        } else if !provider.is_empty() {
            provider.to_string()
        } else {
            "unknown".to_string()
        };

        Self {
            kind,
            id,
            provider: (!provider.is_empty()).then(|| provider.to_string()),
            model: (!result.model.trim().is_empty()).then(|| result.model.trim().to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Completed,
    Partial,
    Failed,
    DryRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub id: ExecutionId,
    pub work_item_id: WorkItemId,
    pub project: String,
    pub goal: String,
    pub status: ExecutionStatus,
    pub dry_run: bool,
    pub started_at: String,
    pub finished_at: String,
    pub workers: Vec<WorkerRef>,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_cost_units: f64,
}

impl TryFrom<&OrchestrationRun> for Execution {
    type Error = EngineeringDomainError;

    fn try_from(run: &OrchestrationRun) -> Result<Self, Self::Error> {
        let status = match run.status {
            RunStatus::Completed => ExecutionStatus::Completed,
            RunStatus::Partial => ExecutionStatus::Partial,
            RunStatus::Failed => ExecutionStatus::Failed,
            RunStatus::DryRun => ExecutionStatus::DryRun,
        };

        let workers: BTreeSet<WorkerRef> = run
            .results
            .iter()
            .map(WorkerRef::from_legacy_result)
            .collect();

        Ok(Self {
            id: ExecutionId::try_new(run.run_id.clone())?,
            work_item_id: WorkItemId::try_new(run.task_id.clone())?,
            project: run.project.clone(),
            goal: run.goal.clone(),
            status,
            dry_run: run.dry_run,
            started_at: run.started_at.clone(),
            finished_at: run.finished_at.clone(),
            workers: workers.into_iter().collect(),
            total_input_tokens: run.total_input_tokens,
            total_output_tokens: run.total_output_tokens,
            total_cost_units: run.total_cost_units,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetStatus {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Context,
    RunReceipt,
    Diff,
    Verification,
    Commit,
    Knowledge,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: EvidenceKind,
    pub locator: String,
}

impl EvidenceRef {
    pub fn try_new(
        kind: EvidenceKind,
        locator: impl Into<String>,
    ) -> Result<Self, EngineeringDomainError> {
        let locator = locator.into();
        if locator.trim().is_empty() {
            return Err(EngineeringDomainError::EmptyEvidenceLocator);
        }
        Ok(Self { kind, locator })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub id: ChangeSetId,
    pub work_item_id: WorkItemId,
    pub execution_id: ExecutionId,
    pub status: ChangeSetStatus,
    pub files: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
}

impl ChangeSet {
    /// Build the initial reviewable changeset projected from an orchestration
    /// run. Review state is deliberately `Proposed`; accept/reject adapters can
    /// update it later when the legacy review path is migrated.
    pub fn try_from_run(run: &OrchestrationRun) -> Result<Option<Self>, EngineeringDomainError> {
        let files: BTreeSet<String> = run
            .results
            .iter()
            .flat_map(|result| result.changed_files.iter().cloned())
            .collect();

        if files.is_empty() {
            return Ok(None);
        }

        let evidence: BTreeSet<EvidenceRef> = run
            .results
            .iter()
            .filter_map(|result| result.diff_path.as_ref())
            .map(|path| EvidenceRef::try_new(EvidenceKind::Diff, path.clone()))
            .collect::<Result<_, _>>()?;

        Ok(Some(Self {
            id: ChangeSetId::try_new(format!("{}-changeset", run.run_id))?,
            work_item_id: WorkItemId::try_new(run.task_id.clone())?,
            execution_id: ExecutionId::try_new(run.run_id.clone())?,
            status: ChangeSetStatus::Proposed,
            files: files.into_iter().collect(),
            evidence: evidence.into_iter().collect(),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub name: String,
    pub status: VerificationStatus,
    pub evidence: Option<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReceipt {
    pub id: VerificationId,
    pub work_item_id: WorkItemId,
    pub changeset_id: Option<ChangeSetId>,
    pub status: VerificationStatus,
    pub checks: Vec<VerificationCheck>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledge {
    pub id: EngineeringKnowledgeId,
    pub project: String,
    pub category: String,
    pub content: String,
    pub evidence: Vec<EvidenceRef>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::SubAgentStatus;
    use tempfile::tempdir;

    #[test]
    fn typed_ids_reject_blank_and_invalid_values() {
        assert!(matches!(
            WorkItemId::try_new("   "),
            Err(EngineeringDomainError::EmptyId { .. })
        ));
        assert!(matches!(
            ExecutionId::try_new(" run-1 "),
            Err(EngineeringDomainError::InvalidId { .. })
        ));
        assert!(WorkItemId::try_new("task-2026-08-07").is_ok());
    }

    #[test]
    fn typed_id_deserialization_enforces_validation() {
        let invalid = serde_json::from_str::<WorkItemId>("\"  \"");
        assert!(invalid.is_err());

        let valid: WorkItemId = serde_json::from_str("\"task-1\"").unwrap();
        assert_eq!(valid.as_str(), "task-1");
    }

    #[test]
    fn task_config_adapts_to_work_item_without_changing_legacy_model() {
        let dir = tempdir().unwrap();
        let now = Utc::now();
        let task = TaskConfig {
            id: "task-1".to_string(),
            project_name: "repodesk".to_string(),
            title: "Typed domain".to_string(),
            status: TaskStatus::Open,
            verify_command: Some("cargo test --workspace".to_string()),
            run_dir: dir.path().to_path_buf(),
            created_at: now,
            updated_at: now,
        };

        let work_item = WorkItem::try_from(&task).unwrap();
        assert_eq!(work_item.id.as_str(), "task-1");
        assert_eq!(work_item.project, "repodesk");
        assert_eq!(work_item.state, WorkItemState::Open);
        assert_eq!(work_item.run_dir, dir.path());
    }

    fn result(agent: &str, changed_files: &[&str], diff_path: Option<&str>) -> SubAgentResult {
        SubAgentResult {
            task_id: "implement".to_string(),
            agent: agent.to_string(),
            provider: if agent == "codex_cli" {
                "".to_string()
            } else {
                "ollama".to_string()
            },
            model: "model".to_string(),
            status: SubAgentStatus::Ok,
            output: "done".to_string(),
            input_tokens: 10,
            output_tokens: 5,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: changed_files
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
            change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
            execution_issues: Vec::new(),
            diff_path: diff_path.map(str::to_string),
            workspace: None,
            notes: Vec::new(),
        }
    }

    #[test]
    fn orchestration_run_adapts_to_execution_and_deduplicates_workers() {
        let run = OrchestrationRun {
            run_id: "run-1".to_string(),
            project: "repodesk".to_string(),
            task_id: "task-1".to_string(),
            goal: "Implement typed domain".to_string(),
            status: RunStatus::Completed,
            dry_run: false,
            started_at: "2026-08-07T10:00:00Z".to_string(),
            finished_at: "2026-08-07T10:01:00Z".to_string(),
            results: vec![
                result("codex_cli", &[], None),
                result("codex_cli", &[], None),
            ],
            total_input_tokens: 20,
            total_output_tokens: 10,
            total_cost_units: 0.1,
        };

        let execution = Execution::try_from(&run).unwrap();
        assert_eq!(execution.id.as_str(), "run-1");
        assert_eq!(execution.work_item_id.as_str(), "task-1");
        assert_eq!(execution.workers.len(), 1);
        assert_eq!(execution.workers[0].kind, WorkerKind::CodingAgent);
    }

    #[test]
    fn orchestration_run_projects_a_stable_changeset() {
        let run = OrchestrationRun {
            run_id: "run-1".to_string(),
            project: "repodesk".to_string(),
            task_id: "task-1".to_string(),
            goal: "Change files".to_string(),
            status: RunStatus::Completed,
            dry_run: false,
            started_at: "start".to_string(),
            finished_at: "finish".to_string(),
            results: vec![
                result(
                    "codex_cli",
                    &["src/b.rs", "src/a.rs"],
                    Some("diffs/1.patch"),
                ),
                result("codex_cli", &["src/a.rs"], Some("diffs/1.patch")),
            ],
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_units: 0.0,
        };

        let changeset = ChangeSet::try_from_run(&run).unwrap().unwrap();
        assert_eq!(changeset.id.as_str(), "run-1-changeset");
        assert_eq!(changeset.files, vec!["src/a.rs", "src/b.rs"]);
        assert_eq!(changeset.evidence.len(), 1);
        assert_eq!(changeset.status, ChangeSetStatus::Proposed);
    }
}
