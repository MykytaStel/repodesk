//! Selection contract for legacy Memory Brain material entering an agent context.
//!
//! The important distinction is between "there are no structured records" and
//! "structured retrieval failed/could not fit required pinned records". Only the
//! former may use the historical `memory.md` compatibility file.

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::retrieval::SliceRender;

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

/// Validate the retrieval result and decide whether compatibility fallback is
/// permitted. Retrieval errors are propagated by the caller before this point;
/// omitted pinned entries are a hard construction failure, not truncation.
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
        // Even if ordinary records were excluded by the hard token budget, a
        // structured store exists. Falling back to memory.md here would bypass
        // the ranked/bounded selection policy and reintroduce unreviewed bytes.
        Ok(MemoryContextSource::StructuredSlice)
    }
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
