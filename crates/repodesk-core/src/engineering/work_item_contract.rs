//! Typed engineering contract for an active RepoDesk Work Item.
//!
//! The contract is deliberately stored as a versioned task-local artifact rather
//! than embedded into the legacy task config. This keeps existing tasks readable
//! and lets the contract schema evolve independently while RepoDesk 2 migrates
//! from free-form task Markdown to enforceable engineering boundaries.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engineering::domain::WorkItemId;
use crate::engineering::events::{
    EngineeringEvent, EngineeringEventKind, append_event, read_events,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::tasks::{TaskInfo, show_active_task};

pub const WORK_ITEM_CONTRACT_FILE: &str = "work-item-contract.json";
pub const WORK_ITEM_CONTRACT_VERSION: u32 = 1;
const MAX_GOAL_CHARS: usize = 2_000;
const MAX_PATH_RULES: usize = 64;
const MAX_ACCEPTANCE_CRITERIA: usize = 64;
const MAX_RULE_CHARS: usize = 300;
const MAX_CRITERION_CHARS: usize = 600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemContract {
    pub version: u32,
    pub project: String,
    pub work_item_id: String,
    pub goal: String,
    pub allowed_paths: Vec<String>,
    pub protected_paths: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemContractUpdate {
    pub goal: String,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub protected_paths: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeComplianceStatus {
    NotEvaluated,
    Unconfigured,
    Compliant,
    Violation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemContractReadiness {
    pub goal_defined: bool,
    pub scope_defined: bool,
    pub acceptance_defined: bool,
    pub protected_paths_defined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeComplianceReport {
    pub status: ScopeComplianceStatus,
    pub changed_files: Vec<String>,
    pub allowed_changed_files: Vec<String>,
    pub out_of_scope_files: Vec<String>,
    pub protected_changed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemContractSnapshot {
    pub configured: bool,
    pub contract: WorkItemContract,
    pub readiness: WorkItemContractReadiness,
    pub compliance: ScopeComplianceReport,
}

pub fn contract_path(run_dir: &Path) -> PathBuf {
    run_dir.join(WORK_ITEM_CONTRACT_FILE)
}

pub fn load_active_work_item_contract() -> RepoDeskResult<WorkItemContractSnapshot> {
    let task = show_active_task()?;
    load_work_item_contract_snapshot(&task)
}

pub fn save_active_work_item_contract(
    update: WorkItemContractUpdate,
) -> RepoDeskResult<WorkItemContractSnapshot> {
    let task = show_active_task()?;
    let contract = normalize_update(&task, update)?;
    let path = contract_path(&task.config.run_dir);
    let content = serde_json::to_string_pretty(&contract)?;
    fs::write(&path, format!("{content}\n"))?;

    // Contract changes are engineering events, but the ledger stores only
    // counts/flags and a stable artifact locator — never the free-form goal.
    let work_item_id = WorkItemId::try_new(task.config.id.clone())
        .map_err(|error| RepoDeskError::Api(error.to_string()))?;
    let event = EngineeringEvent::new(
        task.config.project_name.clone(),
        work_item_id,
        EngineeringEventKind::ScopeChanged,
    )
    .with_attribute("contract_version", json!(contract.version))
    .with_attribute("goal_defined", Value::Bool(!contract.goal.is_empty()))
    .with_attribute("allowed_path_count", json!(contract.allowed_paths.len()))
    .with_attribute(
        "protected_path_count",
        json!(contract.protected_paths.len()),
    )
    .with_attribute(
        "acceptance_criteria_count",
        json!(contract.acceptance_criteria.len()),
    )
    .with_attribute("contract_file", Value::String(path.display().to_string()));
    let _ = append_event(&task.config.run_dir, &event);

    load_work_item_contract_snapshot(&task)
}

pub fn read_work_item_contract(run_dir: &Path) -> RepoDeskResult<Option<WorkItemContract>> {
    let path = contract_path(run_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let contract = serde_json::from_str(&content)?;
    Ok(Some(contract))
}

pub fn load_work_item_contract_snapshot(
    task: &TaskInfo,
) -> RepoDeskResult<WorkItemContractSnapshot> {
    let stored = read_work_item_contract(&task.config.run_dir)?;
    let configured = stored.is_some();
    let contract = stored.unwrap_or_else(|| empty_contract(task));
    let readiness = readiness(&contract);
    let changed_files = latest_changeset_files(&task.config.run_dir)?;
    let compliance = derive_scope_compliance(&contract, &changed_files, configured);

    Ok(WorkItemContractSnapshot {
        configured,
        contract,
        readiness,
        compliance,
    })
}

pub fn derive_scope_compliance(
    contract: &WorkItemContract,
    changed_files: &[String],
    configured: bool,
) -> ScopeComplianceReport {
    if changed_files.is_empty() {
        return ScopeComplianceReport {
            status: ScopeComplianceStatus::NotEvaluated,
            changed_files: Vec::new(),
            allowed_changed_files: Vec::new(),
            out_of_scope_files: Vec::new(),
            protected_changed_files: Vec::new(),
        };
    }

    if !configured || (contract.allowed_paths.is_empty() && contract.protected_paths.is_empty()) {
        return ScopeComplianceReport {
            status: ScopeComplianceStatus::Unconfigured,
            changed_files: changed_files.to_vec(),
            allowed_changed_files: Vec::new(),
            out_of_scope_files: Vec::new(),
            protected_changed_files: Vec::new(),
        };
    }

    let mut allowed_changed_files = Vec::new();
    let mut out_of_scope_files = Vec::new();
    let mut protected_changed_files = Vec::new();

    for path in changed_files {
        let normalized = normalize_changed_path(path);
        if contract
            .protected_paths
            .iter()
            .any(|rule| path_matches_rule(&normalized, rule))
        {
            protected_changed_files.push(path.clone());
            continue;
        }

        if contract.allowed_paths.is_empty()
            || contract
                .allowed_paths
                .iter()
                .any(|rule| path_matches_rule(&normalized, rule))
        {
            allowed_changed_files.push(path.clone());
        } else {
            out_of_scope_files.push(path.clone());
        }
    }

    let status = if protected_changed_files.is_empty() && out_of_scope_files.is_empty() {
        ScopeComplianceStatus::Compliant
    } else {
        ScopeComplianceStatus::Violation
    };

    ScopeComplianceReport {
        status,
        changed_files: changed_files.to_vec(),
        allowed_changed_files,
        out_of_scope_files,
        protected_changed_files,
    }
}

fn empty_contract(task: &TaskInfo) -> WorkItemContract {
    WorkItemContract {
        version: WORK_ITEM_CONTRACT_VERSION,
        project: task.config.project_name.clone(),
        work_item_id: task.config.id.clone(),
        goal: String::new(),
        allowed_paths: Vec::new(),
        protected_paths: Vec::new(),
        acceptance_criteria: Vec::new(),
        updated_at: task.config.updated_at,
    }
}

fn readiness(contract: &WorkItemContract) -> WorkItemContractReadiness {
    WorkItemContractReadiness {
        goal_defined: !contract.goal.trim().is_empty(),
        scope_defined: !contract.allowed_paths.is_empty(),
        acceptance_defined: !contract.acceptance_criteria.is_empty(),
        protected_paths_defined: !contract.protected_paths.is_empty(),
    }
}

fn normalize_update(
    task: &TaskInfo,
    update: WorkItemContractUpdate,
) -> RepoDeskResult<WorkItemContract> {
    let goal = update.goal.trim().to_string();
    if goal.chars().count() > MAX_GOAL_CHARS || goal.contains('\0') {
        return Err(RepoDeskError::Api(
            "Work Item goal is too long or invalid".into(),
        ));
    }

    let allowed_paths = normalize_rules(update.allowed_paths, false)?;
    let protected_paths = normalize_rules(update.protected_paths, true)?;
    let acceptance_criteria = normalize_criteria(update.acceptance_criteria)?;

    Ok(WorkItemContract {
        version: WORK_ITEM_CONTRACT_VERSION,
        project: task.config.project_name.clone(),
        work_item_id: task.config.id.clone(),
        goal,
        allowed_paths,
        protected_paths,
        acceptance_criteria,
        updated_at: Utc::now(),
    })
}

fn normalize_rules(rules: Vec<String>, allow_sensitive: bool) -> RepoDeskResult<Vec<String>> {
    if rules.len() > MAX_PATH_RULES {
        return Err(RepoDeskError::Api(format!(
            "Work Item contract supports at most {MAX_PATH_RULES} path rules"
        )));
    }

    let mut normalized = BTreeSet::new();
    for rule in rules {
        let Some(rule) = normalize_path_rule(&rule) else {
            return Err(RepoDeskError::Api(format!(
                "Invalid Work Item path rule: {rule}"
            )));
        };
        if !allow_sensitive && crate::security::is_blocked_path(&rule).is_some() {
            return Err(RepoDeskError::Api(format!(
                "Sensitive path cannot be added to allowed scope: {rule}"
            )));
        }
        normalized.insert(rule);
    }
    Ok(normalized.into_iter().collect())
}

fn normalize_criteria(criteria: Vec<String>) -> RepoDeskResult<Vec<String>> {
    if criteria.len() > MAX_ACCEPTANCE_CRITERIA {
        return Err(RepoDeskError::Api(format!(
            "Work Item contract supports at most {MAX_ACCEPTANCE_CRITERIA} acceptance criteria"
        )));
    }

    let mut normalized = Vec::new();
    for criterion in criteria {
        let criterion = criterion.trim();
        if criterion.is_empty() {
            continue;
        }
        if criterion.chars().count() > MAX_CRITERION_CHARS || criterion.contains('\0') {
            return Err(RepoDeskError::Api(
                "Acceptance criterion is too long or invalid".into(),
            ));
        }
        if !normalized.iter().any(|value| value == criterion) {
            normalized.push(criterion.to_string());
        }
    }
    Ok(normalized)
}

fn normalize_path_rule(value: &str) -> Option<String> {
    let value = value.trim().replace('\\', "/");
    if value.is_empty() || value.chars().count() > MAX_RULE_CHARS || value.contains('\0') {
        return None;
    }
    if value == "." {
        return Some(value);
    }

    let path = Path::new(&value);
    if path.is_absolute() {
        return None;
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }

    let normalized = value.trim_matches('/').to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_changed_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string()
}

fn path_matches_rule(path: &str, rule: &str) -> bool {
    if rule == "." {
        return true;
    }
    path == rule
        || path
            .strip_prefix(rule)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn latest_changeset_files(run_dir: &Path) -> RepoDeskResult<Vec<String>> {
    let events = read_events(run_dir)?;
    for event in events.iter().rev() {
        if event.kind != EngineeringEventKind::ChangeSetCreated {
            continue;
        }
        let Some(files) = event.attributes.get("files").and_then(Value::as_array) else {
            continue;
        };
        return Ok(files
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect());
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(allowed: &[&str], protected: &[&str]) -> WorkItemContract {
        WorkItemContract {
            version: 1,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            goal: "Keep changes bounded".into(),
            allowed_paths: allowed.iter().map(|value| (*value).into()).collect(),
            protected_paths: protected.iter().map(|value| (*value).into()).collect(),
            acceptance_criteria: vec!["tests pass".into()],
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn scope_compliance_flags_out_of_scope_and_protected_changes() {
        let report = derive_scope_compliance(
            &contract(&["src", "Cargo.toml"], &["src/security"]),
            &[
                "src/lib.rs".into(),
                "src/security/keys.rs".into(),
                "README.md".into(),
            ],
            true,
        );

        assert_eq!(report.status, ScopeComplianceStatus::Violation);
        assert_eq!(report.allowed_changed_files, vec!["src/lib.rs"]);
        assert_eq!(report.protected_changed_files, vec!["src/security/keys.rs"]);
        assert_eq!(report.out_of_scope_files, vec!["README.md"]);
    }

    #[test]
    fn protected_rules_take_precedence_over_allowed_parent() {
        let report = derive_scope_compliance(
            &contract(&["src"], &["src/generated"]),
            &["src/generated/code.rs".into()],
            true,
        );
        assert_eq!(report.status, ScopeComplianceStatus::Violation);
        assert!(report.allowed_changed_files.is_empty());
        assert_eq!(report.protected_changed_files.len(), 1);
    }

    #[test]
    fn unconfigured_contract_does_not_claim_compliance() {
        let report = derive_scope_compliance(&contract(&[], &[]), &["src/lib.rs".into()], false);
        assert_eq!(report.status, ScopeComplianceStatus::Unconfigured);
    }

    #[test]
    fn path_rules_are_relative_and_prefix_safe() {
        assert_eq!(normalize_path_rule("src/core"), Some("src/core".into()));
        assert_eq!(normalize_path_rule("src\\core"), Some("src/core".into()));
        assert_eq!(normalize_path_rule("../secret"), None);
        assert!(path_matches_rule("src/core/lib.rs", "src/core"));
        assert!(!path_matches_rule("src/core2/lib.rs", "src/core"));
    }
}
