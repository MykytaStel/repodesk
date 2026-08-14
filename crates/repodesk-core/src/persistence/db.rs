use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;

pub fn get_db_path() -> RepoDeskResult<PathBuf> {
    let paths = RepoDeskPaths::resolve()?;
    Ok(paths.config_dir.join("repodesk.sqlite"))
}

fn db_error(context: &str, error: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {error}"))
}

fn unique_sibling(path: &Path, label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repodesk.sqlite");
    path.with_file_name(format!(".{file_name}.{label}-{pid}-{stamp}"))
}

fn verify_integrity(conn: &Connection) -> RepoDeskResult<()> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| db_error("Failed to run SQLite integrity_check", error))?;
    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(RepoDeskError::Database(format!(
            "SQLite integrity_check failed: {result}"
        )))
    }
}

fn verify_repodesk_schema(conn: &Connection) -> RepoDeskResult<()> {
    let expected = ["events", "memory", "token_ledger"];
    for table in expected {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .map_err(|error| db_error("Failed to inspect backup schema", error))?;
        if exists != 1 {
            return Err(RepoDeskError::Database(format!(
                "Selected SQLite file is not a RepoDesk state backup: missing table '{table}'"
            )));
        }
    }
    Ok(())
}

fn validate_backup_source(src: &Path) -> RepoDeskResult<()> {
    if !src.is_file() {
        return Err(RepoDeskError::Database(format!(
            "Backup file does not exist: {}",
            src.display()
        )));
    }
    let conn = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| db_error("Failed to open backup database", error))?;
    verify_integrity(&conn)?;
    verify_repodesk_schema(&conn)
}

/// Create a consistent SQLite snapshot at `dest`.
///
/// `VACUUM INTO` asks SQLite itself to produce the snapshot instead of copying a
/// potentially live database file byte-for-byte. The resulting backup is then
/// reopened read-only and validated before success is reported.
pub fn backup_to(dest: &Path) -> RepoDeskResult<()> {
    let conn = init_db()?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        return Err(RepoDeskError::Database(format!(
            "Backup destination already exists: {}",
            dest.display()
        )));
    }

    let dest_text = dest.to_string_lossy().to_string();
    conn.execute("VACUUM INTO ?1", [dest_text.as_str()])
        .map_err(|error| db_error("Failed to create SQLite backup snapshot", error))?;
    drop(conn);

    if let Err(error) = validate_backup_source(dest) {
        let _ = std::fs::remove_file(dest);
        return Err(error);
    }
    Ok(())
}

/// Restore a RepoDesk SQLite backup without overwriting the live database until
/// the candidate has been fully validated and migrated.
///
/// Protocol:
/// 1. Validate the source as an intact RepoDesk database.
/// 2. Copy it to a sibling temporary file.
/// 3. Apply the current schema/migrations to the temporary file and re-check it.
/// 4. Move the live DB aside, promote the validated candidate, and roll back the
///    old DB if promotion fails.
pub fn restore_from(src: &Path) -> RepoDeskResult<()> {
    let dest = get_db_path()?;
    restore_database_file(src, &dest)
}

fn restore_database_file(src: &Path, dest: &Path) -> RepoDeskResult<()> {
    validate_backup_source(src)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let candidate = unique_sibling(dest, "restore-candidate");
    let rollback = unique_sibling(dest, "restore-rollback");

    std::fs::copy(src, &candidate)
        .map_err(|error| db_error("Failed to stage restore candidate", error))?;

    let candidate_result = (|| -> RepoDeskResult<()> {
        let conn = Connection::open(&candidate)
            .map_err(|error| db_error("Failed to open restore candidate", error))?;
        initialize_connection(&conn)?;
        verify_integrity(&conn)?;
        verify_repodesk_schema(&conn)?;
        Ok(())
    })();

    if let Err(error) = candidate_result {
        let _ = std::fs::remove_file(&candidate);
        return Err(error);
    }

    let had_live_db = dest.exists();
    if had_live_db {
        std::fs::rename(dest, &rollback)
            .map_err(|error| db_error("Failed to move current database aside for restore", error))?;
    }

    if let Err(promote_error) = std::fs::rename(&candidate, dest) {
        if had_live_db {
            let _ = std::fs::rename(&rollback, dest);
        }
        let _ = std::fs::remove_file(&candidate);
        return Err(db_error(
            "Failed to promote validated restore database",
            promote_error,
        ));
    }

    let promoted_result = (|| -> RepoDeskResult<()> {
        let conn = Connection::open(dest)
            .map_err(|error| db_error("Failed to reopen restored database", error))?;
        verify_integrity(&conn)?;
        verify_repodesk_schema(&conn)
    })();

    if let Err(error) = promoted_result {
        let _ = std::fs::remove_file(dest);
        if had_live_db {
            std::fs::rename(&rollback, dest)
                .map_err(|rollback_error| db_error("Restore failed and rollback failed", rollback_error))?;
        }
        return Err(error);
    }

    if had_live_db {
        std::fs::remove_file(&rollback)
            .map_err(|error| db_error("Restore succeeded but rollback file cleanup failed", error))?;
    }
    Ok(())
}

fn initialize_connection(conn: &Connection) -> RepoDeskResult<()> {
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
    .map_err(|error| db_error("Failed to create events table", error))?;

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
    .map_err(|error| db_error("Failed to create memory table", error))?;

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
    .map_err(|error| db_error("Failed to create token_ledger table", error))?;

    crate::persistence::migrations::run_migrations(conn)?;
    Ok(())
}

pub fn init_db() -> RepoDeskResult<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path)
        .map_err(|error| db_error("Failed to open DB", error))?;
    initialize_connection(&conn)?;
    Ok(conn)
}

// ── Memory Brain re-exports ──────────────────────────────────────────────────
// The memory model and storage moved to `crate::memory`. These shims keep the
// historical `persistence::db::*` import paths working for existing callers
// (CLI + Tauri commands).
pub use crate::memory::consolidate::consolidate_project_memory;
pub use crate::memory::model::MemoryEntry;
pub use crate::memory::store::{add_memory, list_memory};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_repodesk_db(path: &Path, message: &str) {
        let conn = Connection::open(path).unwrap();
        initialize_connection(&conn).unwrap();
        conn.execute(
            "INSERT INTO events (timestamp, project, task_id, module_name, level, message, metadata) VALUES (?1, 'p', 't', 'test', 'info', ?2, '{}')",
            ("2026-08-14T00:00:00Z", message),
        )
        .unwrap();
    }

    fn event_message(path: &Path) -> String {
        let conn = Connection::open(path).unwrap();
        conn.query_row("SELECT message FROM events ORDER BY id DESC LIMIT 1", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn restore_replaces_live_db_only_after_candidate_validation() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live.sqlite");
        let backup = dir.path().join("backup.sqlite");
        create_repodesk_db(&live, "current");
        create_repodesk_db(&backup, "restored");

        restore_database_file(&backup, &live).unwrap();

        assert_eq!(event_message(&live), "restored");
        validate_backup_source(&live).unwrap();
    }

    #[test]
    fn corrupt_restore_source_never_overwrites_live_db() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live.sqlite");
        let corrupt = dir.path().join("corrupt.sqlite");
        create_repodesk_db(&live, "keep-me");
        std::fs::write(&corrupt, b"not a sqlite database").unwrap();

        assert!(restore_database_file(&corrupt, &live).is_err());
        assert_eq!(event_message(&live), "keep-me");
    }

    #[test]
    fn unrelated_sqlite_file_is_rejected_before_live_db_changes() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live.sqlite");
        let unrelated = dir.path().join("other.sqlite");
        create_repodesk_db(&live, "keep-me");
        let conn = Connection::open(&unrelated).unwrap();
        conn.execute("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)", [])
            .unwrap();
        drop(conn);

        assert!(restore_database_file(&unrelated, &live).is_err());
        assert_eq!(event_message(&live), "keep-me");
    }
}
