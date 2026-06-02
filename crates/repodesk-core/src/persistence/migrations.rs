//! Lightweight, idempotent SQLite migrations for RepoDesk.
//!
//! The base tables (`events`, `memory`, `token_ledger`) are created with
//! `CREATE TABLE IF NOT EXISTS` in [`super::db::init_db`]. This module layers
//! *additive* schema changes on top of them and records the applied version in
//! a `schema_version` table so the work only runs once per database.
//!
//! Migrations here only ever *add* columns/tables/indexes (never drop or
//! rewrite), so an existing `repodesk.sqlite` keeps all of its rows.

use rusqlite::Connection;

use crate::errors::{RepoDeskError, RepoDeskResult};

/// The schema version the current binary expects.
pub const TARGET_VERSION: i64 = 1;

fn db_err(context: &str, e: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {e}"))
}

/// Apply any pending migrations. Safe to call on every connection.
pub fn run_migrations(conn: &Connection) -> RepoDeskResult<()> {
    ensure_version_table(conn)?;

    let current = current_version(conn)?;
    if current >= TARGET_VERSION {
        return Ok(());
    }

    if current < 1 {
        migrate_to_v1(conn)?;
        set_version(conn, 1)?;
    }

    Ok(())
}

fn ensure_version_table(conn: &Connection) -> RepoDeskResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create schema_version table", e))?;
    Ok(())
}

fn current_version(conn: &Connection) -> RepoDeskResult<i64> {
    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .map_err(|e| db_err("Failed to read schema_version", e))?;
    Ok(version)
}

fn set_version(conn: &Connection, version: i64) -> RepoDeskResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
        (version, chrono::Utc::now().to_rfc3339()),
    )
    .map_err(|e| db_err("Failed to record schema_version", e))?;
    Ok(())
}

/// Migration 1 — the Memory Brain extensions.
fn migrate_to_v1(conn: &Connection) -> RepoDeskResult<()> {
    // Provenance + lifecycle columns on the existing `memory` table.
    // Each is added only if missing so dev databases that partially applied
    // this migration still converge cleanly.
    let columns: &[(&str, &str)] = &[
        ("source", "TEXT NOT NULL DEFAULT 'human'"),
        ("agent", "TEXT NOT NULL DEFAULT ''"),
        ("task_id", "TEXT NOT NULL DEFAULT ''"),
        ("status", "TEXT NOT NULL DEFAULT 'active'"),
        ("pinned", "INTEGER NOT NULL DEFAULT 0"),
        ("salience", "REAL NOT NULL DEFAULT 0.5"),
        ("confidence", "REAL NOT NULL DEFAULT 1.0"),
        ("supersedes_id", "INTEGER"),
        ("content_hash", "TEXT NOT NULL DEFAULT ''"),
        ("updated_at", "TEXT NOT NULL DEFAULT ''"),
    ];

    for (name, decl) in columns {
        add_column_if_missing(conn, "memory", name, decl)?;
    }

    // The human-approved proposal queue: capture / dedup / merge / conflict /
    // supersede suggestions live here until accepted or rejected.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_proposals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            project TEXT NOT NULL,
            task_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            payload TEXT NOT NULL,
            applied_entry_id INTEGER
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create memory_proposals table", e))?;

    // Indexes that the brain retrieval and proposal queue rely on.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_project_status ON memory (project, status)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_memory_project_status", e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_content_hash ON memory (content_hash)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_memory_content_hash", e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_proposals_project_status ON memory_proposals (project, status)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_proposals_project_status", e))?;

    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> RepoDeskResult<bool> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| db_err("Failed to inspect table", e))?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| db_err("Failed to read table_info", e))?;
    for name in names.flatten() {
        if name.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> RepoDeskResult<()> {
    if column_exists(conn, table, column)? {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
        [],
    )
    .map_err(|e| db_err(&format!("Failed to add column {table}.{column}"), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_db() -> Connection {
        // Mimic a pre-migration database: the original three tables only.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                project TEXT NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                tags TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn migration_is_idempotent_and_preserves_rows() {
        let conn = legacy_db();
        // A legacy row with none of the new columns.
        conn.execute(
            "INSERT INTO memory (timestamp, project, content, category, tags)
             VALUES ('2026-01-01T00:00:00Z', 'demo', 'old note', 'general', '[]')",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();
        // Running twice must not error.
        run_migrations(&conn).unwrap();

        assert_eq!(current_version(&conn).unwrap(), TARGET_VERSION);

        // Legacy row survives and gets the new defaults.
        let (status, pinned, source): (String, i64, String) = conn
            .query_row(
                "SELECT status, pinned, source FROM memory WHERE content = 'old note'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "active");
        assert_eq!(pinned, 0);
        assert_eq!(source, "human");

        // Proposals table exists.
        let proposal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_proposals", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(proposal_count, 0);
    }
}
