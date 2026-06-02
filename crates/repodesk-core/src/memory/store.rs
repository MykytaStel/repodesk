//! SQLite-backed storage for the Memory Brain.
//!
//! Reuses the shared connection from [`crate::persistence::db::init_db`] (which
//! also runs migrations), so callers never deal with schema setup directly.

use chrono::{DateTime, Utc};
use rusqlite::Row;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::db::init_db;

use super::model::{
    MemoryEntry, MemoryProposal, NewMemoryInput, NewProposal, ProposalPayload,
    compute_content_hash, status,
};

/// Columns selected for a full [`MemoryEntry`], in `row_to_entry` order.
const SELECT_COLUMNS: &str = "id, timestamp, project, content, category, tags, source, agent, \
     task_id, status, pinned, salience, confidence, supersedes_id, content_hash, updated_at";

fn db_err(context: &str, e: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {e}"))
}

fn row_to_entry(row: &Row) -> rusqlite::Result<MemoryEntry> {
    let timestamp_str: String = row.get(1)?;
    let tags_str: String = row.get(5)?;
    let updated_at_str: String = row.get(15)?;
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

    Ok(MemoryEntry {
        id: row.get(0)?,
        timestamp: parse_ts(&timestamp_str).unwrap_or_else(Utc::now),
        project: row.get(2)?,
        content: row.get(3)?,
        category: row.get(4)?,
        tags,
        source: row.get(6)?,
        agent: row.get(7)?,
        task_id: row.get(8)?,
        status: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        salience: row.get(11)?,
        confidence: row.get(12)?,
        supersedes_id: row.get(13)?,
        content_hash: row.get(14)?,
        updated_at: if updated_at_str.is_empty() {
            None
        } else {
            parse_ts(&updated_at_str)
        },
    })
}

fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Insert a fully-specified entry.
pub fn add_entry(input: NewMemoryInput) -> RepoDeskResult<MemoryEntry> {
    let conn = init_db()?;
    let timestamp = Utc::now();
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".to_string());
    let content_hash = compute_content_hash(&input.content);

    conn.execute(
        "INSERT INTO memory
            (timestamp, project, content, category, tags, source, agent, task_id, status,
             pinned, salience, confidence, supersedes_id, content_hash, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, ?13, ?1)",
        rusqlite::params![
            timestamp.to_rfc3339(),
            input.project,
            input.content,
            input.category,
            tags_json,
            input.source,
            input.agent,
            input.task_id,
            input.status,
            input.salience,
            input.confidence,
            input.supersedes_id,
            content_hash,
        ],
    )
    .map_err(|e| db_err("Failed to insert memory", e))?;

    let id = conn.last_insert_rowid();

    Ok(MemoryEntry {
        id,
        timestamp,
        project: input.project,
        content: input.content,
        category: input.category,
        tags: input.tags,
        source: input.source,
        agent: input.agent,
        task_id: input.task_id,
        status: input.status,
        pinned: false,
        salience: input.salience,
        confidence: input.confidence,
        supersedes_id: input.supersedes_id,
        content_hash,
        updated_at: Some(timestamp),
    })
}

/// Backward-compatible helper: add a human-authored entry.
pub fn add_memory(
    project: &str,
    content: &str,
    category: &str,
    tags: &[String],
) -> RepoDeskResult<MemoryEntry> {
    add_entry(NewMemoryInput::human(project, content, category, tags))
}

/// All entries for a project, newest first (any status). Kept for the existing
/// UI/CLI which filter client-side.
pub fn list_memory(project: &str) -> RepoDeskResult<Vec<MemoryEntry>> {
    query_entries(
        &format!("SELECT {SELECT_COLUMNS} FROM memory WHERE project = ?1 ORDER BY timestamp DESC"),
        rusqlite::params![project],
    )
}

/// Only `active` entries for a project — the set the brain reasons over.
pub fn list_active(project: &str) -> RepoDeskResult<Vec<MemoryEntry>> {
    query_entries(
        &format!(
            "SELECT {SELECT_COLUMNS} FROM memory
             WHERE project = ?1 AND status = '{ACTIVE}'
             ORDER BY pinned DESC, timestamp DESC",
            ACTIVE = status::ACTIVE
        ),
        rusqlite::params![project],
    )
}

/// Fetch a single entry by id.
pub fn get_entry(id: i64) -> RepoDeskResult<Option<MemoryEntry>> {
    let mut entries = query_entries(
        &format!("SELECT {SELECT_COLUMNS} FROM memory WHERE id = ?1"),
        rusqlite::params![id],
    )?;
    Ok(entries.pop())
}

/// Case-insensitive search over content, category, and tags.
pub fn search_entries(project: &str, query: &str) -> RepoDeskResult<Vec<MemoryEntry>> {
    let needle = format!("%{}%", query.trim().to_lowercase());
    query_entries(
        &format!(
            "SELECT {SELECT_COLUMNS} FROM memory
             WHERE project = ?1
               AND (LOWER(content) LIKE ?2 OR LOWER(category) LIKE ?2 OR LOWER(tags) LIKE ?2)
             ORDER BY pinned DESC, timestamp DESC"
        ),
        rusqlite::params![project, needle],
    )
}

/// Edit an entry's content/category/tags; refreshes `content_hash` + `updated_at`.
pub fn update_entry(
    id: i64,
    content: &str,
    category: &str,
    tags: &[String],
) -> RepoDeskResult<MemoryEntry> {
    let conn = init_db()?;
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    let content_hash = compute_content_hash(content);
    let now = Utc::now().to_rfc3339();

    let changed = conn
        .execute(
            "UPDATE memory SET content = ?2, category = ?3, tags = ?4, content_hash = ?5,
                updated_at = ?6 WHERE id = ?1",
            rusqlite::params![id, content, category, tags_json, content_hash, now],
        )
        .map_err(|e| db_err("Failed to update memory", e))?;

    if changed == 0 {
        return Err(RepoDeskError::Database(format!(
            "memory entry {id} was not found"
        )));
    }

    get_entry(id)?.ok_or_else(|| RepoDeskError::Database(format!("memory entry {id} not found")))
}

pub fn delete_entry(id: i64) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute("DELETE FROM memory WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| db_err("Failed to delete memory", e))?;
    Ok(())
}

pub fn set_pinned(id: i64, pinned: bool) -> RepoDeskResult<()> {
    set_int_field(id, "pinned", if pinned { 1 } else { 0 })
}

pub fn set_status(id: i64, new_status: &str) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute(
        "UPDATE memory SET status = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![id, new_status, Utc::now().to_rfc3339()],
    )
    .map_err(|e| db_err("Failed to set status", e))?;
    Ok(())
}

/// Mark an entry superseded by another (used when a merge/dedup is accepted).
pub fn mark_superseded(id: i64, by_id: i64) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute(
        "UPDATE memory SET status = ?2, supersedes_id = ?3, updated_at = ?4 WHERE id = ?1",
        rusqlite::params![id, status::SUPERSEDED, by_id, Utc::now().to_rfc3339()],
    )
    .map_err(|e| db_err("Failed to mark superseded", e))?;
    Ok(())
}

fn set_int_field(id: i64, field: &str, value: i64) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute(
        &format!("UPDATE memory SET {field} = ?2, updated_at = ?3 WHERE id = ?1"),
        rusqlite::params![id, value, Utc::now().to_rfc3339()],
    )
    .map_err(|e| db_err("Failed to update field", e))?;
    Ok(())
}

fn query_entries(sql: &str, params: impl rusqlite::Params) -> RepoDeskResult<Vec<MemoryEntry>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| db_err("DB prepare error", e))?;
    let rows = stmt
        .query_map(params, row_to_entry)
        .map_err(|e| db_err("DB query error", e))?;
    Ok(rows.flatten().collect())
}

// ── Proposals (human-approval queue) ─────────────────────────────────────────

const PROPOSAL_COLUMNS: &str =
    "id, created_at, project, task_id, kind, status, payload, applied_entry_id";

fn row_to_proposal(row: &Row) -> rusqlite::Result<MemoryProposal> {
    let created_at_str: String = row.get(1)?;
    let payload_str: String = row.get(6)?;
    let payload: ProposalPayload = serde_json::from_str(&payload_str).unwrap_or_default();

    Ok(MemoryProposal {
        id: row.get(0)?,
        created_at: parse_ts(&created_at_str).unwrap_or_else(Utc::now),
        project: row.get(2)?,
        task_id: row.get(3)?,
        kind: row.get(4)?,
        status: row.get(5)?,
        payload,
        applied_entry_id: row.get(7)?,
    })
}

/// Create a pending proposal.
pub fn add_proposal(input: NewProposal) -> RepoDeskResult<MemoryProposal> {
    let conn = init_db()?;
    let created_at = Utc::now();
    let payload_json = serde_json::to_string(&input.payload).unwrap_or_else(|_| "{}".to_string());

    conn.execute(
        "INSERT INTO memory_proposals (created_at, project, task_id, kind, status, payload)
         VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
        rusqlite::params![
            created_at.to_rfc3339(),
            input.project,
            input.task_id,
            input.kind,
            payload_json,
        ],
    )
    .map_err(|e| db_err("Failed to insert proposal", e))?;

    Ok(MemoryProposal {
        id: conn.last_insert_rowid(),
        created_at,
        project: input.project,
        task_id: input.task_id,
        kind: input.kind,
        status: super::model::proposal_status::PENDING.to_string(),
        payload: input.payload,
        applied_entry_id: None,
    })
}

/// List proposals for a project, optionally filtered by status, newest first.
pub fn list_proposals(
    project: &str,
    status_filter: Option<&str>,
) -> RepoDeskResult<Vec<MemoryProposal>> {
    let conn = init_db()?;
    let (sql, params): (String, Vec<String>) = match status_filter {
        Some(s) => (
            format!(
                "SELECT {PROPOSAL_COLUMNS} FROM memory_proposals
                 WHERE project = ?1 AND status = ?2 ORDER BY created_at DESC, id DESC"
            ),
            vec![project.to_string(), s.to_string()],
        ),
        None => (
            format!(
                "SELECT {PROPOSAL_COLUMNS} FROM memory_proposals
                 WHERE project = ?1 ORDER BY created_at DESC, id DESC"
            ),
            vec![project.to_string()],
        ),
    };
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| db_err("DB prepare error", e))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), row_to_proposal)
        .map_err(|e| db_err("DB query error", e))?;
    Ok(rows.flatten().collect())
}

pub fn get_proposal(id: i64) -> RepoDeskResult<Option<MemoryProposal>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {PROPOSAL_COLUMNS} FROM memory_proposals WHERE id = ?1"
        ))
        .map_err(|e| db_err("DB prepare error", e))?;
    let mut rows = stmt
        .query_map(rusqlite::params![id], row_to_proposal)
        .map_err(|e| db_err("DB query error", e))?;
    Ok(rows.next().and_then(Result::ok))
}

pub fn set_proposal_status(
    id: i64,
    new_status: &str,
    applied_entry_id: Option<i64>,
) -> RepoDeskResult<()> {
    let conn = init_db()?;
    conn.execute(
        "UPDATE memory_proposals SET status = ?2, applied_entry_id = ?3 WHERE id = ?1",
        rusqlite::params![id, new_status, applied_entry_id],
    )
    .map_err(|e| db_err("Failed to update proposal", e))?;
    Ok(())
}

/// Count pending proposals for a project (drives the UI badge).
pub fn count_pending(project: &str) -> RepoDeskResult<usize> {
    let conn = init_db()?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_proposals WHERE project = ?1 AND status = 'pending'",
            rusqlite::params![project],
            |row| row.get(0),
        )
        .map_err(|e| db_err("Failed to count proposals", e))?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::test_support::with_temp_home;

    #[test]
    fn add_list_update_pin_status_delete() {
        with_temp_home(|| {
            let project = "store_demo";
            let entry = add_memory(project, "First note", "decision", &["db".to_string()]).unwrap();
            assert_eq!(entry.source, "human");
            assert_eq!(entry.status, "active");
            assert!(!entry.content_hash.is_empty());

            let listed = list_memory(project).unwrap();
            assert_eq!(listed.len(), 1);

            let updated = update_entry(entry.id, "First note edited", "constraint", &[]).unwrap();
            assert_eq!(updated.content, "First note edited");
            assert_eq!(updated.category, "constraint");
            assert!(updated.updated_at.is_some());

            set_pinned(entry.id, true).unwrap();
            let pinned = get_entry(entry.id).unwrap().unwrap();
            assert!(pinned.pinned);

            set_status(entry.id, "archived").unwrap();
            assert!(list_active(project).unwrap().is_empty());

            let found = search_entries(project, "edited").unwrap();
            assert_eq!(found.len(), 1);

            delete_entry(entry.id).unwrap();
            assert!(list_memory(project).unwrap().is_empty());
        });
    }
}
