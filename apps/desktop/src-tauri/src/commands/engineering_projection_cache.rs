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

use repodesk_core::RepoDeskResult;
use repodesk_core::engineering::{
    AiUsageReport, EngineeringEvent, EngineeringIntelligence, EventLedgerRevision,
    StrategyFeedbackReport, derive_ai_usage_report, derive_engineering_intelligence,
    derive_strategy_feedback, event_ledger_path, event_ledger_revision, read_events,
};

const MAX_STABLE_READ_ATTEMPTS: usize = 3;
const MAX_CACHED_RUN_DIRS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerStamp {
    path: PathBuf,
    canonical: Option<EventLedgerRevision>,
    legacy_exists: bool,
    legacy_len: u64,
    legacy_modified: Option<SystemTime>,
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
    let canonical = event_ledger_revision(run_dir)?;
    match fs::metadata(&path) {
        Ok(metadata) => Ok(LedgerStamp {
            path,
            canonical,
            legacy_exists: true,
            legacy_len: metadata.len(),
            legacy_modified: metadata.modified().ok(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LedgerStamp {
            path,
            canonical,
            legacy_exists: false,
            legacy_len: 0,
            legacy_modified: None,
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
        cache.insert(stamp.path.clone(), ProjectionCacheEntry { stamp, snapshot });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodesk_core::engineering::{
        EngineeringEvent, EngineeringEventKind, WorkItemId, append_event,
    };
    use serial_test::serial;
    use tempfile::TempDir;

    const PROJECT: &str = "RepoDesk";

    fn isolated_home() -> TempDir {
        let home = tempfile::tempdir().unwrap();
        // SAFETY: every test in this module is serialized because REPODESK_HOME
        // is process-global.
        unsafe {
            std::env::set_var("REPODESK_HOME", home.path());
        }
        repodesk_core::init::init_home().unwrap();
        home
    }

    fn run_dir(home: &TempDir, work_item_id: &str) -> PathBuf {
        let run_dir = home.path().join("runs").join(PROJECT).join(work_item_id);
        fs::create_dir_all(&run_dir).unwrap();
        run_dir
    }

    fn event(work_item_id: &str, kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new(
            PROJECT,
            WorkItemId::try_new(work_item_id).unwrap(),
            kind,
        )
    }

    #[test]
    #[serial]
    fn unchanged_ledger_reuses_the_same_projection_snapshot() {
        let home = isolated_home();
        let run_dir = run_dir(&home, "task-cache");
        append_event(
            &run_dir,
            &event("task-cache", EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();

        let first = load_engineering_projection(&run_dir).unwrap();
        let second = load_engineering_projection(&run_dir).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.intelligence.event_count, 1);
    }

    #[test]
    #[serial]
    fn appended_event_invalidates_cached_projection() {
        let home = isolated_home();
        let run_dir = run_dir(&home, "task-cache");
        append_event(
            &run_dir,
            &event("task-cache", EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        let first = load_engineering_projection(&run_dir).unwrap();

        append_event(
            &run_dir,
            &event("task-cache", EngineeringEventKind::ContextBuilt),
        )
        .unwrap();
        let second = load_engineering_projection(&run_dir).unwrap();

        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(second.intelligence.event_count, 2);
    }

    #[test]
    #[serial]
    fn unrelated_work_item_does_not_invalidate_cached_projection() {
        let home = isolated_home();
        let first_dir = run_dir(&home, "task-cache-a");
        let second_dir = run_dir(&home, "task-cache-b");
        append_event(
            &first_dir,
            &event("task-cache-a", EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        let first = load_engineering_projection(&first_dir).unwrap();

        append_event(
            &second_dir,
            &event("task-cache-b", EngineeringEventKind::ContextBuilt),
        )
        .unwrap();
        let first_again = load_engineering_projection(&first_dir).unwrap();

        assert!(Arc::ptr_eq(&first, &first_again));
        assert_eq!(first_again.intelligence.event_count, 1);
    }

    #[test]
    #[serial]
    fn changed_corrupt_legacy_ledger_is_not_hidden_by_previous_cache_entry() {
        let home = isolated_home();
        let run_dir = run_dir(&home, "task-cache");
        append_event(
            &run_dir,
            &event("task-cache", EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        load_engineering_projection(&run_dir).unwrap();

        fs::write(
            event_ledger_path(&run_dir),
            "{not-json}\nmore-corruption\n",
        )
        .unwrap();

        assert!(load_engineering_projection(&run_dir).is_err());
    }

    #[test]
    #[serial]
    fn different_work_items_keep_independent_cache_entries() {
        let home = isolated_home();
        let first_dir = run_dir(&home, "task-cache-a");
        let second_dir = run_dir(&home, "task-cache-b");
        append_event(
            &first_dir,
            &event("task-cache-a", EngineeringEventKind::WorkItemCreated),
        )
        .unwrap();
        append_event(
            &second_dir,
            &event("task-cache-b", EngineeringEventKind::ContextBuilt),
        )
        .unwrap();

        let first = load_engineering_projection(&first_dir).unwrap();
        let second = load_engineering_projection(&second_dir).unwrap();
        let first_again = load_engineering_projection(&first_dir).unwrap();

        assert!(!Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, &first_again));
        assert_ne!(first.events[0].kind, second.events[0].kind);
    }
}
