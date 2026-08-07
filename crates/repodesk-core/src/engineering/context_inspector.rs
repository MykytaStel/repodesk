//! IDE-facing read model for inspecting the active Work Item context.
//!
//! This module composes existing deterministic evidence. It does not calculate
//! frontend-only metrics and it does not require raw prompt/response storage.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engineering::context_compactness::{
    ContextCompactnessReport, load_context_compactness,
};
use crate::engineering::context_manifest::{
    ContextFileEvidenceReport, ContextManifest, load_context_file_evidence, read_context_manifest,
};
use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContextInspectorReport {
    pub manifest: Option<ContextManifest>,
    pub compactness: ContextCompactnessReport,
    pub file_evidence: ContextFileEvidenceReport,
}

pub fn load_context_inspector(run_dir: &Path) -> RepoDeskResult<ContextInspectorReport> {
    Ok(ContextInspectorReport {
        manifest: read_context_manifest(run_dir)?,
        compactness: load_context_compactness(run_dir)?,
        file_evidence: load_context_file_evidence(run_dir)?,
    })
}
