//! Desktop bridge for the hash-chained audit trail. Surfaces the *real* events
//! and chain-verification result; there is no synthetic data on this path.

use repodesk_core::audit::{self, AuditEvent, ChainVerification};

use super::ErrorPayload;

/// Hard cap so the webview can never request an unbounded slice of the trail.
const MAX_AUDIT_EVENTS: usize = 500;

/// Most recent audit events, newest first (capped). Empty when nothing logged.
#[tauri::command]
pub fn audit_recent(limit: Option<usize>) -> Result<Vec<AuditEvent>, ErrorPayload> {
    let limit = limit.unwrap_or(50).min(MAX_AUDIT_EVENTS);
    audit::recent_events(limit).map_err(ErrorPayload::from)
}

/// Verify the SHA-256 hash chain end to end.
#[tauri::command]
pub fn audit_verify() -> Result<ChainVerification, ErrorPayload> {
    audit::verify_audit_chain().map_err(ErrorPayload::from)
}
