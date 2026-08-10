use chrono::{DateTime, Duration, TimeZone, Utc};
use repodesk_core::recovery::{
    ObserveOutcome, RecoveryAction, RecoveryActionKind, RecoveryEngine, RecoveryFailureCode,
    RecoveryObservation, RecoverySeverity, RecoveryState, RepairCompletion,
};

const CAPABILITY_ID: &str = "language:typescript-language-server";
const CONFIRMABLE_ACTION: &str = "install-managed-language-server";
const AUTOMATIC_ACTION: &str = "restart-language-session";

fn at(second: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap() + Duration::seconds(second)
}

fn action(id: &str, kind: RecoveryActionKind) -> RecoveryAction {
    RecoveryAction {
        id: id.into(),
        label: match kind {
            RecoveryActionKind::Automatic => "Restart language session",
            RecoveryActionKind::Confirmable => "Review repair",
            RecoveryActionKind::Manual => "Show manual steps",
        }
        .into(),
        kind,
        recipe_id: (kind == RecoveryActionKind::Confirmable)
            .then(|| "typescript-language-server".into()),
    }
}

fn observation(
    generation: u64,
    state: RecoveryState,
    code: Option<RecoveryFailureCode>,
    actions: Vec<RecoveryAction>,
) -> RecoveryObservation {
    RecoveryObservation {
        capability_id: CAPABILITY_ID.into(),
        module_id: "language_intelligence".into(),
        generation,
        observed_at: at(generation as i64),
        state,
        severity: if state == RecoveryState::Healthy {
            RecoverySeverity::Info
        } else {
            RecoverySeverity::Warning
        },
        code,
        title: if state == RecoveryState::Healthy {
            "TypeScript intelligence is ready"
        } else {
            "TypeScript intelligence is unavailable"
        }
        .into(),
        explanation: "Editing and save remain available.".into(),
        affected: vec!["Hover".into(), "Definitions".into()],
        unaffected: vec!["Editing".into(), "Save".into()],
        evidence: vec![],
        actions,
    }
}

fn missing_language(generation: u64) -> RecoveryObservation {
    observation(
        generation,
        RecoveryState::NeedsApproval,
        Some(RecoveryFailureCode::MissingExecutable),
        vec![action(CONFIRMABLE_ACTION, RecoveryActionKind::Confirmable)],
    )
}

#[test]
fn stale_observation_cannot_overwrite_newer_health() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    let applied = engine.observe(missing_language(2));
    assert!(matches!(applied, ObserveOutcome::Applied(_)));

    let stale = observation(1, RecoveryState::Healthy, None, vec![]);
    assert_eq!(engine.observe(stale), ObserveOutcome::IgnoredStale);

    assert_eq!(
        engine.snapshot().records[0].state,
        RecoveryState::NeedsApproval
    );
}

#[test]
fn failed_verification_never_becomes_healthy() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(missing_language(1));
    engine
        .begin_repair(CAPABILITY_ID, CONFIRMABLE_ACTION, at(2))
        .unwrap();
    let record = engine
        .finish_repair(
            CAPABILITY_ID,
            RepairCompletion::VerificationFailed {
                finished_at: at(3),
                summary: "Server installed but initialization failed".into(),
            },
        )
        .unwrap();

    assert_eq!(record.state, RecoveryState::Degraded);
    assert_eq!(
        engine.history()[0].verification_summary.as_deref(),
        Some("Server installed but initialization failed")
    );
}

#[test]
fn healthy_records_are_not_actionable() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(observation(1, RecoveryState::Healthy, None, vec![]));

    let snapshot = engine.snapshot();
    assert_eq!(snapshot.actionable_count, 0);
    assert!(snapshot.records[0].actions.is_empty());
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn automatic_attempt_budget_stops_after_two_failures() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(observation(
        1,
        RecoveryState::Degraded,
        Some(RecoveryFailureCode::InitializationFailed),
        vec![action(AUTOMATIC_ACTION, RecoveryActionKind::Automatic)],
    ));

    for attempt in 0..2 {
        engine
            .begin_repair(CAPABILITY_ID, AUTOMATIC_ACTION, at(2 + attempt * 2))
            .unwrap();
        engine
            .finish_repair(
                CAPABILITY_ID,
                RepairCompletion::Failed {
                    finished_at: at(3 + attempt * 2),
                    summary: "Restart did not initialize the server".into(),
                },
            )
            .unwrap();
    }

    let record = &engine.snapshot().records[0];
    assert_eq!(record.automatic_attempts, 2);
    assert_eq!(record.state, RecoveryState::Blocked);
    assert!(
        record
            .actions
            .iter()
            .all(|candidate| candidate.kind != RecoveryActionKind::Automatic)
    );
    assert!(
        engine
            .begin_repair(CAPABILITY_ID, AUTOMATIC_ACTION, at(8))
            .is_err()
    );
}

#[test]
fn repeated_observation_of_same_failure_preserves_automatic_attempt_budget() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    for generation in 1..=2 {
        engine.observe(observation(
            generation,
            RecoveryState::Degraded,
            Some(RecoveryFailureCode::InitializationFailed),
            vec![action(AUTOMATIC_ACTION, RecoveryActionKind::Automatic)],
        ));
        engine
            .begin_repair(CAPABILITY_ID, AUTOMATIC_ACTION, at(generation as i64 * 3))
            .unwrap();
        engine
            .finish_repair(
                CAPABILITY_ID,
                RepairCompletion::Failed {
                    finished_at: at(generation as i64 * 3 + 1),
                    summary: "Restart did not initialize the server".into(),
                },
            )
            .unwrap();
    }

    engine.observe(observation(
        3,
        RecoveryState::Degraded,
        Some(RecoveryFailureCode::InitializationFailed),
        vec![action(AUTOMATIC_ACTION, RecoveryActionKind::Automatic)],
    ));
    let record = &engine.snapshot().records[0];
    assert_eq!(record.automatic_attempts, 2);
    assert!(
        record
            .actions
            .iter()
            .all(|candidate| candidate.kind != RecoveryActionKind::Automatic)
    );
}

#[test]
fn history_is_bounded() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 2);

    for generation in 1..=3 {
        engine.observe(missing_language(generation));
        engine
            .begin_repair(CAPABILITY_ID, CONFIRMABLE_ACTION, at(generation as i64 * 3))
            .unwrap();
        engine
            .finish_repair(
                CAPABILITY_ID,
                RepairCompletion::Failed {
                    finished_at: at(generation as i64 * 3 + 1),
                    summary: format!("attempt {generation} failed"),
                },
            )
            .unwrap();
    }

    let history = engine.history();
    assert_eq!(history.len(), 2);
    assert_eq!(
        history[0].verification_summary.as_deref(),
        Some("attempt 2 failed")
    );
    assert_eq!(
        history[1].verification_summary.as_deref(),
        Some("attempt 3 failed")
    );
}

#[test]
fn unknown_failure_has_no_guessed_recipe() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(observation(
        1,
        RecoveryState::Blocked,
        Some(RecoveryFailureCode::UnknownFailure),
        vec![action("show-manual-details", RecoveryActionKind::Manual)],
    ));

    let record = &engine.snapshot().records[0];
    assert_eq!(record.code, Some(RecoveryFailureCode::UnknownFailure));
    assert!(record.actions.iter().all(|candidate| {
        candidate.kind == RecoveryActionKind::Manual && candidate.recipe_id.is_none()
    }));
}
