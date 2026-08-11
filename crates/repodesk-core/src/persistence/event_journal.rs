use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::persistence::db::{get_db_path, init_db};
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

const EVENT_SCHEMA_VERSION: i64 = 1;
const LEGACY_IMPORT_KEY: &str = "legacy_event_journal_jsonl_v1";
const MAX_LEGACY_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;

fn db_err(context: &str, e: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {e}"))
}

// ── Severity ──────────────────────────────────────────────────────────────────

/// Severity level for journal events. The UI uses this to colour-code entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Info,
    Warn,
    Error,
    /// A security-related event (sandbox block, secret detected, etc.)
    Security,
    /// Emitted by a user action in the desktop UI (not a daemon / CLI action).
    Ui,
}

impl EventSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Security => "security",
            Self::Ui => "ui",
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            "security" => Self::Security,
            "ui" => Self::Ui,
            _ => Self::Info,
        }
    }
}

// ── Public types ──────────────────────────────────────────────────────────────

/// Compatibility input used by the existing CLI/Tauri/orchestrator callers.
/// First-class ledger fields are derived from the active Work Item and metadata,
/// while callers can migrate gradually to [`EngineeringEventInput`].
#[derive(Debug, Clone)]
pub struct LogEventInput {
    pub module_name: String,
    /// One of: info, warn, error, security, ui
    pub level: String,
    pub message: String,
    pub metadata: Vec<(String, String)>,
}

/// Stable compatibility projection consumed by the existing UI and CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub task_id: String,
    pub module_name: String,
    pub level: String,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

impl EventEntry {
    pub fn severity(&self) -> EventSeverity {
        EventSeverity::from_str_lossy(&self.level)
    }
}

/// Canonical engineering event stored in SQLite. This is the evidence-bearing
/// representation used by future Trust Graph work; `EventEntry` is only its
/// backwards-compatible UI projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineeringEvent {
    pub sequence: i64,
    pub schema_version: i64,
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub work_item_id: String,
    pub run_id: String,
    pub kind: String,
    pub module_name: String,
    pub level: String,
    pub message: String,
    pub payload: BTreeMap<String, String>,
    pub prev_hash: String,
    pub event_hash: String,
}

impl From<EngineeringEvent> for EventEntry {
    fn from(event: EngineeringEvent) -> Self {
        Self {
            timestamp: event.timestamp,
            project: event.project,
            task_id: event.work_item_id,
            module_name: event.module_name,
            level: event.level,
            message: event.message,
            metadata: event.payload,
        }
    }
}

/// Explicit input for callers that already know their evidence identity.
#[derive(Debug, Clone)]
pub struct EngineeringEventInput {
    pub project: String,
    pub work_item_id: String,
    pub run_id: String,
    pub kind: String,
    pub module_name: String,
    pub level: String,
    pub message: String,
    pub payload: BTreeMap<String, String>,
}

/// A paginated, pre-processed view of the event journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventJournalSnapshot {
    pub generated_at: DateTime<Utc>,
    /// Total entries in the canonical SQLite ledger (before pagination).
    pub total_entries: usize,
    /// How many entries are returned in this snapshot.
    pub returned: usize,
    /// Breakdown: how many entries per severity level.
    pub counts_by_severity: BTreeMap<String, usize>,
    pub entries: Vec<EventEntry>,
}

#[derive(Debug, Clone)]
struct PendingEvent {
    timestamp: DateTime<Utc>,
    project: String,
    work_item_id: String,
    run_id: String,
    kind: String,
    module_name: String,
    level: String,
    message: String,
    payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct StoredEvent {
    sequence: i64,
    schema_version: i64,
    timestamp: String,
    project: String,
    work_item_id: String,
    run_id: String,
    kind: String,
    module_name: String,
    level: String,
    message: String,
    payload_json: String,
    prev_hash: String,
    event_hash: String,
}

#[derive(Serialize)]
struct EventHashMaterial<'a> {
    sequence: i64,
    schema_version: i64,
    timestamp: &'a str,
    project: &'a str,
    work_item_id: &'a str,
    run_id: &'a str,
    kind: &'a str,
    module_name: &'a str,
    level: &'a str,
    message: &'a str,
    payload: &'a str,
    prev_hash: &'a str,
}

impl StoredEvent {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            sequence: row.get(0)?,
            schema_version: row.get(1)?,
            timestamp: row.get(2)?,
            project: row.get(3)?,
            work_item_id: row.get(4)?,
            run_id: row.get(5)?,
            kind: row.get(6)?,
            module_name: row.get(7)?,
            level: row.get(8)?,
            message: row.get(9)?,
            payload_json: row.get(10)?,
            prev_hash: row.get(11)?,
            event_hash: row.get(12)?,
        })
    }

    fn expected_hash(&self) -> RepoDeskResult<String> {
        compute_event_hash(EventHashMaterial {
            sequence: self.sequence,
            schema_version: self.schema_version,
            timestamp: &self.timestamp,
            project: &self.project,
            work_item_id: &self.work_item_id,
            run_id: &self.run_id,
            kind: &self.kind,
            module_name: &self.module_name,
            level: &self.level,
            message: &self.message,
            payload: &self.payload_json,
            prev_hash: &self.prev_hash,
        })
    }

    fn verify_hash(&self) -> RepoDeskResult<()> {
        let expected = self.expected_hash()?;
        if expected != self.event_hash {
            return Err(RepoDeskError::Database(format!(
                "engineering event ledger hash mismatch at sequence {}",
                self.sequence
            )));
        }
        Ok(())
    }

    fn into_event(self) -> RepoDeskResult<EngineeringEvent> {
        let timestamp = DateTime::parse_from_rfc3339(&self.timestamp)
            .map_err(|e| db_err("Failed to parse engineering event timestamp", e))?
            .with_timezone(&Utc);
        let payload = serde_json::from_str::<BTreeMap<String, String>>(&self.payload_json)
            .map_err(|e| db_err("Failed to parse engineering event payload", e))?;

        Ok(EngineeringEvent {
            sequence: self.sequence,
            schema_version: self.schema_version,
            timestamp,
            project: self.project,
            work_item_id: self.work_item_id,
            run_id: self.run_id,
            kind: self.kind,
            module_name: self.module_name,
            level: self.level,
            message: self.message,
            payload,
            prev_hash: self.prev_hash,
            event_hash: self.event_hash,
        })
    }
}

fn compute_event_hash(material: EventHashMaterial<'_>) -> RepoDeskResult<String> {
    let bytes = serde_json::to_vec(&material)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn event_kind(module_name: &str, payload: &BTreeMap<String, String>) -> String {
    payload
        .get("event_kind")
        .or_else(|| payload.get("kind"))
        .map(|kind| kind.trim())
        .filter(|kind| !kind.is_empty())
        .unwrap_or(module_name)
        .to_string()
}

fn event_run_id(payload: &BTreeMap<String, String>) -> String {
    payload
        .get("run_id")
        .or_else(|| payload.get("run"))
        .map(|run| run.trim().to_string())
        .unwrap_or_default()
}

fn stored_tail(conn: &Connection) -> RepoDeskResult<Option<StoredEvent>> {
    conn.query_row(
        "SELECT sequence, schema_version, timestamp, project, work_item_id, run_id,
                kind, module_name, level, message, payload, prev_hash, event_hash
         FROM engineering_events ORDER BY sequence DESC LIMIT 1",
        [],
        StoredEvent::from_row,
    )
    .optional()
    .map_err(|e| db_err("Failed to read engineering event ledger tail", e))
}

fn verify_tail(conn: &Connection, tail: &StoredEvent) -> RepoDeskResult<()> {
    tail.verify_hash()?;

    if tail.sequence == 1 {
        if !tail.prev_hash.is_empty() {
            return Err(RepoDeskError::Database(
                "engineering event ledger genesis event has a non-empty prev_hash".to_string(),
            ));
        }
        return Ok(());
    }

    let previous_hash: Option<String> = conn
        .query_row(
            "SELECT event_hash FROM engineering_events WHERE sequence = ?1",
            [tail.sequence - 1],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| db_err("Failed to verify engineering event ledger tail", e))?;

    if previous_hash.as_deref() != Some(tail.prev_hash.as_str()) {
        return Err(RepoDeskError::Database(format!(
            "engineering event ledger chain is broken before sequence {}",
            tail.sequence
        )));
    }
    Ok(())
}

fn append_pending(tx: &Transaction<'_>, pending: PendingEvent) -> RepoDeskResult<EngineeringEvent> {
    let tail = stored_tail(tx)?;
    if let Some(tail) = tail.as_ref() {
        verify_tail(tx, tail)?;
    }

    let sequence = match tail.as_ref() {
        Some(tail) => tail
            .sequence
            .checked_add(1)
            .ok_or_else(|| RepoDeskError::Database("event sequence overflow".to_string()))?,
        None => 1,
    };
    let prev_hash = tail
        .as_ref()
        .map(|event| event.event_hash.clone())
        .unwrap_or_default();
    let timestamp = pending
        .timestamp
        .to_rfc3339_opts(SecondsFormat::Nanos, true);
    let payload_json = serde_json::to_string(&pending.payload)?;

    let event_hash = compute_event_hash(EventHashMaterial {
        sequence,
        schema_version: EVENT_SCHEMA_VERSION,
        timestamp: &timestamp,
        project: &pending.project,
        work_item_id: &pending.work_item_id,
        run_id: &pending.run_id,
        kind: &pending.kind,
        module_name: &pending.module_name,
        level: &pending.level,
        message: &pending.message,
        payload: &payload_json,
        prev_hash: &prev_hash,
    })?;

    tx.execute(
        "INSERT INTO engineering_events (
            sequence, schema_version, timestamp, project, work_item_id, run_id,
            kind, module_name, level, message, payload, prev_hash, event_hash
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            sequence,
            EVENT_SCHEMA_VERSION,
            timestamp,
            pending.project,
            pending.work_item_id,
            pending.run_id,
            pending.kind,
            pending.module_name,
            pending.level,
            pending.message,
            payload_json,
            prev_hash,
            event_hash,
        ],
    )
    .map_err(|e| db_err("Failed to append engineering event", e))?;

    stored_tail(tx)?
        .ok_or_else(|| RepoDeskError::Database("appended event disappeared".to_string()))?
        .into_event()
}

// ── Legacy JSONL migration ───────────────────────────────────────────────────

fn legacy_journal_path() -> RepoDeskResult<PathBuf> {
    Ok(RepoDeskPaths::resolve()?.logs_dir.join("event-journal.jsonl"))
}

fn import_marker(conn: &Connection) -> RepoDeskResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM event_ledger_meta WHERE key = ?1",
        [LEGACY_IMPORT_KEY],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| db_err("Failed to read legacy event import marker", e))
}

fn mark_import_complete(
    tx: &Transaction<'_>,
    imported: usize,
    skipped: usize,
) -> RepoDeskResult<()> {
    let value = serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "completed_at": Utc::now().to_rfc3339(),
    })
    .to_string();
    tx.execute(
        "INSERT OR REPLACE INTO event_ledger_meta (key, value) VALUES (?1, ?2)",
        params![LEGACY_IMPORT_KEY, value],
    )
    .map_err(|e| db_err("Failed to record legacy event import marker", e))?;
    Ok(())
}

fn read_legacy_entries(path: &Path) -> RepoDeskResult<(Vec<EventEntry>, usize)> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(RepoDeskError::Database(format!(
            "refusing to import symlinked legacy event journal: {}",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(RepoDeskError::Database(format!(
            "legacy event journal is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_LEGACY_JOURNAL_BYTES {
        return Err(RepoDeskError::Database(format!(
            "legacy event journal exceeds {} bytes",
            MAX_LEGACY_JOURNAL_BYTES
        )));
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<EventEntry>(&line) {
            Ok(entry) => entries.push(entry),
            Err(_) => skipped += 1,
        }
    }
    Ok((entries, skipped))
}

fn ensure_legacy_jsonl_import() -> RepoDeskResult<()> {
    init::init_home()?;
    let mut conn = init_db()?;
    if import_marker(&conn)?.is_some() {
        return Ok(());
    }

    let legacy_path = legacy_journal_path()?;
    let (entries, skipped) = if legacy_path.exists() {
        read_legacy_entries(&legacy_path)?
    } else {
        (Vec::new(), 0)
    };

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| db_err("Failed to start legacy event import transaction", e))?;

    // Another process may have completed the import while we were reading the
    // bounded legacy file. Re-check under the writer lock before inserting.
    if import_marker(&tx)?.is_some() {
        tx.commit()
            .map_err(|e| db_err("Failed to finish legacy event import transaction", e))?;
        return Ok(());
    }

    let imported = entries.len();
    for entry in entries {
        let kind = event_kind(&entry.module_name, &entry.metadata);
        let run_id = event_run_id(&entry.metadata);
        append_pending(
            &tx,
            PendingEvent {
                timestamp: entry.timestamp,
                project: entry.project,
                work_item_id: entry.task_id,
                run_id,
                kind,
                module_name: entry.module_name,
                level: entry.level,
                message: entry.message,
                payload: entry.metadata,
            },
        )?;
    }
    mark_import_complete(&tx, imported, skipped)?;
    tx.commit()
        .map_err(|e| db_err("Failed to commit legacy event import", e))?;
    Ok(())
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Append a first-class engineering event transactionally and return the exact
/// stored record. This never writes JSONL; SQLite is the canonical source.
pub fn append_engineering_event(input: EngineeringEventInput) -> RepoDeskResult<EngineeringEvent> {
    ensure_legacy_jsonl_import()?;

    let mut conn = init_db()?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| db_err("Failed to start engineering event transaction", e))?;
    let event = append_pending(
        &tx,
        PendingEvent {
            timestamp: Utc::now(),
            project: input.project,
            work_item_id: input.work_item_id,
            run_id: input.run_id,
            kind: input.kind,
            module_name: input.module_name,
            level: input.level,
            message: input.message,
            payload: input.payload,
        },
    )?;
    tx.commit()
        .map_err(|e| db_err("Failed to commit engineering event", e))?;
    Ok(event)
}

/// Compatibility append used throughout the current product. The active task is
/// the current Work Item identity. `run_id` and `kind` are promoted from metadata
/// when present; otherwise kind falls back to the module name.
pub fn log_event(input: LogEventInput) -> RepoDeskResult<PathBuf> {
    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let work_item_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());
    let payload = input.metadata.into_iter().collect::<BTreeMap<_, _>>();
    let run_id = event_run_id(&payload);
    let kind = event_kind(&input.module_name, &payload);

    append_engineering_event(EngineeringEventInput {
        project,
        work_item_id,
        run_id,
        kind,
        module_name: input.module_name,
        level: input.level,
        message: input.message,
        payload,
    })?;

    get_db_path()
}

// ── Read / integrity verification ────────────────────────────────────────────

fn load_verified_ledger() -> RepoDeskResult<Vec<EngineeringEvent>> {
    ensure_legacy_jsonl_import()?;
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT sequence, schema_version, timestamp, project, work_item_id, run_id,
                    kind, module_name, level, message, payload, prev_hash, event_hash
             FROM engineering_events ORDER BY sequence ASC",
        )
        .map_err(|e| db_err("Failed to prepare engineering event query", e))?;
    let rows = stmt
        .query_map([], StoredEvent::from_row)
        .map_err(|e| db_err("Failed to read engineering events", e))?;

    let mut events = Vec::new();
    let mut expected_sequence = 1i64;
    let mut expected_prev_hash = String::new();

    for row in rows {
        let stored = row.map_err(|e| db_err("Failed to decode engineering event row", e))?;
        stored.verify_hash()?;
        if stored.sequence != expected_sequence {
            return Err(RepoDeskError::Database(format!(
                "engineering event ledger sequence gap: expected {}, found {}",
                expected_sequence, stored.sequence
            )));
        }
        if stored.prev_hash != expected_prev_hash {
            return Err(RepoDeskError::Database(format!(
                "engineering event ledger chain mismatch at sequence {}",
                stored.sequence
            )));
        }

        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| RepoDeskError::Database("event sequence overflow".to_string()))?;
        expected_prev_hash = stored.event_hash.clone();
        events.push(stored.into_event()?);
    }

    Ok(events)
}

/// Canonical engineering events, newest first. Reads fail closed if any stored
/// sequence/hash link is corrupt.
pub fn read_engineering_events(limit: usize) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let events = load_verified_ledger()?;
    let keep = limit.min(events.len());
    Ok(events.into_iter().rev().take(keep).collect())
}

pub fn read_events(limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    Ok(read_engineering_events(limit)?
        .into_iter()
        .map(EventEntry::from)
        .collect())
}

/// The most recent events for a single task, newest-first. Backs the per-task
/// activity timeline; filters by Work Item identity before applying `limit`.
pub fn read_task_events(task_id: &str, limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    let all = read_engineering_events(usize::MAX)?;
    Ok(all
        .into_iter()
        .filter(|entry| entry.work_item_id == task_id)
        .take(limit)
        .map(EventEntry::from)
        .collect())
}

/// Build a journal snapshot while preserving integrity errors for callers that
/// can surface them (notably the Tauri boundary).
pub fn try_journal_snapshot(limit: usize) -> RepoDeskResult<EventJournalSnapshot> {
    let all_entries = read_events(usize::MAX)?;
    let total_entries = all_entries.len();

    let mut counts_by_severity: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &all_entries {
        *counts_by_severity
            .entry(entry.severity().as_str().to_string())
            .or_insert(0) += 1;
    }

    let entries: Vec<EventEntry> = all_entries.into_iter().take(limit).collect();
    let returned = entries.len();

    Ok(EventJournalSnapshot {
        generated_at: Utc::now(),
        total_entries,
        returned,
        counts_by_severity,
        entries,
    })
}

/// Backwards-compatible snapshot helper for non-error-aware callers. New
/// boundaries should prefer [`try_journal_snapshot`] so corruption is visible.
pub fn journal_snapshot(limit: usize) -> EventJournalSnapshot {
    try_journal_snapshot(limit).unwrap_or_else(|_| EventJournalSnapshot {
        generated_at: Utc::now(),
        total_entries: 0,
        returned: 0,
        counts_by_severity: BTreeMap::new(),
        entries: Vec::new(),
    })
}

// ── JSONL export ──────────────────────────────────────────────────────────────

/// Materialize the verified SQLite ledger as the historical JSONL shape.
/// JSONL is an export artifact only; modifying it never changes canonical state.
pub fn export_event_journal_jsonl() -> RepoDeskResult<PathBuf> {
    let entries = read_events(usize::MAX)?;
    let target = legacy_journal_path()?;
    let temp = target.with_file_name(format!(
        ".event-journal.jsonl.tmp-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));

    let result = (|| -> RepoDeskResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        // Historical JSONL was append-ordered (oldest first).
        for entry in entries.iter().rev() {
            writeln!(file, "{}", serde_json::to_string(entry)?)?;
        }
        file.flush()?;
        file.sync_all()?;

        if target.exists() {
            let metadata = fs::symlink_metadata(&target)?;
            if metadata.file_type().is_symlink() || metadata.is_file() {
                fs::remove_file(&target)?;
            } else {
                return Err(RepoDeskError::Database(format!(
                    "event journal export target is not a file: {}",
                    target.display()
                )));
            }
        }
        fs::rename(&temp, &target)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    Ok(target)
}

// ── Formatting (CLI output) ───────────────────────────────────────────────────

pub fn format_events(events: &[EventEntry]) -> String {
    if events.is_empty() {
        return "No events recorded yet.\n".to_string();
    }

    let mut output = String::new();
    output.push_str("Event journal:\n\n");

    for event in events {
        output.push_str(&format!(
            "- [{}] {} :: {}\n",
            event.level, event.module_name, event.message
        ));
        output.push_str(&format!("  time: {}\n", event.timestamp));
        output.push_str(&format!("  project: {}\n", event.project));
        output.push_str(&format!("  task: {}\n", event.task_id));

        if !event.metadata.is_empty() {
            output.push_str("  metadata:\n");
            for (key, value) in &event.metadata {
                output.push_str(&format!("    {}: {}\n", key, value));
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn isolated_home() -> TempDir {
        let home = TempDir::new().expect("temp home");
        // SAFETY: every test in this module is serial because REPODESK_HOME is
        // process-global.
        unsafe {
            std::env::set_var("REPODESK_HOME", home.path());
        }
        init::init_home().expect("init home");
        home
    }

    fn input(message: &str) -> LogEventInput {
        LogEventInput {
            module_name: "orchestrator".to_string(),
            level: "info".to_string(),
            message: message.to_string(),
            metadata: vec![
                ("run_id".to_string(), "run-1".to_string()),
                ("event_kind".to_string(), "execution".to_string()),
            ],
        }
    }

    #[test]
    #[serial]
    fn sqlite_is_canonical_and_hash_chain_is_explicit() {
        let home = isolated_home();
        let db_path = log_event(input("first")).expect("log first");
        log_event(input("second")).expect("log second");

        assert_eq!(db_path, get_db_path().expect("db path"));
        assert!(db_path.exists());
        assert!(
            !home.path().join("logs/event-journal.jsonl").exists(),
            "JSONL must not be written as the canonical append path"
        );

        let events = read_engineering_events(10).expect("read ledger");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 2);
        assert_eq!(events[1].sequence, 1);
        assert_eq!(events[0].prev_hash, events[1].event_hash);
        assert_eq!(events[0].schema_version, EVENT_SCHEMA_VERSION);
        assert_eq!(events[0].run_id, "run-1");
        assert_eq!(events[0].kind, "execution");
        assert_eq!(events[0].event_hash.len(), 64);
    }

    #[test]
    #[serial]
    fn legacy_jsonl_import_is_transactional_idempotent_and_one_time() {
        let home = isolated_home();
        let legacy = home.path().join("logs/event-journal.jsonl");
        let old = serde_json::json!({
            "timestamp": "2026-01-01T00:00:00Z",
            "project": "demo",
            "task_id": "work-1",
            "module_name": "orchestrator",
            "level": "info",
            "message": "old",
            "metadata": {"run_id": "run-old"}
        });
        let newer = serde_json::json!({
            "timestamp": "2026-01-01T00:01:00Z",
            "project": "demo",
            "task_id": "work-1",
            "module_name": "review",
            "level": "warn",
            "message": "newer",
            "metadata": {}
        });
        fs::write(&legacy, format!("{old}\nnot-json\n{newer}\n")).expect("legacy journal");

        let first = read_events(10).expect("first import");
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].message, "newer");
        assert_eq!(first[1].message, "old");

        let second = read_events(10).expect("second read");
        assert_eq!(second.len(), 2, "legacy import must be idempotent");

        // Once migrated, later edits to the JSONL export/legacy file are not a
        // source of truth and must not appear in SQLite.
        let fake = serde_json::json!({
            "timestamp": "2026-01-01T00:02:00Z",
            "project": "demo",
            "task_id": "work-1",
            "module_name": "fake",
            "level": "error",
            "message": "must-not-import",
            "metadata": {}
        });
        let mut file = OpenOptions::new().append(true).open(&legacy).expect("append");
        writeln!(file, "{fake}").expect("append fake");
        assert_eq!(read_events(10).expect("read canonical").len(), 2);
    }

    #[test]
    #[serial]
    fn tampered_tail_blocks_further_append() {
        let _home = isolated_home();
        log_event(input("original")).expect("first event");

        let conn = init_db().expect("db");
        conn.execute(
            "UPDATE engineering_events SET message = 'tampered' WHERE sequence = 1",
            [],
        )
        .expect("tamper");

        let error = log_event(input("must fail")).expect_err("tampered ledger must fail closed");
        assert!(error.to_string().contains("hash mismatch"));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM engineering_events", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1, "failed append must not create a second event");
    }

    #[test]
    #[serial]
    fn jsonl_export_is_explicit_and_cannot_mutate_canonical_history() {
        let _home = isolated_home();
        log_event(input("canonical")).expect("event");

        let exported = export_event_journal_jsonl().expect("export");
        let content = fs::read_to_string(&exported).expect("read export");
        assert!(content.contains("canonical"));

        fs::write(
            &exported,
            "{\"timestamp\":\"2026-01-01T00:00:00Z\",\"project\":\"fake\",\"task_id\":\"fake\",\"module_name\":\"fake\",\"level\":\"error\",\"message\":\"fake\",\"metadata\":{}}\n",
        )
        .expect("rewrite export");

        let events = read_events(10).expect("read canonical");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "canonical");
    }
}
