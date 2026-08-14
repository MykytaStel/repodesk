//! Typed engineering-event compatibility facade over the canonical SQLite ledger.
//!
//! New engineering events are appended only to the hash-chained SQLite journal.
//! The historical task-local JSONL file is read-only compatibility for Work Items
//! created before this migration; it is never mutated by the current write path.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engineering::domain::{
    ChangeSetId, EngineeringEventId, EvidenceRef, ExecutionId, VerificationId, WorkItemId,
    WorkerRef,
};
use crate::errors::RepoDeskResult;
use crate::persistence::db::get_db_path;
use crate::persistence::event_journal::{
    EngineeringEventInput, append_engineering_event, read_engineering_events,
};

pub const ENGINEERING_EVENT_LEDGER_FILE: &str = "engineering-events.jsonl";
const TYPED_EVENT_PAYLOAD_KEY: &str = "typed_engineering_event";

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringEventKind {
    WorkItemCreated,
    ScopeChanged,
    ContextBuilt,
    ContextEdited,
    AiStrategySelected,
    ExecutionStarted,
    ExecutionFinished,
    WorkerHandoff,
    ChangeSetCreated,
    ChangeSetReviewed,
    VerificationStarted,
    VerificationFinished,
    CommitCreated,
    KnowledgeProposed,
    KnowledgeAccepted,
    KnowledgeRejected,
    KnowledgeArchived,
    HumanOverride,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringEvent {
    pub id: EngineeringEventId,
    pub occurred_at: DateTime<Utc>,
    pub project: String,
    pub work_item_id: WorkItemId,
    pub kind: EngineeringEventKind,
    #[serde(default)]
    pub execution_id: Option<ExecutionId>,
    #[serde(default)]
    pub changeset_id: Option<ChangeSetId>,
    #[serde(default)]
    pub verification_id: Option<VerificationId>,
    #[serde(default)]
    pub worker: Option<WorkerRef>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

impl EngineeringEvent {
    pub fn new(
        project: impl Into<String>,
        work_item_id: WorkItemId,
        kind: EngineeringEventKind,
    ) -> Self {
        Self {
            id: next_event_id(),
            occurred_at: Utc::now(),
            project: project.into(),
            work_item_id,
            kind,
            execution_id: None,
            changeset_id: None,
            verification_id: None,
            worker: None,
            evidence: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_execution(mut self, execution_id: ExecutionId) -> Self {
        self.execution_id = Some(execution_id);
        self
    }

    pub fn with_changeset(mut self, changeset_id: ChangeSetId) -> Self {
        self.changeset_id = Some(changeset_id);
        self
    }

    pub fn with_verification(mut self, verification_id: VerificationId) -> Self {
        self.verification_id = Some(verification_id);
        self
    }

    pub fn with_worker(mut self, worker: WorkerRef) -> Self {
        self.worker = Some(worker);
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }
}

fn next_event_id() -> EngineeringEventId {
    let timestamp = Utc::now().timestamp_micros();
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    EngineeringEventId::try_new(format!("evt-{timestamp}-{pid}-{sequence}"))
        .expect("generated engineering event id must be valid")
}

fn event_kind_label(kind: EngineeringEventKind) -> &'static str {
    match kind {
        EngineeringEventKind::WorkItemCreated => "work_item_created",
        EngineeringEventKind::ScopeChanged => "scope_changed",
        EngineeringEventKind::ContextBuilt => "context_built",
        EngineeringEventKind::ContextEdited => "context_edited",
        EngineeringEventKind::AiStrategySelected => "ai_strategy_selected",
        EngineeringEventKind::ExecutionStarted => "execution_started",
        EngineeringEventKind::ExecutionFinished => "execution_finished",
        EngineeringEventKind::WorkerHandoff => "worker_handoff",
        EngineeringEventKind::ChangeSetCreated => "changeset_created",
        EngineeringEventKind::ChangeSetReviewed => "changeset_reviewed",
        EngineeringEventKind::VerificationStarted => "verification_started",
        EngineeringEventKind::VerificationFinished => "verification_finished",
        EngineeringEventKind::CommitCreated => "commit_created",
        EngineeringEventKind::KnowledgeProposed => "knowledge_proposed",
        EngineeringEventKind::KnowledgeAccepted => "knowledge_accepted",
        EngineeringEventKind::KnowledgeRejected => "knowledge_rejected",
        EngineeringEventKind::KnowledgeArchived => "knowledge_archived",
        EngineeringEventKind::HumanOverride => "human_override",
    }
}

pub fn event_ledger_path(run_dir: &Path) -> PathBuf {
    run_dir.join(ENGINEERING_EVENT_LEDGER_FILE)
}

/// Append one typed engineering event to the canonical hash-chained SQLite
/// journal. `run_dir` is retained in the compatibility signature because many
/// existing instrumentation call sites already carry it; it is no longer a
/// storage destination.
pub fn append_event(_run_dir: &Path, event: &EngineeringEvent) -> RepoDeskResult<PathBuf> {
    let kind = event_kind_label(event.kind);
    let mut payload = BTreeMap::new();
    payload.insert(
        TYPED_EVENT_PAYLOAD_KEY.to_string(),
        serde_json::to_string(event)?,
    );
    payload.insert("event_id".to_string(), event.id.to_string());
    payload.insert("occurred_at".to_string(), event.occurred_at.to_rfc3339());
    if let Some(changeset_id) = event.changeset_id.as_ref() {
        payload.insert("changeset_id".to_string(), changeset_id.to_string());
    }
    if let Some(verification_id) = event.verification_id.as_ref() {
        payload.insert("verification_id".to_string(), verification_id.to_string());
    }

    append_engineering_event(EngineeringEventInput {
        project: event.project.clone(),
        work_item_id: event.work_item_id.to_string(),
        run_id: event
            .execution_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        kind: kind.to_string(),
        module_name: "engineering".to_string(),
        level: "info".to_string(),
        message: format!("engineering event: {kind}"),
        payload,
    })?;

    get_db_path()
}

/// Replay typed engineering events for the Work Item represented by `run_dir`.
/// Canonical SQLite events are authoritative. A historical task-local JSONL is
/// merged read-only so pre-migration evidence remains visible; no current code
/// appends to that file.
pub fn read_events(run_dir: &Path) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let task_id = run_dir
        .file_name()
        .map(|value| value.to_string_lossy().to_string());
    let project = run_dir
        .parent()
        .and_then(Path::file_name)
        .map(|value| value.to_string_lossy().to_string());

    let mut canonical = read_engineering_events(usize::MAX)?
        .into_iter()
        .filter(|entry| {
            task_id
                .as_deref()
                .is_none_or(|task_id| entry.work_item_id == task_id)
                && project
                    .as_deref()
                    .is_none_or(|project| entry.project == project)
        })
        .filter_map(|entry| entry.payload.get(TYPED_EVENT_PAYLOAD_KEY).cloned())
        .map(|encoded| serde_json::from_str::<EngineeringEvent>(&encoded))
        .collect::<Result<Vec<_>, _>>()?;

    // Canonical journal reads are newest-first; typed replay remains oldest-first.
    canonical.reverse();

    let legacy_path = event_ledger_path(run_dir);
    let legacy = if legacy_path.exists() {
        read_event_file(&legacy_path)?
    } else {
        Vec::new()
    };

    if legacy.is_empty() {
        return Ok(canonical);
    }

    let mut seen = BTreeSet::new();
    let mut combined = Vec::with_capacity(legacy.len() + canonical.len());
    for event in legacy.into_iter().chain(canonical) {
        if seen.insert(event.id.clone()) {
            combined.push(event);
        }
    }
    combined.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(combined)
}

fn read_event_file(path: &Path) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        events.push(serde_json::from_str(&line)?);
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::domain::{EvidenceKind, EvidenceRef};
    use serial_test::serial;
    use tempfile::TempDir;

    fn isolated_run() -> (TempDir, PathBuf) {
        let home = TempDir::new().expect("temp home");
        // SAFETY: tests in this module are serialized because REPODESK_HOME is
        // process-global.
        unsafe {
            std::env::set_var("REPODESK_HOME", home.path());
        }
        crate::init::init_home().expect("init home");
        let run_dir = home.path().join("runs/repodesk/task-1");
        fs::create_dir_all(&run_dir).expect("run dir");
        (home, run_dir)
    }

    #[test]
    #[serial]
    fn missing_legacy_ledger_and_empty_sqlite_is_an_empty_history() {
        let (_home, run_dir) = isolated_run();
        assert!(read_events(&run_dir).unwrap().is_empty());
    }

    #[test]
    #[serial]
    fn canonical_ledger_appends_and_replays_typed_events_in_order() {
        let (_home, run_dir) = isolated_run();
        let work_item_id = WorkItemId::try_new("task-1").unwrap();

        let first = EngineeringEvent::new(
            "repodesk",
            work_item_id.clone(),
            EngineeringEventKind::WorkItemCreated,
        )
        .with_attribute("title", Value::String("Typed domain".to_string()));

        let second =
            EngineeringEvent::new("repodesk", work_item_id, EngineeringEventKind::ContextBuilt)
                .with_evidence(EvidenceRef::try_new(EvidenceKind::Context, "context.md").unwrap())
                .with_attribute("tokens", Value::from(1200));

        let path = append_event(&run_dir, &first).unwrap();
        append_event(&run_dir, &second).unwrap();

        assert_eq!(path, get_db_path().unwrap());
        assert!(path.exists());
        assert!(
            !event_ledger_path(&run_dir).exists(),
            "task-local JSONL must not be written by the canonical append path"
        );

        let events = read_events(&run_dir).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EngineeringEventKind::WorkItemCreated);
        assert_eq!(events[1].kind, EngineeringEventKind::ContextBuilt);
        assert_eq!(events[1].attributes["tokens"], Value::from(1200));
        assert_ne!(events[0].id, events[1].id);
    }

    #[test]
    #[serial]
    fn historical_task_jsonl_is_read_only_compatibility() {
        let (_home, run_dir) = isolated_run();
        let work_item_id = WorkItemId::try_new("task-1").unwrap();
        let legacy = EngineeringEvent::new(
            "repodesk",
            work_item_id.clone(),
            EngineeringEventKind::WorkItemCreated,
        );
        let legacy_path = event_ledger_path(&run_dir);
        fs::write(
            &legacy_path,
            format!("{}\n", serde_json::to_string(&legacy).unwrap()),
        )
        .unwrap();

        let current =
            EngineeringEvent::new("repodesk", work_item_id, EngineeringEventKind::ContextBuilt)
                .with_attribute("tokens", Value::from(800));
        append_event(&run_dir, &current).unwrap();

        let events = read_events(&run_dir).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event.id == legacy.id));
        assert!(events.iter().any(|event| event.id == current.id));

        let raw = fs::read_to_string(legacy_path).unwrap();
        assert_eq!(raw.lines().count(), 1, "legacy JSONL must remain read-only");
    }
}
