use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
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

    Ok(conn)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,
}

pub fn add_memory(
    project: &str,
    content: &str,
    category: &str,
    tags: &[String],
) -> RepoDeskResult<MemoryEntry> {
    let conn = init_db()?;
    let timestamp = Utc::now();
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO memory (timestamp, project, content, category, tags) VALUES (?1, ?2, ?3, ?4, ?5)",
        (timestamp.to_rfc3339(), project, content, category, &tags_json),
    ).map_err(|e| crate::errors::RepoDeskError::Database(format!("Failed to insert memory: {}", e)))?;

    let id = conn.last_insert_rowid();

    Ok(MemoryEntry {
        id,
        timestamp,
        project: project.to_string(),
        content: content.to_string(),
        category: category.to_string(),
        tags: tags.to_vec(),
    })
}

pub fn list_memory(project: &str) -> RepoDeskResult<Vec<MemoryEntry>> {
    let conn = init_db()?;
    let mut stmt = conn.prepare("SELECT id, timestamp, project, content, category, tags FROM memory WHERE project = ? ORDER BY timestamp DESC")
        .map_err(|e| crate::errors::RepoDeskError::Database(format!("DB prep error: {}", e)))?;

    let rows = stmt
        .query_map([project], |row: &rusqlite::Row| {
            let timestamp_str: String = row.get(1)?;
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(MemoryEntry {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                project: row.get(2)?,
                content: row.get(3)?,
                category: row.get(4)?,
                tags,
            })
        })
        .map_err(|e| crate::errors::RepoDeskError::Database(format!("DB query error: {}", e)))?;

    let mut entries = Vec::new();
    for entry in rows.flatten() {
        entries.push(entry);
    }

    Ok(entries)
}
