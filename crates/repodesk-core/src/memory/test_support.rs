//! Test-only helpers for memory DB integration tests.
//!
//! `REPODESK_HOME` is process-global, so DB tests must run serially or they
//! clobber each other. [`with_temp_home`] takes a shared lock for the duration
//! of the test and points the home at a unique temp dir.

use std::sync::Mutex;

/// Serializes every test that mutates the global `REPODESK_HOME`.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Run `test` with a fresh, isolated RepoDesk home. Serialized across the suite.
pub fn with_temp_home(test: impl FnOnce()) {
    // Recover from poisoning so one failing test doesn't cascade into the rest.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

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

    test();

    let _ = std::fs::remove_dir_all(&test_home);
}
