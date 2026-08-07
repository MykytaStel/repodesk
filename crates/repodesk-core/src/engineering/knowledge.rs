//! Project-scoped Engineering Knowledge for RepoDesk 2.
//!
//! Knowledge is deliberately different from provider/model memory. Records are
//! local to one repository, reviewable by a human, provenance-aware, and only
//! become eligible for agent context after they are explicitly accepted.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::engineering::acceptance_evidence::active_verification_is_fresh;
use crate::engineering::domain::{EngineeringKnowledgeId, EvidenceKind, EvidenceRef, WorkItemId};
use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, append_event};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::estimate_text;
use crate::workflow::load_receipt;

pub const ENGINEERING_KNOWLEDGE_FILE: &str = "engineering-knowledge.json";
pub const ENGINEERING_KNOWLEDGE_VERSION: u32 = 1;
const MAX_RECORDS: usize = 512;
const MAX_TITLE_CHARS: usize = 120;
const MAX_CONTENT_CHARS: usize = 4_000;
const MAX_CONTEXT_RECORD_CHARS: usize = 1_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringKnowledgeCategory {
    Architecture,
    Convention,
    Hazard,
    Command,
    Testing,
    Decision,
    Performance,
    Tooling,
}

impl EngineeringKnowledgeCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Convention => "convention",
            Self::Hazard => "hazard",
            Self::Command => "command",
            Self::Testing => "testing",
            Self::Decision => "decision",
            Self::Performance => "performance",
            Self::Tooling => "tooling",
        }
    }

    fn context_priority(self) -> usize {
        match self {
            Self::Hazard => 6,
            Self::Architecture | Self::Convention => 5,
            Self::Command | Self::Testing | Self::Decision => 4,
            Self::Performance | Self::Tooling => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringKnowledgeStatus {
    Candidate,
    Accepted,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringKnowledgeOrigin {
    Human,
    Verification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeRecord {
    pub id: EngineeringKnowledgeId,
    pub project: String,
    pub category: EngineeringKnowledgeCategory,
    pub title: String,
    pub content: String,
    pub status: EngineeringKnowledgeStatus,
    pub origin: EngineeringKnowledgeOrigin,
    #[serde(default)]
    pub source_work_item_id: Option<WorkItemId>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeStore {
    pub version: u32,
    pub project: String,
    #[serde(default)]
    pub records: Vec<EngineeringKnowledgeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeCounts {
    pub candidates: usize,
    pub accepted: usize,
    pub archived: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeSuggestion {
    pub suggestion_id: String,
    pub category: EngineeringKnowledgeCategory,
    pub title: String,
    pub content: String,
    pub source_work_item_id: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeSnapshot {
    pub project: String,
    pub records: Vec<EngineeringKnowledgeRecord>,
    pub counts: EngineeringKnowledgeCounts,
    pub suggestions: Vec<EngineeringKnowledgeSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeProposalInput {
    pub category: EngineeringKnowledgeCategory,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineeringKnowledgeContext {
    pub candidate_markdown: String,
    pub markdown: String,
    pub included_ids: Vec<String>,
    pub candidate_tokens: usize,
    pub included_tokens: usize,
}

pub fn engineering_knowledge_path(project_name: &str) -> RepoDeskResult<PathBuf> {
    Ok(RepoDeskPaths::resolve()?
        .project_dir(project_name)
        .join(ENGINEERING_KNOWLEDGE_FILE))
}

pub fn load_active_engineering_knowledge() -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    let project = get_active_project()?;
    let store = read_store(&project.name)?;
    Ok(snapshot_from_store(store))
}

pub fn propose_active_engineering_knowledge(
    input: EngineeringKnowledgeProposalInput,
) -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    let project = get_active_project()?;
    let mut store = read_store(&project.name)?;
    ensure_capacity(&store)?;

    let title = normalize_text("knowledge title", &input.title, MAX_TITLE_CHARS)?;
    let content = normalize_text("knowledge content", &input.content, MAX_CONTENT_CHARS)?;
    let source_work_item_id = show_active_task()
        .ok()
        .filter(|task| task.config.project_name == project.name)
        .and_then(|task| WorkItemId::try_new(task.config.id).ok());
    let now = Utc::now();
    let record = EngineeringKnowledgeRecord {
        id: new_knowledge_id(&project.name, &title, &content)?,
        project: project.name.clone(),
        category: input.category,
        title,
        content,
        status: EngineeringKnowledgeStatus::Candidate,
        origin: EngineeringKnowledgeOrigin::Human,
        source_work_item_id,
        evidence: Vec::new(),
        created_at: now,
        updated_at: now,
    };
    store.records.push(record.clone());
    write_store(&store)?;
    record_knowledge_event(EngineeringEventKind::KnowledgeProposed, &record);
    Ok(snapshot_from_store(store))
}

/// Turn a successful command from the fresh canonical VerificationReceipt into
/// a reviewable knowledge candidate. The command is re-read from the receipt;
/// the caller cannot manufacture verification provenance.
pub fn capture_active_verified_command(command: &str) -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    if task.config.project_name != project.name {
        return Err(RepoDeskError::Api("Active Work Item does not belong to the active project".into()));
    }
    let receipt = load_receipt()?.ok_or_else(|| {
        RepoDeskError::Api("No canonical verification receipt exists for the active Work Item".into())
    })?;
    if !active_verification_is_fresh(&receipt)? {
        return Err(RepoDeskError::Api(
            "Verification is missing or stale; verify the current reviewed tree first".into(),
        ));
    }
    let verification = receipt.verification.as_ref().ok_or_else(|| {
        RepoDeskError::Api("No verification receipt exists for the active Work Item".into())
    })?;
    let requested = command.trim();
    let check = verification
        .commands
        .iter()
        .find(|check| check.command == requested)
        .ok_or_else(|| RepoDeskError::Api("Command is not part of the current VerificationReceipt".into()))?;
    if !check.success {
        return Err(RepoDeskError::Api(
            "Only successful verification commands can become reusable project knowledge".into(),
        ));
    }

    let mut store = read_store(&project.name)?;
    if store.records.iter().any(|record| {
        record.category == EngineeringKnowledgeCategory::Testing
            && record.content == requested
            && record.status != EngineeringKnowledgeStatus::Archived
    }) {
        return Ok(snapshot_from_store(store));
    }
    ensure_capacity(&store)?;

    let receipt_path = task.config.run_dir.join("task-run-receipt.json");
    let evidence = EvidenceRef::try_new(
        EvidenceKind::Verification,
        receipt_path.display().to_string(),
    )
    .map_err(|error| RepoDeskError::Api(error.to_string()))?;
    let title = verification_title(requested);
    let now = Utc::now();
    let record = EngineeringKnowledgeRecord {
        id: new_knowledge_id(&project.name, &title, requested)?,
        project: project.name.clone(),
        category: EngineeringKnowledgeCategory::Testing,
        title,
        content: requested.to_string(),
        status: EngineeringKnowledgeStatus::Candidate,
        origin: EngineeringKnowledgeOrigin::Verification,
        source_work_item_id: WorkItemId::try_new(task.config.id.clone()).ok(),
        evidence: vec![evidence],
        created_at: now,
        updated_at: now,
    };
    store.records.push(record.clone());
    write_store(&store)?;
    record_knowledge_event(EngineeringEventKind::KnowledgeProposed, &record);
    Ok(snapshot_from_store(store))
}

pub fn accept_active_engineering_knowledge(
    knowledge_id: &str,
) -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    set_active_knowledge_status(
        knowledge_id,
        EngineeringKnowledgeStatus::Accepted,
        EngineeringEventKind::KnowledgeAccepted,
    )
}

pub fn archive_active_engineering_knowledge(
    knowledge_id: &str,
) -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    set_active_knowledge_status(
        knowledge_id,
        EngineeringKnowledgeStatus::Archived,
        EngineeringEventKind::KnowledgeRejected,
    )
}

/// Build a deterministic, bounded slice of accepted Project Knowledge for an
/// agent context. Ranking is lexical + category-priority only; there is no AI
/// relevance score and no candidate/archived record can enter the context.
pub fn engineering_knowledge_context(
    project_name: &str,
    query: &str,
    budget_tokens: usize,
) -> RepoDeskResult<EngineeringKnowledgeContext> {
    let store = read_store(project_name)?;
    let query_terms = terms(query);
    let mut accepted = store
        .records
        .iter()
        .filter(|record| record.status == EngineeringKnowledgeStatus::Accepted)
        .collect::<Vec<_>>();

    accepted.sort_by(|left, right| {
        knowledge_score(right, &query_terms)
            .cmp(&knowledge_score(left, &query_terms))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });

    let candidate_markdown = render_records(&accepted);
    let candidate_tokens = estimate_text(&candidate_markdown).estimated_tokens;
    let mut blocks = Vec::new();
    let mut included_ids = Vec::new();

    for record in accepted {
        let block = render_record(record);
        let proposed = if blocks.is_empty() {
            block.clone()
        } else {
            format!("{}\n\n{}", blocks.join("\n\n"), block)
        };
        let tokens = estimate_text(&proposed).estimated_tokens;
        if tokens <= budget_tokens {
            blocks.push(block);
            included_ids.push(record.id.to_string());
            continue;
        }
        if blocks.is_empty() && budget_tokens > 0 {
            let chars = (budget_tokens.saturating_mul(4)).min(MAX_CONTEXT_RECORD_CHARS);
            let trimmed = block.chars().take(chars).collect::<String>();
            blocks.push(format!("{trimmed}\n[RepoDesk: knowledge trimmed for context budget]"));
            included_ids.push(record.id.to_string());
        }
        break;
    }

    let markdown = if blocks.is_empty() {
        "No accepted Engineering Knowledge matched the current project context.".to_string()
    } else {
        blocks.join("\n\n")
    };
    let included_tokens = estimate_text(&markdown).estimated_tokens;

    Ok(EngineeringKnowledgeContext {
        candidate_markdown,
        markdown,
        included_ids,
        candidate_tokens,
        included_tokens,
    })
}

fn snapshot_from_store(mut store: EngineeringKnowledgeStore) -> EngineeringKnowledgeSnapshot {
    store.records.sort_by(|left, right| {
        status_order(left.status)
            .cmp(&status_order(right.status))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    let counts = EngineeringKnowledgeCounts {
        candidates: store
            .records
            .iter()
            .filter(|record| record.status == EngineeringKnowledgeStatus::Candidate)
            .count(),
        accepted: store
            .records
            .iter()
            .filter(|record| record.status == EngineeringKnowledgeStatus::Accepted)
            .count(),
        archived: store
            .records
            .iter()
            .filter(|record| record.status == EngineeringKnowledgeStatus::Archived)
            .count(),
    };
    let suggestions = verified_command_suggestions(&store);
    EngineeringKnowledgeSnapshot {
        project: store.project,
        records: store.records,
        counts,
        suggestions,
    }
}

fn verified_command_suggestions(store: &EngineeringKnowledgeStore) -> Vec<EngineeringKnowledgeSuggestion> {
    let Ok(task) = show_active_task() else {
        return Vec::new();
    };
    if task.config.project_name != store.project {
        return Vec::new();
    }
    let Ok(Some(receipt)) = load_receipt() else {
        return Vec::new();
    };
    if !active_verification_is_fresh(&receipt).unwrap_or(false) {
        return Vec::new();
    }
    let Some(verification) = receipt.verification.as_ref() else {
        return Vec::new();
    };
    let receipt_path = task.config.run_dir.join("task-run-receipt.json");
    let evidence = EvidenceRef::try_new(
        EvidenceKind::Verification,
        receipt_path.display().to_string(),
    )
    .ok();

    verification
        .commands
        .iter()
        .filter(|check| check.success)
        .filter(|check| {
            !store.records.iter().any(|record| {
                record.category == EngineeringKnowledgeCategory::Testing
                    && record.content == check.command
            })
        })
        .map(|check| EngineeringKnowledgeSuggestion {
            suggestion_id: stable_suggestion_id(&task.config.id, &check.command),
            category: EngineeringKnowledgeCategory::Testing,
            title: verification_title(&check.command),
            content: check.command.clone(),
            source_work_item_id: task.config.id.clone(),
            evidence: evidence.clone().into_iter().collect(),
        })
        .collect()
}

fn set_active_knowledge_status(
    knowledge_id: &str,
    status: EngineeringKnowledgeStatus,
    event_kind: EngineeringEventKind,
) -> RepoDeskResult<EngineeringKnowledgeSnapshot> {
    let project = get_active_project()?;
    let mut store = read_store(&project.name)?;
    let record = store
        .records
        .iter_mut()
        .find(|record| record.id.as_str() == knowledge_id)
        .ok_or_else(|| RepoDeskError::Api("Engineering Knowledge record not found".into()))?;
    record.status = status;
    record.updated_at = Utc::now();
    let changed = record.clone();
    write_store(&store)?;
    record_knowledge_event(event_kind, &changed);
    Ok(snapshot_from_store(store))
}

fn read_store(project_name: &str) -> RepoDeskResult<EngineeringKnowledgeStore> {
    let path = engineering_knowledge_path(project_name)?;
    if !path.exists() {
        return Ok(EngineeringKnowledgeStore {
            version: ENGINEERING_KNOWLEDGE_VERSION,
            project: project_name.to_string(),
            records: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path)?;
    let store: EngineeringKnowledgeStore = serde_json::from_str(&content)?;
    if store.version != ENGINEERING_KNOWLEDGE_VERSION || store.project != project_name {
        return Err(RepoDeskError::Api(
            "Engineering Knowledge artifact version/project mismatch".into(),
        ));
    }
    Ok(store)
}

fn write_store(store: &EngineeringKnowledgeStore) -> RepoDeskResult<PathBuf> {
    let path = engineering_knowledge_path(&store.project)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    fs::write(&path, format!("{content}\n"))?;
    Ok(path)
}

fn record_knowledge_event(kind: EngineeringEventKind, record: &EngineeringKnowledgeRecord) {
    let Ok(task) = show_active_task() else {
        return;
    };
    if task.config.project_name != record.project {
        return;
    }
    let Ok(work_item_id) = WorkItemId::try_new(task.config.id.clone()) else {
        return;
    };
    let mut event = EngineeringEvent::new(record.project.clone(), work_item_id, kind)
        .with_attribute("knowledge_id", Value::String(record.id.to_string()))
        .with_attribute("category", Value::String(record.category.label().to_string()))
        .with_attribute("origin", json!(record.origin));
    if let Ok(path) = engineering_knowledge_path(&record.project)
        && let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Knowledge, path.display().to_string())
    {
        event = event.with_evidence(evidence);
    }
    let _ = append_event(&task.config.run_dir, &event);
}

fn ensure_capacity(store: &EngineeringKnowledgeStore) -> RepoDeskResult<()> {
    if store.records.len() >= MAX_RECORDS {
        Err(RepoDeskError::Api(format!(
            "Engineering Knowledge is limited to {MAX_RECORDS} records per project"
        )))
    } else {
        Ok(())
    }
}

fn normalize_text(label: &str, value: &str, max_chars: usize) -> RepoDeskResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars || value.contains('\0') {
        return Err(RepoDeskError::Api(format!("{label} is empty, too long, or invalid")));
    }
    Ok(value.to_string())
}

fn new_knowledge_id(project: &str, title: &str, content: &str) -> RepoDeskResult<EngineeringKnowledgeId> {
    let now = Utc::now().timestamp_micros();
    let digest = Sha256::digest(format!("{project}\n{title}\n{content}\n{now}").as_bytes());
    EngineeringKnowledgeId::try_new(format!("knowledge-{now}-{}", &hex::encode(digest)[..10]))
        .map_err(|error| RepoDeskError::Api(error.to_string()))
}

fn stable_suggestion_id(work_item_id: &str, command: &str) -> String {
    let digest = Sha256::digest(format!("{work_item_id}\n{command}").as_bytes());
    format!("suggestion-{}", &hex::encode(digest)[..16])
}

fn verification_title(command: &str) -> String {
    let compact = command.split_whitespace().collect::<Vec<_>>().join(" ");
    let prefix = "Verified command: ";
    let remaining = MAX_TITLE_CHARS.saturating_sub(prefix.len());
    let body = compact.chars().take(remaining).collect::<String>();
    format!("{prefix}{body}")
}

fn status_order(status: EngineeringKnowledgeStatus) -> usize {
    match status {
        EngineeringKnowledgeStatus::Candidate => 0,
        EngineeringKnowledgeStatus::Accepted => 1,
        EngineeringKnowledgeStatus::Archived => 2,
    }
}

fn terms(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|term| term.len() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn knowledge_score(record: &EngineeringKnowledgeRecord, query_terms: &BTreeSet<String>) -> usize {
    let title = record.title.to_ascii_lowercase();
    let content = record.content.to_ascii_lowercase();
    let lexical = query_terms
        .iter()
        .map(|term| {
            usize::from(title.contains(term)) * 4 + usize::from(content.contains(term))
        })
        .sum::<usize>();
    record.category.context_priority() + lexical
}

fn render_records(records: &[&EngineeringKnowledgeRecord]) -> String {
    if records.is_empty() {
        return "No accepted Engineering Knowledge is available for this project.".to_string();
    }
    records
        .iter()
        .map(|record| render_record(record))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_record(record: &EngineeringKnowledgeRecord) -> String {
    let content = if record.content.chars().count() > MAX_CONTEXT_RECORD_CHARS {
        format!(
            "{}…",
            record
                .content
                .chars()
                .take(MAX_CONTEXT_RECORD_CHARS)
                .collect::<String>()
        )
    } else {
        record.content.clone()
    };
    let source = record
        .source_work_item_id
        .as_ref()
        .map(|id| format!(" · source Work Item `{id}`"))
        .unwrap_or_default();
    format!(
        "### [{}] {}\n\n{}\n\n_Provenance: {} evidence ref(s){}_",
        record.category.label(),
        record.title,
        content,
        record.evidence.len(),
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        id: &str,
        category: EngineeringKnowledgeCategory,
        title: &str,
        content: &str,
        status: EngineeringKnowledgeStatus,
    ) -> EngineeringKnowledgeRecord {
        EngineeringKnowledgeRecord {
            id: EngineeringKnowledgeId::try_new(id).unwrap(),
            project: "repodesk".into(),
            category,
            title: title.into(),
            content: content.into(),
            status,
            origin: EngineeringKnowledgeOrigin::Human,
            source_work_item_id: None,
            evidence: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn candidates_are_never_rendered_into_agent_context() {
        let accepted = record(
            "knowledge-1",
            EngineeringKnowledgeCategory::Hazard,
            "Auth boundary",
            "Never bypass token validation.",
            EngineeringKnowledgeStatus::Accepted,
        );
        let candidate = record(
            "knowledge-2",
            EngineeringKnowledgeCategory::Convention,
            "Unreviewed",
            "This must not enter context.",
            EngineeringKnowledgeStatus::Candidate,
        );
        let query = terms("token validation auth");
        let mut records = vec![&accepted, &candidate];
        records.retain(|record| record.status == EngineeringKnowledgeStatus::Accepted);
        let rendered = render_records(&records);
        assert!(rendered.contains("Never bypass token validation"));
        assert!(!rendered.contains("must not enter context"));
        assert!(knowledge_score(&accepted, &query) > 0);
    }

    #[test]
    fn hazard_and_matching_terms_rank_above_unrelated_tooling() {
        let hazard = record(
            "knowledge-1",
            EngineeringKnowledgeCategory::Hazard,
            "Auth tokens",
            "Refresh tokens are single use.",
            EngineeringKnowledgeStatus::Accepted,
        );
        let tooling = record(
            "knowledge-2",
            EngineeringKnowledgeCategory::Tooling,
            "Formatter",
            "Run cargo fmt.",
            EngineeringKnowledgeStatus::Accepted,
        );
        let query = terms("auth refresh token rotation");
        assert!(knowledge_score(&hazard, &query) > knowledge_score(&tooling, &query));
    }

    #[test]
    fn generated_ids_are_valid_domain_ids() {
        let id = new_knowledge_id("repodesk", "Title", "Content").unwrap();
        assert!(id.as_str().starts_with("knowledge-"));
    }
}
