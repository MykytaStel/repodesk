//! IDE-facing read model for inspecting the active Work Item context.
//!
//! This module composes existing deterministic evidence. It does not calculate
//! frontend-only metrics and it does not require raw prompt/response storage.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engineering::context_compactness::{
    ContextCompactnessReport, derive_context_compactness,
};
use crate::engineering::context_manifest::{
    ContextFileEvidenceReport, ContextManifest, derive_context_file_evidence, read_context_manifest,
};
use crate::engineering::events::{EngineeringEvent, read_events};
use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContextInspectorReport {
    pub manifest: Option<ContextManifest>,
    pub compactness: ContextCompactnessReport,
    pub file_evidence: ContextFileEvidenceReport,
}

pub fn derive_context_inspector(
    events: &[EngineeringEvent],
    manifest: Option<ContextManifest>,
) -> ContextInspectorReport {
    ContextInspectorReport {
        manifest,
        compactness: derive_context_compactness(events),
        file_evidence: derive_context_file_evidence(events),
    }
}

pub fn load_context_inspector(run_dir: &Path) -> RepoDeskResult<ContextInspectorReport> {
    let events = read_events(run_dir)?;
    let manifest = read_context_manifest(run_dir)?;
    Ok(derive_context_inspector(&events, manifest))
}
