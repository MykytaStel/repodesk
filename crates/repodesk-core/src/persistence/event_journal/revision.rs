use rusqlite::{OptionalExtension, params};

use crate::errors::RepoDeskResult;
use crate::persistence::db::init_db;

use super::{db_err, ensure_legacy_jsonl_import};

/// Cheap identity for the latest canonical event in one Work Item.
///
/// This is intentionally scoped so an unrelated Work Item does not invalidate
/// observational caches. It does not replace full hash-chain verification when
/// events are actually replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineeringEventRevision {
    pub sequence: i64,
    pub event_hash: String,
}

pub fn engineering_event_revision(
    project: &str,
    work_item_id: &str,
) -> RepoDeskResult<Option<EngineeringEventRevision>> {
    ensure_legacy_jsonl_import()?;
    let conn = init_db()?;
    conn.query_row(
        "SELECT sequence, event_hash
         FROM engineering_events
         WHERE project = ?1 AND work_item_id = ?2
         ORDER BY sequence DESC
         LIMIT 1",
        params![project, work_item_id],
        |row| {
            Ok(EngineeringEventRevision {
                sequence: row.get(0)?,
                event_hash: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(|error| db_err("Failed to read Work Item event revision", error))
}
