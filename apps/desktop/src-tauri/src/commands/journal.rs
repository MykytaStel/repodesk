//! Tauri commands for the event journal.
//!
//! The event journal is a persistent, append-only log of all significant
//! actions performed by RepoDesk (agent runs, sandbox blocks, secret detections,
//! provider switches, etc.). The UI calls these commands to display an audit
//! trail and to record its own user-initiated events.

use repodesk_core::persistence::event_journal::{
    EventJournalSnapshot, LogEventInput, log_event, try_journal_snapshot,
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

/// Return a paginated snapshot of the canonical event ledger.
///
/// The snapshot contains:
/// - `total_entries` — total verified entries in SQLite
/// - `returned` — number of entries in this response
/// - `counts_by_severity` — `{ "info": 10, "error": 2, … }`
/// - `entries` — the most-recent `limit` entries (newest first)
///
/// Integrity/database errors are returned to the frontend instead of being
/// flattened into an empty journal.
#[tauri::command]
pub fn get_event_journal(
    input: GetJournalInput,
) -> Result<EventJournalSnapshot, ErrorPayload> {
    let limit = input.limit.unwrap_or(50).min(500);
    try_journal_snapshot(limit).map_err(ErrorPayload::from)
}

/// Log an event that originated in the desktop UI (e.g. "user clicked Run",
/// "user switched project", "rate-limit banner shown").
///
/// Returns the updated snapshot so the UI can refresh in one round-trip.
#[tauri::command]
pub fn log_ui_event(input: LogUiEventInput) -> Result<EventJournalSnapshot, ErrorPayload> {
    // Basic validation
    let module_name = input.module_name.trim().to_string();
    if module_name.is_empty() || module_name.len() > 80 {
        return Err(ErrorPayload::from_message(
            "module_name must be 1–80 characters",
        ));
    }

    let message = input.message.trim().to_string();
    if message.is_empty() || message.len() > 1_000 {
        return Err(ErrorPayload::from_message(
            "message must be 1–1000 characters",
        ));
    }

    let level = match input.level.to_ascii_lowercase().as_str() {
        "info" | "warn" | "warning" | "error" | "security" | "ui" => {
            input.level.to_ascii_lowercase()
        }
        _ => {
            return Err(ErrorPayload::from_message(format!(
                "unsupported level '{}'",
                input.level
            )));
        }
    };

    let metadata = input.metadata.unwrap_or_default();

    log_event(LogEventInput {
        module_name,
        level,
        message,
        metadata,
    })?;

    try_journal_snapshot(50).map_err(ErrorPayload::from)
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
        Self::with_category("internal", message)
    }

    /// A user-facing setup/validation error (configuration category).
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::with_category("configuration", message)
    }

    /// A "too large / over budget" error (resource_limit category).
    pub fn resource_limit(message: impl Into<String>) -> Self {
        Self::with_category("resource_limit", message)
    }

    fn with_category(category: &str, message: impl Into<String>) -> Self {
        Self {
            category: category.to_string(),
            message: message.into(),
            retryable: false,
            detail: None,
        }
    }

    /// Build a payload from a bare message string, inferring the category from
    /// keywords (for errors that arrive as plain `String`s rather than a
    /// `RepoDeskError`). Mirrors the frontend's `guessCategory`.
    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let category = guess_category(&message);
        Self {
            retryable: category == "provider_transient",
            category: category.to_string(),
            message,
            detail: None,
        }
    }
}

impl From<repodesk_core::errors::RepoDeskError> for ErrorPayload {
    fn from(err: repodesk_core::errors::RepoDeskError) -> Self {
        ErrorPayload::from_core(&err)
    }
}

impl From<String> for ErrorPayload {
    fn from(message: String) -> Self {
        ErrorPayload::from_message(message)
    }
}

impl From<&str> for ErrorPayload {
    fn from(message: &str) -> Self {
        ErrorPayload::from_message(message)
    }
}

fn guess_category(message: &str) -> &'static str {
    let m = message.to_lowercase();
    if m.contains("rate")
        || m.contains("unreachable")
        || m.contains("unavailable")
        || m.contains("timeout")
        || m.contains("connect")
        || m.contains("429")
    {
        "provider_transient"
    } else if m.contains("secret")
        || m.contains("credential")
        || m.contains("blocked")
        || m.contains("sandbox")
        || m.contains("denied")
    {
        "security_block"
    } else if m.contains("too large")
        || m.contains("budget")
        || m.contains("exceeds")
        || m.contains("hard limit")
        || m.contains("token limit")
    {
        "resource_limit"
    } else if m.contains("not set")
        || m.contains("not found")
        || m.contains("invalid")
        || m.contains("must ")
        || m.contains("required")
        || m.contains("configure")
    {
        "configuration"
    } else {
        "internal"
    }
}
