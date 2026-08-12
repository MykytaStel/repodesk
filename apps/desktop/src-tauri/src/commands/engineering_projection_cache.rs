//! Desktop-only cache for deterministic engineering observability projections.
//!
//! The append-only event ledger remains authoritative and uncached for workflow
//! gates. This cache is used only by `work_engineering_intelligence` so repeated
//! UI reads do not reparse and re-derive an unchanged ledger.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use repodesk_core::engineering::{
    AiUsageReport, EngineeringEvent, EngineeringIntelligence, StrategyFeedbackReport,
    derive_ai_usage_report, derive_engineering_intelligence, derive_strategy_feedback,
    event_ledger_path, read_events,
};
use repodesk_core::RepoDeskResult;

const MAX_STABLE_READ_ATTEMPTS: usize = 3;
const MAX_CACHED_RUN_DIRS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerStamp {
    path: PathBuf,
    exists: bool,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug)]
pub(crate) struct EngineeringProjectionSnapshot {
    pub events: Arc<Vec<EngineeringEvent>>,
    pub intelligence: EngineeringIntelligence,
    pub ai_usage_report: AiUsageReport,
    pub strategy_feedback: StrategyFeedbackReport,
}

#[derive(Debug)]
struct ProjectionCacheEntry {
    stamp: LedgerStamp,
    snapshot: Arc<EngineeringProjectionSnapshot>,
}

static PROJECTION_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, ProjectionCacheEntry>>> = OnceLock::new();

pub(crate) fn load_engineering_projection(
    run_dir: &Path,
) -> RepoDeskResult<Arc<EngineeringProjectionSnapshot>> {
    let initial_stamp = ledger_stamp(run_dir)?;
    if let Some(snapshot) = cached_snapshot(&initial_stamp) {
        return Ok(snapshot);
    }

    for _ in 0..MAX_STABLE_READ_ATTEMPTS {
        let before = ledger_stamp(run_dir)?;
        let events = read_events(run_dir)?;
        let after = ledger_stamp(run_dir)?;
        let snapshot = derive_projection(events);
        if before == after {
            cache_snapshot(after, Arc::clone(&snapshot));
            return Ok(snapshot);
        }
    }

    // A writer kept changing the ledger while this observational read was in
    // flight. Return one fresh replay but deliberately do not cache it under an
    // unstable stamp. A later request can establish a stable snapshot.
    Ok(derive_projection(read_events(run_dir)?))
}

fn derive_projection(events: Vec<EngineeringEvent>) -> Arc<EngineeringProjectionSnapshot> {
    let intelligence = derive_engineering_intelligence(&events);
    let ai_usage_report = derive_ai_usage_report(&events, &intelligence);
    let strategy_feedback = derive_strategy_feedback(&events);
    Arc::new(EngineeringProjectionSnapshot {
        events: Arc::new(events),
        intelligence,
        ai_usage_report,
        strategy_feedback,
    })
}

fn ledger_stamp(run_dir: &Path) -> RepoDeskResult<LedgerStamp> {
    let path = event_ledger_path(run_dir);
    match fs::metadata(&path) {
        Ok(metadata) => Ok(LedgerStamp {
            path,
            exists: true,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LedgerStamp {
            path,
            exists: false,
            len: 0,
            modified: None,
        }),
        Err(error) => Err(error.into()),
    }
}

fn projection_cache() -> &'static Mutex<BTreeMap<PathBuf, ProjectionCacheEntry>> {
    PROJECTION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn cached_snapshot(stamp: &LedgerStamp) -> Option<Arc<EngineeringProjectionSnapshot>> {
    let cache = projection_cache().lock().ok()?;
    let entry = cache.get(&stamp.path)?;
    if entry.stamp == *stamp {
        Some(Arc::clone(&entry.snapshot))
    } else {
        None
    }
}

fn cache_snapshot(stamp: LedgerStamp, snapshot: Arc<EngineeringProjectionSnapshot>) {
    if let Ok(mut cache) = projection_cache().lock() {
        if !cache.contains_key(&stamp.path) && cache.len() >= MAX_CACHED_RUN_DIRS {
            let oldest_key = cache.keys().next().cloned();
            if let Some(key) = oldest_key {
                cache.remove(&key);
            }
        }
        cache.insert(
            stamp.path.clone(),
            ProjectionCacheEntry { stamp, snapshot },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodesk_core::engineering::{
        EngineeringEvent, EngineeringEventKind, WorkItemId, append_event,
    };
    use tempfile::tempdir;

    fn event(kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new(
            "RepoDesk",
            WorkItemId::try_new("task-cache").unwrap(),
            kind,
        )
    }

    #[test]
    fn unchanged_ledger_reuses_the_same_projection_snapshot() {
        let run_dir = tempdir().unwrap();
        append_event(
            run_dir.path(),
            &event(EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();

        let first = load_engineering_projection(run_dir.path()).unwrap();
        let second = load_engineering_projection(run_dir.path()).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.intelligence.event_count, 1);
    }

    #[test]
    fn appended_event_invalidates_cached_projection() {
        let run_dir = tempdir().unwrap();
        append_event(
            run_dir.path(),
            &event(EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        let first = load_engineering_projection(run_dir.path()).unwrap();

        append_event(
            run_dir.path(),
            &event(EngineeringEventKind::ContextBuilt),
        )
        .unwrap();
        let second = load_engineering_projection(run_dir.path()).unwrap();

        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(second.intelligence.event_count, 2);
    }

    #[test]
    fn changed_corrupt_ledger_is_not_hidden_by_previous_cache_entry() {
        let run_dir = tempdir().unwrap();
        append_event(
            run_dir.path(),
            &event(EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        load_engineering_projection(run_dir.path()).unwrap();

        fs::write(event_ledger_path(run_dir.path()), "{not-json}\nmore-corruption\n").unwrap();

        assert!(load_engineering_projection(run_dir.path()).is_err());
    }

    #[test]
    fn different_run_directories_keep_independent_cache_entries() {
        let first_dir = tempdir().unwrap();
        let second_dir = tempdir().unwrap();
        append_event(
            first_dir.path(),
            &event(EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        append_event(
            second_dir.path(),
            &event(EngineeringEventKind::ContextBuilt),
        )
        .unwrap();

        let first = load_engineering_projection(first_dir.path()).unwrap();
        let second = load_engineering_projection(second_dir.path()).unwrap();
        let first_again = load_engineering_projection(first_dir.path()).unwrap();

        assert!(!Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, &first_again));
        assert_ne!(first.events[0].kind, second.events[0].kind);
    }
}
