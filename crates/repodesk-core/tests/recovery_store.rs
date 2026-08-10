use chrono::{DateTime, Duration, TimeZone, Utc};
use repodesk_core::recovery::{
    RecoveryAction, RecoveryActionKind, RecoveryEngine, RecoveryFailureCode, RecoveryObservation,
    RecoverySeverity, RecoveryState, RecoveryStore, RepairCompletion,
};
use std::fs;

const CAPABILITY_ID: &str = "language:typescript-language-server";
const ACTION_ID: &str = "install-managed-language-server";

fn base() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap()
}

fn missing_language(generation: u64, observed_at: DateTime<Utc>) -> RecoveryObservation {
    RecoveryObservation {
        capability_id: CAPABILITY_ID.into(),
        module_id: "language_intelligence".into(),
        generation,
        observed_at,
        state: RecoveryState::NeedsApproval,
        severity: RecoverySeverity::Warning,
        code: Some(RecoveryFailureCode::MissingExecutable),
        title: "TypeScript intelligence is unavailable".into(),
        explanation: "The language server was not found.".into(),
        affected: vec!["Hover".into(), "Definitions".into()],
        unaffected: vec!["Editing".into(), "Save".into()],
        evidence: vec![],
        actions: vec![RecoveryAction {
            id: ACTION_ID.into(),
            label: "Review repair".into(),
            kind: RecoveryActionKind::Confirmable,
            recipe_id: Some("typescript-language-server".into()),
        }],
    }
}

fn add_failed_attempt(engine: &mut RecoveryEngine, generation: u64, started_at: DateTime<Utc>) {
    engine.observe(missing_language(generation, started_at));
    engine
        .begin_repair(CAPABILITY_ID, ACTION_ID, started_at)
        .unwrap();
    engine
        .finish_repair(
            CAPABILITY_ID,
            RepairCompletion::Failed {
                finished_at: started_at + Duration::seconds(1),
                summary: format!("attempt {generation} failed"),
            },
        )
        .unwrap();
}

#[test]
fn records_and_history_survive_atomic_reload() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recovery-state.json");
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    add_failed_attempt(&mut engine, 1, base());

    RecoveryStore::save(&path, &engine).unwrap();
    let loaded = RecoveryStore::load(&path, "RepoDesk".into(), 100).unwrap();

    assert_eq!(loaded.snapshot().records, engine.snapshot().records);
    assert_eq!(loaded.history(), engine.history());
    assert!(!path.with_extension("json.staging").exists());
}

#[test]
fn corrupt_state_is_reported_without_preventing_in_memory_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recovery-state.json");
    fs::write(&path, b"{not-json").unwrap();

    let error = RecoveryStore::load(&path, "RepoDesk".into(), 100).unwrap_err();
    assert!(error.to_string().contains("JSON"));

    let fallback = RecoveryEngine::new("RepoDesk".into(), 100);
    assert!(fallback.snapshot().records.is_empty());
    assert!(fallback.history().is_empty());
}

#[test]
fn history_retains_only_the_newest_hundred_attempts_within_thirty_days() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    let cutoff = base() - Duration::days(30);

    for generation in 1..=3 {
        add_failed_attempt(
            &mut engine,
            generation,
            cutoff - Duration::days(generation as i64),
        );
    }
    for generation in 4..=106 {
        add_failed_attempt(
            &mut engine,
            generation,
            cutoff + Duration::hours(generation as i64),
        );
    }

    engine.prune_history(cutoff);
    let history = engine.history();
    assert_eq!(history.len(), 100);
    assert!(
        history
            .iter()
            .all(|attempt| attempt.finished_at.is_some_and(|time| time >= cutoff))
    );
    assert_eq!(
        history.last().unwrap().verification_summary.as_deref(),
        Some("attempt 106 failed")
    );
}
