use chrono::{DateTime, Duration, TimeZone, Utc};
use repodesk_core::engineering::domain::{VerificationId, WorkItemId};
use repodesk_core::engineering::events::{EngineeringEvent, EngineeringEventKind};
use repodesk_core::engineering::instrumentation::VerificationCheckTelemetry;
use repodesk_core::engineering::{VerificationHistoryConfidence, derive_verification_history};
use serde_json::json;

fn event(
    verification_id: &str,
    check_id: &str,
    status: &str,
    duration_ms: u64,
    occurred_at: DateTime<Utc>,
) -> EngineeringEvent {
    let finished_at = occurred_at + Duration::milliseconds(duration_ms as i64);
    let telemetry = VerificationCheckTelemetry {
        check_id: check_id.into(),
        command: format!("cargo test -- {check_id}"),
        status: status.into(),
        exit_code: (status == "passed").then_some(0),
        duration_ms,
        started_at: occurred_at,
        finished_at,
        tree_identity: Some(format!("tree-{duration_ms}")),
        log_evidence_ref: Some(format!("log-{duration_ms}")),
        tests_observed: None,
    };
    let mut event = EngineeringEvent::new(
        "repodesk",
        WorkItemId::try_new("work-123").unwrap(),
        EngineeringEventKind::VerificationFinished,
    )
    .with_verification(VerificationId::try_new(verification_id).unwrap())
    .with_attribute("success", json!(status == "passed"))
    .with_attribute("check_results", json!([telemetry]));
    event.occurred_at = occurred_at;
    event
}

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + seconds, 0)
        .single()
        .unwrap()
}

#[test]
fn history_uses_median_and_marks_three_samples_calibrated() {
    let history = derive_verification_history(&[
        event("verify-1", "unit-tests", "passed", 100, at(1)),
        event("verify-2", "unit-tests", "passed", 300, at(2)),
        event("verify-3", "unit-tests", "passed", 200, at(3)),
    ]);

    let check = &history.checks[0];
    assert_eq!(check.measured_runs, 3);
    assert_eq!(check.failed_runs, 0);
    assert_eq!(check.median_duration_ms, Some(200));
    assert_eq!(check.confidence, VerificationHistoryConfidence::Calibrated);
    assert_eq!(check.latest_status.as_deref(), Some("passed"));
    assert_eq!(check.latest_tree_identity.as_deref(), Some("tree-200"));
}

#[test]
fn history_is_provisional_for_one_or_two_usable_samples() {
    let one = derive_verification_history(&[event("verify-1", "unit-tests", "passed", 100, at(1))]);
    assert_eq!(
        one.checks[0].confidence,
        VerificationHistoryConfidence::Provisional
    );

    let two = derive_verification_history(&[
        event("verify-1", "unit-tests", "passed", 100, at(1)),
        event("verify-2", "unit-tests", "failed", 300, at(2)),
    ]);
    assert_eq!(two.checks[0].measured_runs, 2);
    assert_eq!(two.checks[0].failed_runs, 1);
    assert_eq!(two.checks[0].median_duration_ms, Some(200));
    assert_eq!(
        two.checks[0].confidence,
        VerificationHistoryConfidence::Provisional
    );
}

#[test]
fn timeout_is_counted_but_excluded_from_duration_estimate() {
    let history = derive_verification_history(&[
        event("verify-1", "unit-tests", "passed", 100, at(1)),
        event("verify-2", "unit-tests", "timeout", 120_000, at(2)),
        event("verify-3", "unit-tests", "failed", 300, at(3)),
    ]);

    let check = &history.checks[0];
    assert_eq!(check.measured_runs, 3);
    assert_eq!(check.failed_runs, 1);
    assert_eq!(check.median_duration_ms, Some(200));
    assert_eq!(check.latest_status.as_deref(), Some("timeout"));
}

#[test]
fn duplicate_verification_check_records_do_not_inflate_history() {
    let history = derive_verification_history(&[
        event("verify-1", "unit-tests", "passed", 100, at(1)),
        event("verify-1", "unit-tests", "failed", 900, at(2)),
        event("verify-2", "unit-tests", "passed", 300, at(3)),
    ]);

    let check = &history.checks[0];
    assert_eq!(check.measured_runs, 2);
    assert_eq!(check.failed_runs, 1);
    assert_eq!(check.median_duration_ms, Some(600));
    assert_eq!(check.latest_status.as_deref(), Some("passed"));
    assert_eq!(check.latest_at, Some(at(3) + Duration::milliseconds(300)));
}

#[test]
fn aggregate_only_events_remain_readable_without_creating_fake_check_history() {
    let event = EngineeringEvent::new(
        "repodesk",
        WorkItemId::try_new("work-123").unwrap(),
        EngineeringEventKind::VerificationFinished,
    )
    .with_verification(VerificationId::try_new("verify-old").unwrap())
    .with_attribute("success", json!(true))
    .with_attribute("command_count", json!(2));

    let history = derive_verification_history(&[event]);
    assert!(history.checks.is_empty());
}
