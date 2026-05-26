use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DesktopStorageError {
    #[error("storage io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopDbStatus {
    pub exists: bool,
    pub path: String,
    pub schema_version: i64,
    pub tables: Vec<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredActionRun {
    pub id: i64,
    pub created_at: String,
    pub action: String,
    pub verdict: String,
    pub status: String,
    pub duration_ms: i64,
    pub output_preview: String,
}

pub fn db_path(repodesk_home: &Path) -> PathBuf {
    repodesk_home.join("repodesk.db")
}

pub fn init_db(repodesk_home: &Path) -> Result<DesktopDbStatus, DesktopStorageError> {
    fs::create_dir_all(repodesk_home)?;
    let path = db_path(repodesk_home);
    let conn = Connection::open(&path)?;
    run_migrations(&conn)?;
    read_db_status(repodesk_home)
}

pub fn read_db_status(repodesk_home: &Path) -> Result<DesktopDbStatus, DesktopStorageError> {
    let path = db_path(repodesk_home);
    if !path.exists() {
        return Ok(DesktopDbStatus {
            exists: false,
            path: path.display().to_string(),
            schema_version: 0,
            tables: Vec::new(),
            checked_at: Utc::now().to_rfc3339(),
        });
    }

    let conn = Connection::open(&path)?;
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DesktopDbStatus {
        exists: true,
        path: path.display().to_string(),
        schema_version,
        tables,
        checked_at: Utc::now().to_rfc3339(),
    })
}

pub fn record_action_run(
    repodesk_home: &Path,
    action: &str,
    verdict: &str,
    status: &str,
    duration_ms: i64,
    output: &str,
) -> Result<(), DesktopStorageError> {
    init_db(repodesk_home)?;
    let path = db_path(repodesk_home);
    let conn = Connection::open(&path)?;
    let output_preview = trim_output(output, 8_000);

    conn.execute(
        r#"
        INSERT INTO desktop_action_runs (
            created_at,
            action,
            verdict,
            status,
            duration_ms,
            output_preview
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            Utc::now().to_rfc3339(),
            action,
            verdict,
            status,
            duration_ms,
            output_preview
        ],
    )?;

    Ok(())
}

pub fn list_action_runs(
    repodesk_home: &Path,
    limit: usize,
) -> Result<Vec<StoredActionRun>, DesktopStorageError> {
    init_db(repodesk_home)?;
    let path = db_path(repodesk_home);
    let conn = Connection::open(&path)?;
    let limit = limit.clamp(1, 100) as i64;

    let mut stmt = conn.prepare(
        r#"
        SELECT id, created_at, action, verdict, status, duration_ms, output_preview
        FROM desktop_action_runs
        ORDER BY id DESC
        LIMIT ?1
        "#,
    )?;

    let rows = stmt
        .query_map([limit], |row| {
            Ok(StoredActionRun {
                id: row.get(0)?,
                created_at: row.get(1)?,
                action: row.get(2)?,
                verdict: row.get(3)?,
                status: row.get(4)?,
                duration_ms: row.get(5)?,
                output_preview: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

fn run_migrations(conn: &Connection) -> Result<(), DesktopStorageError> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            module TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_receipts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            agent TEXT NOT NULL,
            outcome TEXT NOT NULL,
            summary TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS knowledge_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            kind TEXT NOT NULL,
            text TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            name TEXT NOT NULL,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS security_audits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            verdict TEXT NOT NULL,
            findings_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS desktop_action_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            action TEXT NOT NULL,
            verdict TEXT NOT NULL,
            status TEXT NOT NULL,
            duration_ms INTEGER NOT NULL,
            output_preview TEXT NOT NULL
        );

        PRAGMA user_version=2;
        "#,
    )?;

    Ok(())
}

fn trim_output(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let trimmed = value.chars().take(max_chars).collect::<String>();
    format!("{trimmed}\n\n[RepoDesk desktop: output trimmed]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_home() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("repodesk-desktop-storage-test-{nonce}"))
    }

    #[test]
    fn init_db_creates_action_runs_table() {
        let home = temp_home();
        let status = init_db(&home).unwrap();
        assert!(status.exists);
        assert!(status.tables.contains(&"desktop_action_runs".to_string()));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn records_and_lists_action_runs() {
        let home = temp_home();
        record_action_run(&home, "workflow_next", "allow", "success", 12, "done").unwrap();
        let rows = list_action_runs(&home, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "workflow_next");
        let _ = fs::remove_dir_all(home);
    }
}
