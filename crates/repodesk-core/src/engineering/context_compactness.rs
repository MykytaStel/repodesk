//! Context-build evidence and deterministic compactness metrics.
//!
//! `candidate_tokens` describes the context pack assembled by the current
//! builder before its local trimming rules are applied. It is deliberately not
//! presented as the size of the whole repository or as proof of relevance.
//! Component fingerprints are persisted instead of raw component text so reuse
//! can be measured without copying prompt/context payloads into the event ledger.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engineering::context_manifest::{ContextFileStatus, ContextManifest};
use crate::engineering::domain::{EvidenceKind, EvidenceRef, WorkItemId};
use crate::engineering::events::{
    EngineeringEvent, EngineeringEventKind, append_event, read_events,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::tasks::TaskConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextComponentTelemetry {
    pub name: String,
    pub candidate_tokens: usize,
    pub included_tokens: usize,
    pub trimmed: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextBuildTelemetry<'a> {
    pub context_file: &'a str,
    pub token_estimate_file: &'a str,
    pub manifest_file: Option<&'a str>,
    pub manifest: Option<&'a ContextManifest>,
    pub included_tokens: usize,
    pub candidate_tokens: usize,
    pub context_fingerprint: &'a str,
    pub components: &'a [ContextComponentTelemetry],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContextCompactnessReport {
    /// All ContextBuilt events, including historical events without v0 metrics.
    pub builds: usize,
    /// ContextBuilt events carrying both candidate and included token counts.
    pub measured_builds: usize,
    pub total_candidate_tokens: usize,
    pub total_included_tokens: usize,
    pub total_compacted_tokens: usize,
    /// Metrics for the latest ContextBuilt event only. Historical events that
    /// predate compactness telemetry intentionally produce `None` here rather
    /// than leaking a stale previous measurement into the UI.
    pub latest: Option<ContextBuildCompactness>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBuildCompactness {
    pub candidate_tokens: usize,
    pub included_tokens: usize,
    pub compacted_tokens: usize,
    /// Included / candidate. This is descriptive, not a quality score.
    pub compactness_ratio: Option<f64>,
    /// Tokens in variable context components whose fingerprints are identical
    /// to the immediately preceding measured context build.
    pub repeated_tokens: Option<usize>,
    /// repeated_tokens / included_tokens. Static pack framing is deliberately
    /// excluded, making this a conservative reuse signal.
    pub repeated_context_ratio: Option<f64>,
    pub components: Vec<ContextComponentCompactness>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextComponentCompactness {
    pub name: String,
    pub candidate_tokens: usize,
    pub included_tokens: usize,
    pub trimmed: bool,
    pub reused_from_previous_build: bool,
}

/// Append one enriched ContextBuilt event. Recording remains best-effort at the
/// caller: context generation must not fail merely because telemetry cannot be
/// persisted.
pub fn record_context_build(
    task: &TaskConfig,
    telemetry: ContextBuildTelemetry<'_>,
) -> RepoDeskResult<()> {
    let work_item_id = WorkItemId::try_new(task.id.clone()).map_err(|error| {
        RepoDeskError::Api(format!("context compactness instrumentation: {error}"))
    })?;

    let mut event = EngineeringEvent::new(
        task.project_name.clone(),
        work_item_id,
        EngineeringEventKind::ContextBuilt,
    )
    .with_attribute("estimated_tokens", json!(telemetry.included_tokens))
    .with_attribute("candidate_tokens", json!(telemetry.candidate_tokens))
    .with_attribute(
        "context_fingerprint",
        Value::String(telemetry.context_fingerprint.to_string()),
    )
    .with_attribute("components", json!(telemetry.components));

    if let Some(manifest) = telemetry.manifest {
        let included_files = manifest
            .entries
            .iter()
            .filter(|entry| entry.status == ContextFileStatus::Included)
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        let excluded_files = manifest
            .entries
            .iter()
            .filter(|entry| entry.status == ContextFileStatus::Excluded)
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();

        event = event
            .with_attribute("context_manifest_version", json!(manifest.version))
            .with_attribute("included_file_count", json!(manifest.included_files))
            .with_attribute("excluded_file_count", json!(manifest.excluded_files))
            .with_attribute("included_file_tokens", json!(manifest.included_file_tokens))
            .with_attribute("included_files", json!(included_files))
            .with_attribute("excluded_files", json!(excluded_files));
    }

    if let Ok(evidence) =
        EvidenceRef::try_new(EvidenceKind::Context, telemetry.context_file.to_string())
    {
        event = event.with_evidence(evidence);
    }
    if let Ok(evidence) = EvidenceRef::try_new(
        EvidenceKind::Other,
        telemetry.token_estimate_file.to_string(),
    ) {
        event = event.with_evidence(evidence);
    }
    if let Some(manifest_file) = telemetry.manifest_file
        && let Ok(evidence) = EvidenceRef::try_new(EvidenceKind::Context, manifest_file.to_string())
    {
        event = event.with_evidence(evidence);
    }

    append_event(&task.run_dir, &event).map(|_| ())
}

pub fn load_context_compactness(run_dir: &Path) -> RepoDeskResult<ContextCompactnessReport> {
    let events = read_events(run_dir)?;
    Ok(derive_context_compactness(&events))
}

pub fn derive_context_compactness(events: &[EngineeringEvent]) -> ContextCompactnessReport {
    let mut report = ContextCompactnessReport::default();
    let mut previous_components: Option<BTreeMap<String, ContextComponentTelemetry>> = None;

    for event in events {
        if event.kind != EngineeringEventKind::ContextBuilt {
            continue;
        }

        report.builds += 1;
        report.latest = None;

        let Some(included_tokens) = attribute_usize(event, "estimated_tokens") else {
            previous_components = None;
            continue;
        };
        let Some(candidate_tokens) = attribute_usize(event, "candidate_tokens") else {
            previous_components = None;
            continue;
        };

        report.measured_builds += 1;
        report.total_candidate_tokens = report
            .total_candidate_tokens
            .saturating_add(candidate_tokens);
        report.total_included_tokens = report.total_included_tokens.saturating_add(included_tokens);

        let compacted_tokens = candidate_tokens.saturating_sub(included_tokens);
        report.total_compacted_tokens = report
            .total_compacted_tokens
            .saturating_add(compacted_tokens);

        let components = recorded_components(event);
        let (repeated_tokens, rendered_components, current_components) = match components {
            Some(components) => {
                let current = components
                    .iter()
                    .cloned()
                    .map(|component| (component.name.clone(), component))
                    .collect::<BTreeMap<_, _>>();

                let repeated = previous_components.as_ref().map(|previous| {
                    components
                        .iter()
                        .filter(|component| {
                            previous
                                .get(&component.name)
                                .is_some_and(|old| old.fingerprint == component.fingerprint)
                        })
                        .fold(0usize, |total, component| {
                            total.saturating_add(component.included_tokens)
                        })
                });

                let rendered = components
                    .into_iter()
                    .map(|component| ContextComponentCompactness {
                        reused_from_previous_build: previous_components
                            .as_ref()
                            .and_then(|previous| previous.get(&component.name))
                            .is_some_and(|old| old.fingerprint == component.fingerprint),
                        name: component.name,
                        candidate_tokens: component.candidate_tokens,
                        included_tokens: component.included_tokens,
                        trimmed: component.trimmed,
                    })
                    .collect();

                (repeated, rendered, Some(current))
            }
            None => (None, Vec::new(), None),
        };

        report.latest = Some(ContextBuildCompactness {
            candidate_tokens,
            included_tokens,
            compacted_tokens,
            compactness_ratio: ratio(included_tokens, candidate_tokens),
            repeated_tokens,
            repeated_context_ratio: repeated_tokens
                .and_then(|tokens| ratio(tokens, included_tokens)),
            components: rendered_components,
        });

        previous_components = current_components;
    }

    report
}

fn recorded_components(event: &EngineeringEvent) -> Option<Vec<ContextComponentTelemetry>> {
    event
        .attributes
        .get("components")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn attribute_usize(event: &EngineeringEvent, key: &str) -> Option<usize> {
    event
        .attributes
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::domain::WorkItemId;

    fn context_event(included: usize, candidate: usize, components: Value) -> EngineeringEvent {
        EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            EngineeringEventKind::ContextBuilt,
        )
        .with_attribute("estimated_tokens", json!(included))
        .with_attribute("candidate_tokens", json!(candidate))
        .with_attribute("components", components)
    }

    #[test]
    fn derives_compaction_and_conservative_component_reuse() {
        let first = context_event(
            1_000,
            1_500,
            json!([
                {
                    "name": "task",
                    "candidate_tokens": 400,
                    "included_tokens": 400,
                    "trimmed": false,
                    "fingerprint": "task-v1"
                },
                {
                    "name": "memory",
                    "candidate_tokens": 500,
                    "included_tokens": 300,
                    "trimmed": true,
                    "fingerprint": "memory-v1"
                }
            ]),
        );
        let second = context_event(
            800,
            1_200,
            json!([
                {
                    "name": "task",
                    "candidate_tokens": 400,
                    "included_tokens": 400,
                    "trimmed": false,
                    "fingerprint": "task-v1"
                },
                {
                    "name": "memory",
                    "candidate_tokens": 300,
                    "included_tokens": 200,
                    "trimmed": true,
                    "fingerprint": "memory-v2"
                }
            ]),
        );

        let report = derive_context_compactness(&[first, second]);

        assert_eq!(report.builds, 2);
        assert_eq!(report.measured_builds, 2);
        assert_eq!(report.total_candidate_tokens, 2_700);
        assert_eq!(report.total_included_tokens, 1_800);
        assert_eq!(report.total_compacted_tokens, 900);

        let latest = report.latest.unwrap();
        assert_eq!(latest.candidate_tokens, 1_200);
        assert_eq!(latest.included_tokens, 800);
        assert_eq!(latest.compacted_tokens, 400);
        assert_eq!(latest.compactness_ratio, Some(800.0 / 1_200.0));
        assert_eq!(latest.repeated_tokens, Some(400));
        assert_eq!(latest.repeated_context_ratio, Some(0.5));
        assert!(latest.components[0].reused_from_previous_build);
        assert!(!latest.components[1].reused_from_previous_build);
    }

    #[test]
    fn legacy_latest_build_does_not_reuse_stale_compactness() {
        let measured = context_event(800, 1_000, json!([]));
        let legacy = EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            EngineeringEventKind::ContextBuilt,
        )
        .with_attribute("estimated_tokens", json!(700));

        let report = derive_context_compactness(&[measured, legacy]);

        assert_eq!(report.builds, 2);
        assert_eq!(report.measured_builds, 1);
        assert_eq!(report.total_candidate_tokens, 1_000);
        assert_eq!(report.total_included_tokens, 800);
        assert_eq!(report.total_compacted_tokens, 200);
        assert_eq!(report.latest, None);
    }

    #[test]
    fn first_measured_build_has_unknown_repeated_ratio() {
        let report = derive_context_compactness(&[context_event(500, 750, json!([]))]);
        let latest = report.latest.unwrap();

        assert_eq!(latest.repeated_tokens, None);
        assert_eq!(latest.repeated_context_ratio, None);
    }
}
