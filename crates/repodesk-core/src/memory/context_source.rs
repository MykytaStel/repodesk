//! Selection contract for legacy Memory Brain material entering agent context.
//!
//! This module owns retrieval/fallback/provenance so the context builder does not
//! need a second interpretation of Memory Brain state.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::retrieval::{SliceRender, memory_slice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryContextSource {
    StructuredSlice,
    LegacyFile,
}

impl MemoryContextSource {
    pub fn locator(self) -> &'static str {
        match self {
            Self::StructuredSlice => "memory-brain:active-slice",
            Self::LegacyFile => "project:memory.md",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedMemoryContext {
    pub markdown: String,
    pub source: MemoryContextSource,
    pub observed_at: Option<DateTime<Utc>>,
}

/// Retrieve the bounded structured slice and resolve the only permitted legacy
/// fallback. Retrieval failure and pinned overflow propagate as hard errors.
pub fn resolve_context_memory(
    project: &str,
    token_budget: usize,
    legacy_path: &Path,
) -> RepoDeskResult<ResolvedMemoryContext> {
    let slice = memory_slice(project, token_budget)?;
    let source = context_source_for_slice(&slice)?;
    let (markdown, observed_at) = match source {
        MemoryContextSource::StructuredSlice => (
            slice.markdown.clone(),
            structured_observed_at(project, &slice.included_ids),
        ),
        MemoryContextSource::LegacyFile => (
            fs::read_to_string(legacy_path).unwrap_or_else(|_| "Not available.".to_string()),
            file_observed_at(legacy_path),
        ),
    };

    Ok(ResolvedMemoryContext {
        markdown,
        source,
        observed_at,
    })
}

/// Decide whether compatibility fallback is permitted. Retrieval errors are
/// already propagated by the caller; omitted pinned entries are a construction
/// failure, never ordinary truncation.
pub fn context_source_for_slice(slice: &SliceRender) -> RepoDeskResult<MemoryContextSource> {
    if !slice.pinned_overflow_ids.is_empty() {
        return Err(RepoDeskError::Api(format!(
            "context construction blocked: {} pinned project-memory record(s) do not fit the configured memory budget (ids: {})",
            slice.pinned_overflow_ids.len(),
            slice
                .pinned_overflow_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    if slice.total_active == 0 {
        Ok(MemoryContextSource::LegacyFile)
    } else {
        // Active structured records exist even if ordinary entries were excluded
        // by the hard budget. memory.md cannot bypass ranked selection.
        Ok(MemoryContextSource::StructuredSlice)
    }
}

fn structured_observed_at(project: &str, included_ids: &[i64]) -> Option<DateTime<Utc>> {
    if included_ids.is_empty() {
        return None;
    }
    let included = included_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    super::store::list_active(project)
        .ok()?
        .into_iter()
        .filter(|entry| included.contains(&entry.id))
        .map(|entry| entry.updated_at.unwrap_or(entry.timestamp))
        .min()
}

fn file_observed_at(path: &Path) -> Option<DateTime<Utc>> {
    Some(DateTime::<Utc>::from(
        fs::metadata(path).ok()?.modified().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(total_active: usize) -> SliceRender {
        SliceRender {
            markdown: String::new(),
            estimated_tokens: 0,
            included_ids: Vec::new(),
            excluded_ids: Vec::new(),
            pinned_overflow_ids: Vec::new(),
            total_active,
            budget_exhausted: false,
        }
    }

    #[test]
    fn legacy_fallback_is_only_allowed_when_structured_store_is_empty() {
        assert_eq!(
            context_source_for_slice(&slice(0)).unwrap(),
            MemoryContextSource::LegacyFile
        );
        assert_eq!(
            context_source_for_slice(&slice(3)).unwrap(),
            MemoryContextSource::StructuredSlice
        );
    }

    #[test]
    fn pinned_overflow_blocks_context_instead_of_falling_back() {
        let mut value = slice(2);
        value.pinned_overflow_ids = vec![7, 11];
        value.excluded_ids = vec![7, 11];
        value.budget_exhausted = true;

        let error = context_source_for_slice(&value).unwrap_err().to_string();
        assert!(error.contains("pinned project-memory"));
        assert!(error.contains("7, 11"));
    }
}
