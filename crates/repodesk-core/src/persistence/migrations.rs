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
pub const TARGET_VERSION: i64 = 5;

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

    if current < 2 {
        migrate_to_v2(conn)?;
        set_version(conn, 2)?;
    }

    if current < 3 {
        migrate_to_v3(conn)?;
        set_version(conn, 3)?;
    }

    if current < 4 {
        migrate_to_v4(conn)?;
        set_version(conn, 4)?;
    }

    if current < 5 {
        migrate_to_v5(conn)?;
        set_version(conn, 5)?;
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

/// Migration 2 — desktop action history moves from a JSONL file into SQLite so
/// it persists with the rest of the local state and can be backed up together.
fn migrate_to_v2(conn: &Connection) -> RepoDeskResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS action_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_id TEXT NOT NULL,
            ok INTEGER NOT NULL,
            finished_at_ms INTEGER NOT NULL,
            payload TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create action_runs table", e))?;
    Ok(())
}

/// Migration 3 — the orchestrator outcome ledger (the N8 "Hermes" learning
/// signal). One row per executed sub-agent step records what was routed where,
/// what it cost, and how it turned out. The auto-verdict is provisional until a
/// human confirms it (`verdict_source` flips to `human`); this is the same
/// propose→approve discipline the Memory Brain uses. The adaptive router reads
/// the aggregate of these rows; nothing here mutates routing on its own.
fn migrate_to_v3(conn: &Connection) -> RepoDeskResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS run_outcomes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            project TEXT NOT NULL,
            task_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            step_id TEXT NOT NULL,
            task_kind TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cost_units REAL NOT NULL DEFAULT 0.0,
            verdict TEXT NOT NULL DEFAULT 'neutral',
            verdict_source TEXT NOT NULL DEFAULT 'auto',
            confirmed INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT ''
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create run_outcomes table", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outcomes_project_kind_provider
            ON run_outcomes (project, task_kind, provider)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_outcomes_project_kind_provider", e))?;

    Ok(())
}

/// Migration 4 — RAG document embeddings. This table stores the chunked text and
/// its vector embedding blob for local semantic search against project files.
fn migrate_to_v4(conn: &Connection) -> RepoDeskResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS project_embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project TEXT NOT NULL,
            file_path TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            embedding_blob BLOB NOT NULL
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create project_embeddings table", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_embeddings_project ON project_embeddings (project)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_embeddings_project", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_embeddings_project_path ON project_embeddings (project, file_path)",
        [],
    )
    .map_err(|e| db_err("Failed to create idx_embeddings_project_path", e))?;

    Ok(())
}

/// Migration 5 — canonical engineering-event ledger.
///
/// The historical `events` table was part of the original base schema but the
/// event journal never wrote to it; production event history lived in a JSONL
/// file instead. The new ledger is intentionally separate so its stronger
/// invariants are explicit: monotonically increasing sequence numbers, event
/// schema versioning, first-class work-item/run/kind fields, canonical payloads,
/// and a SHA-256 hash chain. JSONL is now only an import/export representation.
fn migrate_to_v5(conn: &Connection) -> RepoDeskResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS engineering_events (
            sequence INTEGER PRIMARY KEY,
            schema_version INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            project TEXT NOT NULL,
            work_item_id TEXT NOT NULL,
            run_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL,
            module_name TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            payload TEXT NOT NULL,
            prev_hash TEXT NOT NULL,
            event_hash TEXT NOT NULL UNIQUE
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create engineering_events table", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS event_ledger_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| db_err("Failed to create event_ledger_meta table", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_engineering_events_project_sequence
            ON engineering_events (project, sequence)",
        [],
    )
    .map_err(|e| db_err("Failed to create project event index", e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_engineering_events_work_item_sequence
            ON engineering_events (work_item_id, sequence)",
        [],
    )
    .map_err(|e| db_err("Failed to create work-item event index", e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_engineering_events_run_sequence
            ON engineering_events (run_id, sequence)",
        [],
    )
    .map_err(|e| db_err("Failed to create run event index", e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_engineering_events_kind_sequence
            ON engineering_events (kind, sequence)",
        [],
    )
    .map_err(|e| db_err("Failed to create kind event index", e))?;

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

        // v2: action_runs table exists.
        let action_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM action_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(action_count, 0);

        // v3: run_outcomes table exists.
        let outcome_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM run_outcomes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(outcome_count, 0);

        // v4: project_embeddings table exists.
        let emb_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM project_embeddings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(emb_count, 0);

        // v5: canonical engineering-event ledger + migration metadata exist.
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM engineering_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(event_count, 0);
        let meta_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM event_ledger_meta", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(meta_count, 0);
    }
}
