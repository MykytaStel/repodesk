//! IDE-facing read model for inspecting the active Work Item context.
//!
//! This module composes existing deterministic evidence. It does not calculate
//! frontend-only metrics and it does not require raw prompt/response storage.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context_pipeline::ContextPipelineSnapshot;
use crate::engineering::context_compactness::{
    ContextCompactnessReport, derive_context_compactness,
};
use crate::engineering::context_manifest::{
    ContextFileEvidenceReport, ContextManifest, derive_context_file_evidence, read_context_manifest,
};
use crate::engineering::events::{EngineeringEvent, read_events};
use crate::errors::RepoDeskResult;

const CONTEXT_PIPELINE_FILE: &str = "context-pipeline.json";
const MAX_CONTEXT_PIPELINE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContextInspectorReport {
    pub manifest: Option<ContextManifest>,
    pub compactness: ContextCompactnessReport,
    pub file_evidence: ContextFileEvidenceReport,
    /// Structural source/ranking/packing evidence for the latest context build.
    /// Raw context text is intentionally not present in this snapshot.
    pub pipeline: Option<ContextPipelineSnapshot>,
    /// Pipeline evidence is recoverable by rebuilding context, so corruption is
    /// surfaced to the Inspector without making the whole Work surface fail.
    pub pipeline_error: Option<String>,
}

pub fn derive_context_inspector(
    events: &[EngineeringEvent],
    manifest: Option<ContextManifest>,
) -> ContextInspectorReport {
    ContextInspectorReport {
        manifest,
        compactness: derive_context_compactness(events),
        file_evidence: derive_context_file_evidence(events),
        pipeline: None,
        pipeline_error: None,
    }
}

pub fn load_context_inspector(run_dir: &Path) -> RepoDeskResult<ContextInspectorReport> {
    let events = read_events(run_dir)?;
    let manifest = read_context_manifest(run_dir)?;
    let mut report = derive_context_inspector(&events, manifest);

    match read_context_pipeline(run_dir) {
        Ok(pipeline) => report.pipeline = pipeline,
        Err(error) => report.pipeline_error = Some(error.to_string()),
    }

    Ok(report)
}

fn read_context_pipeline(run_dir: &Path) -> RepoDeskResult<Option<ContextPipelineSnapshot>> {
    let path = run_dir.join(CONTEXT_PIPELINE_FILE);
    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(crate::errors::RepoDeskError::Api(
            "Context pipeline evidence is not a regular file".into(),
        ));
    }
    if metadata.len() > MAX_CONTEXT_PIPELINE_BYTES {
        return Err(crate::errors::RepoDeskError::Api(format!(
            "Context pipeline evidence exceeds the {} byte limit",
            MAX_CONTEXT_PIPELINE_BYTES
        )));
    }

    let content = fs::read_to_string(path)?;
    let snapshot: ContextPipelineSnapshot = serde_json::from_str(&content)?;
    snapshot
        .validate()
        .map_err(|error| crate::errors::RepoDeskError::Api(error.to_string()))?;
    Ok(Some(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_pipeline::{
        ContextCandidate, ContextProvenance, ContextSelection, ContextSelectionState,
        ContextSourceKind, ContextTrust,
    };
    use tempfile::tempdir;

    #[test]
    fn missing_pipeline_is_a_valid_prebuild_state() {
        let dir = tempdir().unwrap();
        assert_eq!(read_context_pipeline(dir.path()).unwrap(), None);
    }

    #[test]
    fn damaged_pipeline_is_reported_without_becoming_absence() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(CONTEXT_PIPELINE_FILE), "{ definitely not json").unwrap();
        assert!(read_context_pipeline(dir.path()).is_err());
    }

    #[test]
    fn valid_pipeline_round_trips_as_structural_evidence() {
        let dir = tempdir().unwrap();
        let candidate = ContextCandidate {
            id: "task".into(),
            provenance: ContextProvenance {
                kind: ContextSourceKind::TaskDocument,
                locator: "task.md".into(),
                fingerprint: "sha256:task".into(),
                observed_at: None,
            },
            trust: ContextTrust::Authoritative,
            candidate_tokens: 20,
            required: true,
            relevance_score: Some(1.0),
            freshness_score: None,
        };
        let snapshot = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            Some(100),
            vec![candidate],
            vec![ContextSelection {
                candidate_id: "task".into(),
                state: ContextSelectionState::Included,
                included_tokens: 20,
                trimmed: false,
                exclusion_reason: None,
                order: Some(0),
            }],
        )
        .unwrap();
        fs::write(
            dir.path().join(CONTEXT_PIPELINE_FILE),
            serde_json::to_string(&snapshot).unwrap(),
        )
        .unwrap();

        assert_eq!(read_context_pipeline(dir.path()).unwrap(), Some(snapshot));
    }
}
