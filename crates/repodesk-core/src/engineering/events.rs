//! Append-only engineering event ledger.
//!
//! Events are stored as JSON Lines inside the legacy task run directory. This
//! keeps the first ledger slice local, inspectable, migration-free, and easy to
//! replay into later Engineering Intelligence reports.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
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

pub const ENGINEERING_EVENT_LEDGER_FILE: &str = "engineering-events.jsonl";

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringEventKind {
    WorkItemCreated,
    ScopeChanged,
    ContextBuilt,
    ContextEdited,
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

pub fn event_ledger_path(run_dir: &Path) -> PathBuf {
    run_dir.join(ENGINEERING_EVENT_LEDGER_FILE)
}

/// Append one event as a single JSONL record. Existing records are never
/// rewritten by this API, preserving the ledger as replayable evidence.
pub fn append_event(run_dir: &Path, event: &EngineeringEvent) -> RepoDeskResult<PathBuf> {
    fs::create_dir_all(run_dir)?;
    let path = event_ledger_path(run_dir);
    let mut record = serde_json::to_vec(event)?;
    record.push(b'\n');

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(&record)?;
    file.flush()?;

    Ok(path)
}

/// Replay the task-local ledger in append order. A missing ledger is a valid
/// empty history, which makes adoption backward-compatible for existing tasks.
pub fn read_events(run_dir: &Path) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let path = event_ledger_path(run_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    read_event_file(&path)
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
    use tempfile::tempdir;

    #[test]
    fn missing_ledger_is_an_empty_history() {
        let dir = tempdir().unwrap();
        assert!(read_events(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn ledger_appends_and_replays_events_in_order() {
        let dir = tempdir().unwrap();
        let work_item_id = WorkItemId::try_new("task-1").unwrap();

        let first = EngineeringEvent::new(
            "repodesk",
            work_item_id.clone(),
            EngineeringEventKind::WorkItemCreated,
        )
        .with_attribute("title", Value::String("Typed domain".to_string()));

        let second = EngineeringEvent::new(
            "repodesk",
            work_item_id,
            EngineeringEventKind::ContextBuilt,
        )
        .with_evidence(EvidenceRef::try_new(EvidenceKind::Context, "context.md").unwrap())
        .with_attribute("tokens", Value::from(1200));

        let path = append_event(dir.path(), &first).unwrap();
        append_event(dir.path(), &second).unwrap();

        assert_eq!(path, dir.path().join(ENGINEERING_EVENT_LEDGER_FILE));

        let events = read_events(dir.path()).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EngineeringEventKind::WorkItemCreated);
        assert_eq!(events[1].kind, EngineeringEventKind::ContextBuilt);
        assert_eq!(events[1].attributes["tokens"], Value::from(1200));
        assert_ne!(events[0].id, events[1].id);
    }

    #[test]
    fn ledger_is_plain_json_lines_for_inspection_and_replay() {
        let dir = tempdir().unwrap();
        let event = EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            EngineeringEventKind::HumanOverride,
        );

        let path = append_event(dir.path(), &event).unwrap();
        let raw = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = raw.lines().collect();

        assert_eq!(lines.len(), 1);
        let decoded: EngineeringEvent = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(decoded.id, event.id);
    }
}
