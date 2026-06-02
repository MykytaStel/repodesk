use rusqlite::Connection;
use std::path::PathBuf;

use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;

pub fn get_db_path() -> RepoDeskResult<PathBuf> {
    let paths = RepoDeskPaths::resolve()?;
    Ok(paths.config_dir.join("repodesk.sqlite"))
}

pub fn init_db() -> RepoDeskResult<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path)
        .map_err(|e| crate::errors::RepoDeskError::Database(format!("Failed to open DB: {}", e)))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            project TEXT NOT NULL,
            task_id TEXT NOT NULL,
            module_name TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            metadata TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| {
        crate::errors::RepoDeskError::Database(format!("Failed to create events table: {}", e))
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            project TEXT NOT NULL,
            content TEXT NOT NULL,
            category TEXT NOT NULL,
            tags TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| {
        crate::errors::RepoDeskError::Database(format!("Failed to create memory table: {}", e))
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS token_ledger (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            project TEXT NOT NULL,
            task_id TEXT NOT NULL,
            agent TEXT NOT NULL,
            model TEXT NOT NULL,
            category TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            total_tokens INTEGER NOT NULL,
            notes TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| {
        crate::errors::RepoDeskError::Database(format!(
            "Failed to create token_ledger table: {}",
            e
        ))
    })?;

    // Apply additive Memory Brain schema (provenance columns, proposals table,
    // indexes). Idempotent and version-gated.
    crate::persistence::migrations::run_migrations(&conn)?;

    Ok(conn)
}

// ── Memory Brain re-exports ──────────────────────────────────────────────────
// The memory model and storage moved to `crate::memory`. These shims keep the
// historical `persistence::db::*` import paths working for existing callers
// (CLI + Tauri commands).
pub use crate::memory::consolidate::consolidate_project_memory;
pub use crate::memory::model::MemoryEntry;
pub use crate::memory::store::{add_memory, list_memory};
