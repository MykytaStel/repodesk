use std::collections::{BTreeMap, VecDeque};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::db::init_db;

use super::{
    EngineeringEvent, EventEntry, EventJournalSnapshot, EventSeverity, StoredEvent, db_err,
    ensure_legacy_jsonl_import,
};

fn scan_verified_stored(
    mut visit: impl FnMut(StoredEvent) -> RepoDeskResult<()>,
) -> RepoDeskResult<()> {
    ensure_legacy_jsonl_import()?;
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT sequence, schema_version, timestamp, project, work_item_id, run_id,
                    kind, module_name, level, message, payload, prev_hash, event_hash
             FROM engineering_events ORDER BY sequence ASC",
        )
        .map_err(|error| db_err("Failed to prepare engineering event query", error))?;
    let rows = stmt
        .query_map([], StoredEvent::from_row)
        .map_err(|error| db_err("Failed to read engineering events", error))?;

    let mut expected_sequence = 1i64;
    let mut expected_prev_hash = String::new();

    for row in rows {
        let stored = row.map_err(|error| db_err("Failed to decode engineering event row", error))?;
        stored.verify_hash()?;
        if stored.sequence != expected_sequence {
            return Err(RepoDeskError::Database(format!(
                "engineering event ledger sequence gap: expected {}, found {}",
                expected_sequence, stored.sequence
            )));
        }
        if stored.prev_hash != expected_prev_hash {
            return Err(RepoDeskError::Database(format!(
                "engineering event ledger chain mismatch at sequence {}",
                stored.sequence
            )));
        }

        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| RepoDeskError::Database("event sequence overflow".to_string()))?;
        expected_prev_hash.clone_from(&stored.event_hash);
        visit(stored)?;
    }

    Ok(())
}

fn retain_latest<T>(buffer: &mut VecDeque<T>, value: T, limit: usize) {
    if limit == 0 {
        return;
    }
    buffer.push_back(value);
    if buffer.len() > limit {
        buffer.pop_front();
    }
}

fn decode_newest(buffer: VecDeque<StoredEvent>) -> RepoDeskResult<Vec<EngineeringEvent>> {
    buffer
        .into_iter()
        .rev()
        .map(StoredEvent::into_event)
        .collect()
}

/// Canonical engineering events, newest first. The complete hash chain is
/// verified, but only the latest `limit` raw rows are retained in memory.
pub(super) fn read_engineering_events(limit: usize) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let mut retained = VecDeque::new();
    scan_verified_stored(|stored| {
        retain_latest(&mut retained, stored, limit);
        Ok(())
    })?;
    decode_newest(retained)
}

/// Canonical engineering events for an optional project/Work Item scope,
/// newest first. Filtering happens only after each row passes integrity checks.
pub(crate) fn read_engineering_events_for_scope(
    project: Option<&str>,
    work_item_id: Option<&str>,
) -> RepoDeskResult<Vec<EngineeringEvent>> {
    let mut retained = VecDeque::new();
    scan_verified_stored(|stored| {
        let matches_project = project.is_none_or(|project| stored.project == project);
        let matches_work_item =
            work_item_id.is_none_or(|work_item_id| stored.work_item_id == work_item_id);
        if matches_project && matches_work_item {
            retained.push_back(stored);
        }
        Ok(())
    })?;
    decode_newest(retained)
}

pub(super) fn read_events(limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    Ok(read_engineering_events(limit)?
        .into_iter()
        .map(EventEntry::from)
        .collect())
}

/// The most recent events for a single Work Item, newest first. The whole
/// ledger is still verified, while retained memory is bounded by `limit`.
pub(super) fn read_task_events(task_id: &str, limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    let mut retained = VecDeque::new();
    scan_verified_stored(|stored| {
        if stored.work_item_id == task_id {
            retain_latest(&mut retained, stored, limit);
        }
        Ok(())
    })?;
    Ok(decode_newest(retained)?
        .into_iter()
        .map(EventEntry::from)
        .collect())
}

/// Build a journal snapshot in one verified pass. Aggregate counters cover the
/// complete ledger; only the latest requested entries are retained.
pub(super) fn try_journal_snapshot(limit: usize) -> RepoDeskResult<EventJournalSnapshot> {
    let mut total_entries = 0usize;
    let mut counts_by_severity: BTreeMap<String, usize> = BTreeMap::new();
    let mut retained = VecDeque::new();

    scan_verified_stored(|stored| {
        total_entries = total_entries.saturating_add(1);
        let severity = EventSeverity::from_str_lossy(&stored.level);
        *counts_by_severity
            .entry(severity.as_str().to_string())
            .or_insert(0) += 1;
        retain_latest(&mut retained, stored, limit);
        Ok(())
    })?;

    let entries = decode_newest(retained)?
        .into_iter()
        .map(EventEntry::from)
        .collect::<Vec<_>>();
    let returned = entries.len();

    Ok(EventJournalSnapshot {
        generated_at: chrono::Utc::now(),
        total_entries,
        returned,
        counts_by_severity,
        entries,
    })
}

pub(super) fn journal_snapshot(limit: usize) -> EventJournalSnapshot {
    try_journal_snapshot(limit).unwrap_or_else(|_| EventJournalSnapshot {
        generated_at: chrono::Utc::now(),
        total_entries: 0,
        returned: 0,
        counts_by_severity: BTreeMap::new(),
        entries: Vec::new(),
    })
}

/// Visit every canonical event oldest-first after verifying each hash-chain
/// link. The caller controls the sink, so exports can remain O(one event).
pub(super) fn visit_verified_events(
    mut visit: impl FnMut(EngineeringEvent) -> RepoDeskResult<()>,
) -> RepoDeskResult<()> {
    scan_verified_stored(|stored| visit(stored.into_event()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::event_journal::{EngineeringEventInput, append_engineering_event};
    use serial_test::serial;
    use tempfile::TempDir;

    #[test]
    fn bounded_retention_never_exceeds_the_requested_limit() {
        let mut retained = VecDeque::new();
        for value in 0..1_000 {
            retain_latest(&mut retained, value, 3);
            assert!(retained.len() <= 3);
        }
        assert_eq!(retained.into_iter().collect::<Vec<_>>(), vec![997, 998, 999]);
    }

    #[test]
    fn zero_limit_retains_nothing() {
        let mut retained = VecDeque::new();
        retain_latest(&mut retained, 1, 0);
        assert!(retained.is_empty());
    }

    #[test]
    #[serial]
    fn scoped_reads_still_fail_on_corruption_outside_the_scope() {
        let home = TempDir::new().expect("temp home");
        // SAFETY: this test is serialized because REPODESK_HOME is process-global.
        unsafe {
            std::env::set_var("REPODESK_HOME", home.path());
        }
        crate::init::init_home().expect("init home");

        let input = |work_item_id: &str, message: &str| EngineeringEventInput {
            project: "demo".to_string(),
            work_item_id: work_item_id.to_string(),
            run_id: String::new(),
            kind: "test".to_string(),
            module_name: "test".to_string(),
            level: "info".to_string(),
            message: message.to_string(),
            payload: BTreeMap::new(),
        };
        append_engineering_event(input("wanted", "wanted event")).expect("wanted event");
        append_engineering_event(input("other", "other event")).expect("other event");

        let conn = init_db().expect("db");
        conn.execute(
            "UPDATE engineering_events SET message = 'tampered' WHERE work_item_id = 'other'",
            [],
        )
        .expect("tamper unrelated event");

        let error = read_engineering_events_for_scope(Some("demo"), Some("wanted"))
            .expect_err("unrelated corruption must still fail closed");
        assert!(error.to_string().contains("hash mismatch"));
    }
}
