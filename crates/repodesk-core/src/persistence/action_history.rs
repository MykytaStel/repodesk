//! SQLite-backed desktop action history (migration v2). Each desktop action run
//! is stored as a JSON payload plus a few indexed columns, replacing the old
//! `action-history.jsonl` file so history persists with the rest of local state.

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::db::init_db;
use crate::workflow::ActionRunResult;

fn db_err(context: &str, e: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {e}"))
}

/// Append one action run to the history.
pub fn record_action_run(run: &ActionRunResult) -> RepoDeskResult<()> {
    let conn = init_db()?;
    let payload = serde_json::to_string(run)?;
    conn.execute(
        "INSERT INTO action_runs (action_id, ok, finished_at_ms, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            run.id,
            run.result.ok as i64,
            run.finished_at_ms as i64,
            payload
        ],
    )
    .map_err(|e| db_err("Failed to record action run", e))?;
    Ok(())
}

/// Most recent action runs, newest first.
pub fn recent_action_runs(limit: usize) -> RepoDeskResult<Vec<ActionRunResult>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare("SELECT payload FROM action_runs ORDER BY id DESC LIMIT ?1")
        .map_err(|e| db_err("Failed to prepare action history query", e))?;
    let rows = stmt
        .query_map([limit as i64], |row| row.get::<_, String>(0))
        .map_err(|e| db_err("Failed to read action history", e))?;

    let mut runs = Vec::new();
    for payload in rows.flatten() {
        if let Ok(run) = serde_json::from_str::<ActionRunResult>(&payload) {
            runs.push(run);
        }
    }
    Ok(runs)
}

/// Total number of recorded action runs (for the DB status panel).
pub fn count_action_runs() -> RepoDeskResult<i64> {
    let conn = init_db()?;
    conn.query_row("SELECT COUNT(*) FROM action_runs", [], |row| row.get(0))
        .map_err(|e| db_err("Failed to count action runs", e))
}
