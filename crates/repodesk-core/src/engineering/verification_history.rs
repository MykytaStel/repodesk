use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::events::{EngineeringEvent, EngineeringEventKind};
use super::instrumentation::VerificationCheckTelemetry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationHistoryConfidence {
    Unknown,
    Provisional,
    Calibrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheckHistory {
    pub check_id: String,
    pub measured_runs: usize,
    pub failed_runs: usize,
    pub latest_status: Option<String>,
    pub latest_at: Option<DateTime<Utc>>,
    pub latest_tree_identity: Option<String>,
    pub median_duration_ms: Option<u64>,
    pub confidence: VerificationHistoryConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VerificationHistory {
    pub checks: Vec<VerificationCheckHistory>,
}

#[derive(Debug, Clone)]
struct StoredCheckResult {
    event_occurred_at: DateTime<Utc>,
    event_id: String,
    result: VerificationCheckTelemetry,
}

/// Rebuild per-check execution history from the append-only engineering events.
///
/// Only bounded `check_results` records are considered. Aggregate-only legacy
/// events remain useful to aggregate verification consumers, but cannot create
/// a per-check record without a stable check identity.
pub fn derive_verification_history(events: &[EngineeringEvent]) -> VerificationHistory {
    let mut latest_by_identity = BTreeMap::<(String, String), StoredCheckResult>::new();

    for event in events
        .iter()
        .filter(|event| event.kind == EngineeringEventKind::VerificationFinished)
    {
        let Some(Value::Array(results)) = event.attributes.get("check_results") else {
            continue;
        };

        let verification_key = event
            .verification_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| event.id.to_string());

        for value in results {
            let Ok(result) = serde_json::from_value::<VerificationCheckTelemetry>(value.clone())
            else {
                continue;
            };
            if result.check_id.trim().is_empty() {
                continue;
            }

            let identity = (verification_key.clone(), result.check_id.clone());
            let candidate = StoredCheckResult {
                event_occurred_at: event.occurred_at,
                event_id: event.id.to_string(),
                result,
            };
            let should_replace = latest_by_identity
                .get(&identity)
                .is_none_or(|current| is_newer(&candidate, current));
            if should_replace {
                latest_by_identity.insert(identity, candidate);
            }
        }
    }

    let mut grouped = BTreeMap::<String, Vec<StoredCheckResult>>::new();
    for stored in latest_by_identity.into_values() {
        grouped
            .entry(stored.result.check_id.clone())
            .or_default()
            .push(stored);
    }

    VerificationHistory {
        checks: grouped
            .into_iter()
            .map(|(check_id, records)| history_for_check(check_id, records))
            .collect(),
    }
}

fn is_newer(candidate: &StoredCheckResult, current: &StoredCheckResult) -> bool {
    (
        candidate.event_occurred_at,
        candidate.event_id.as_str(),
        candidate.result.finished_at,
    ) > (
        current.event_occurred_at,
        current.event_id.as_str(),
        current.result.finished_at,
    )
}

fn history_for_check(
    check_id: String,
    records: Vec<StoredCheckResult>,
) -> VerificationCheckHistory {
    let measured_runs = records.len();
    let failed_runs = records
        .iter()
        .filter(|record| record.result.status.eq_ignore_ascii_case("failed"))
        .count();
    let latest = records.iter().max_by_key(|record| {
        (
            record.result.finished_at,
            record.event_occurred_at,
            record.event_id.as_str(),
        )
    });
    let mut usable_durations = records
        .iter()
        .filter(|record| {
            record.result.status.eq_ignore_ascii_case("passed")
                || record.result.status.eq_ignore_ascii_case("failed")
        })
        .map(|record| record.result.duration_ms)
        .collect::<Vec<_>>();
    usable_durations.sort_unstable();

    VerificationCheckHistory {
        check_id,
        measured_runs,
        failed_runs,
        latest_status: latest.map(|record| record.result.status.clone()),
        latest_at: latest.map(|record| record.result.finished_at),
        latest_tree_identity: latest.and_then(|record| record.result.tree_identity.clone()),
        median_duration_ms: median(&usable_durations),
        confidence: confidence_for(usable_durations.len()),
    }
}

fn median(values: &[u64]) -> Option<u64> {
    match values.len() {
        0 => None,
        length if length % 2 == 1 => Some(values[length / 2]),
        length => {
            let left = values[length / 2 - 1] as u128;
            let right = values[length / 2] as u128;
            Some(((left + right) / 2) as u64)
        }
    }
}

fn confidence_for(usable_samples: usize) -> VerificationHistoryConfidence {
    match usable_samples {
        0 => VerificationHistoryConfidence::Unknown,
        1..=2 => VerificationHistoryConfidence::Provisional,
        _ => VerificationHistoryConfidence::Calibrated,
    }
}
