use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

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

// ── Core types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LogEventInput {
    pub module_name: String,
    /// One of: info, warn, error, security, ui
    pub level: String,
    pub message: String,
    pub metadata: Vec<(String, String)>,
}

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

// ── Snapshot type (sent to the frontend) ─────────────────────────────────────

/// A paginated, pre-processed view of the event journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventJournalSnapshot {
    pub generated_at: DateTime<Utc>,
    /// Total entries in the journal file (before pagination).
    pub total_entries: usize,
    /// How many entries are returned in this snapshot.
    pub returned: usize,
    /// Breakdown: how many entries per severity level.
    pub counts_by_severity: BTreeMap<String, usize>,
    pub entries: Vec<EventEntry>,
}

// ── Write ─────────────────────────────────────────────────────────────────────

pub fn log_event(input: LogEventInput) -> RepoDeskResult<PathBuf> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let journal_file = paths.logs_dir.join("event-journal.jsonl");

    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let task_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());

    let metadata = input.metadata.into_iter().collect::<BTreeMap<_, _>>();

    let entry = EventEntry {
        timestamp: Utc::now(),
        project,
        task_id,
        module_name: input.module_name,
        level: input.level,
        message: input.message,
        metadata,
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&journal_file)?;

    writeln!(file, "{}", serde_json::to_string(&entry)?)?;

    Ok(journal_file)
}

// ── Read ──────────────────────────────────────────────────────────────────────

pub fn read_events(limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let journal_file = paths.logs_dir.join("event-journal.jsonl");

    if !journal_file.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(journal_file)?;
    let mut entries = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Skip lines that fail to parse rather than crashing the whole read.
        if let Ok(entry) = serde_json::from_str::<EventEntry>(line) {
            entries.push(entry);
        }
    }

    let keep = limit.min(entries.len());
    Ok(entries.into_iter().rev().take(keep).collect())
}

/// Build a `EventJournalSnapshot` — the primary type exposed to the Tauri UI.
pub fn journal_snapshot(limit: usize) -> EventJournalSnapshot {
    let all_entries = read_events(usize::MAX).unwrap_or_default();
    let total_entries = all_entries.len();

    // Severity breakdown (across ALL entries, not just the page).
    let mut counts_by_severity: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &all_entries {
        *counts_by_severity
            .entry(entry.severity().as_str().to_string())
            .or_insert(0) += 1;
    }

    // Take the `limit` most-recent entries for the response.
    let entries: Vec<EventEntry> = all_entries.into_iter().take(limit).collect();
    let returned = entries.len();

    EventJournalSnapshot {
        generated_at: Utc::now(),
        total_entries,
        returned,
        counts_by_severity,
        entries,
    }
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

