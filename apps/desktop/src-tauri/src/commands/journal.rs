//! Tauri commands for the event journal.
//!
//! The event journal is a persistent, append-only log of all significant
//! actions performed by RepoDesk (agent runs, sandbox blocks, secret detections,
//! provider switches, etc.).  The UI calls these commands to display an audit
//! trail and to record its own user-initiated events.

use repodesk_core::persistence::event_journal::{
    journal_snapshot, log_event, EventJournalSnapshot, LogEventInput,
};
use serde::{Deserialize, Serialize};

// ── Input types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetJournalInput {
    /// Maximum number of entries to return (most-recent first). Defaults to 50.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct LogUiEventInput {
    pub module_name: String,
    /// One of: info, warn, error, security, ui
    pub level: String,
    pub message: String,
    /// Optional key/value pairs attached to the entry.
    pub metadata: Option<Vec<(String, String)>>,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Return a paginated snapshot of the event journal.
///
/// The snapshot contains:
/// - `total_entries`  — total lines in the journal file
/// - `returned`       — number of entries in this response
/// - `counts_by_severity` — `{ "info": 10, "error": 2, … }`
/// - `entries`        — the most-recent `limit` entries (newest first)
#[tauri::command]
pub fn get_event_journal(input: GetJournalInput) -> EventJournalSnapshot {
    let limit = input.limit.unwrap_or(50).min(500);
    journal_snapshot(limit)
}

/// Log an event that originated in the desktop UI (e.g. "user clicked Run",
/// "user switched project", "rate-limit banner shown").
///
/// Returns the updated snapshot so the UI can refresh in one round-trip.
#[tauri::command]
pub fn log_ui_event(input: LogUiEventInput) -> Result<EventJournalSnapshot, String> {
    // Basic validation
    let module_name = input.module_name.trim().to_string();
    if module_name.is_empty() || module_name.len() > 80 {
        return Err("module_name must be 1–80 characters".into());
    }

    let message = input.message.trim().to_string();
    if message.is_empty() || message.len() > 1_000 {
        return Err("message must be 1–1000 characters".into());
    }

    let level = match input.level.to_ascii_lowercase().as_str() {
        "info" | "warn" | "warning" | "error" | "security" | "ui" => {
            input.level.to_ascii_lowercase()
        }
        _ => return Err(format!("unsupported level '{}'", input.level)),
    };

    let metadata = input.metadata.unwrap_or_default();

    log_event(LogEventInput {
        module_name,
        level,
        message,
        metadata,
    })
    .map_err(|error| error.to_string())?;

    Ok(journal_snapshot(50))
}

/// Structured error payload sent from Tauri commands to the frontend.
///
/// Every `Err(String)` returned by a Tauri command is automatically converted
/// to a JSON object by calling `error_payload()` before returning, so the
/// frontend always receives a consistent structure it can pattern-match on.
#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    /// Machine-readable category: configuration | provider_transient |
    ///   security_block | resource_limit | internal
    pub category: String,
    /// Human-readable message shown in the notification toast.
    pub message: String,
    /// Whether the frontend should offer a "Retry" button.
    pub retryable: bool,
    /// Optional structured data the frontend can use for deep-linking.
    pub detail: Option<serde_json::Value>,
}

impl ErrorPayload {
    pub fn from_core(err: &repodesk_core::errors::RepoDeskError) -> Self {
        use repodesk_core::errors::ErrorCategory;
        let category = match err.category() {
            ErrorCategory::Configuration => "configuration",
            ErrorCategory::ProviderTransient => "provider_transient",
            ErrorCategory::SecurityBlock => "security_block",
            ErrorCategory::ResourceLimit => "resource_limit",
            ErrorCategory::Internal => "internal",
        };

        Self {
            category: category.to_string(),
            message: err.to_string(),
            retryable: err.is_retryable(),
            detail: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            category: "internal".into(),
            message: message.into(),
            retryable: false,
            detail: None,
        }
    }
}
