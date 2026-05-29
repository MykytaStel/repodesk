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
        crate::errors::RepoDeskError::Database(format!("Failed to create token_ledger table: {}", e))
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

pub fn consolidate_project_memory(project_name: &str) -> RepoDeskResult<String> {
    let entries = list_memory(project_name)?;
    let mut markdown = format!(
        "# {} Memory\n\nProject-specific notes, constraints, and workflow memory.\n",
        project_name
    );

    use std::collections::BTreeMap;
    let mut categories: BTreeMap<String, Vec<MemoryEntry>> = BTreeMap::new();
    for entry in entries {
        categories
            .entry(entry.category.clone())
            .or_default()
            .push(entry);
    }

    for (category, cat_entries) in categories {
        markdown.push_str(&format!("\n## {}\n\n", category));
        for entry in cat_entries {
            let tags_str = if entry.tags.is_empty() {
                String::new()
            } else {
                format!(" (Tags: {})", entry.tags.join(", "))
            };
            markdown.push_str(&format!(
                "- **[{}]**{}:\n  {}\n\n",
                entry.timestamp.to_rfc3339(),
                tags_str,
                entry.content.replace("\n", "\n  ").trim_end()
            ));
        }
    }

    let paths = RepoDeskPaths::resolve()?;
    let project_dir = paths.project_dir(project_name);
    let memory_file = project_dir.join("memory.md");

    std::fs::create_dir_all(&project_dir)?;
    std::fs::write(&memory_file, &markdown)?;

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidate_project_memory_behavior() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
                let test_home = std::env::temp_dir().join(format!("repodesk-test-{now}"));
        std::fs::create_dir_all(&test_home).unwrap();
        unsafe {
            std::env::set_var("REPODESK_HOME", &test_home);
        }
        crate::init::init_home().unwrap();

        let project_name = "test_memory_project";

        // Add some memory entries
        let tags_1 = vec!["architecture".to_string(), "backend".to_string()];
        add_memory(
            project_name,
            "First note about design patterns",
            "Design",
            &tags_1,
        ).unwrap();

        let tags_2 = vec!["db".to_string()];
        add_memory(
            project_name,
            "Second note about schema design",
            "Database",
            &tags_2,
        ).unwrap();

        let tags_3 = vec![];
        add_memory(
            project_name,
            "Third note without tags",
            "Design",
            &tags_3,
        ).unwrap();

        // Let's consolidate project memory
        let md = consolidate_project_memory(project_name).unwrap();

        // Assert markdown structure and contents
        assert!(md.contains("# test_memory_project Memory"));
        assert!(md.contains("## Database"));
        assert!(md.contains("## Design"));
        assert!(md.contains("First note about design patterns"));
        assert!(md.contains("Second note about schema design"));
        assert!(md.contains("Third note without tags"));
        assert!(md.contains("(Tags: architecture, backend)"));
        assert!(md.contains("(Tags: db)"));

        // Verify the file was written to the correct location
        let paths = RepoDeskPaths::resolve().unwrap();
        let expected_file = paths.project_dir(project_name).join("memory.md");
        assert!(expected_file.exists());

        let file_content = std::fs::read_to_string(expected_file).unwrap();
        assert_eq!(file_content, md);

        // Clean up
        let _ = std::fs::remove_dir_all(&test_home);
    }
}


