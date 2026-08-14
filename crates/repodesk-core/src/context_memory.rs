use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::memory::retrieval::SliceRender;

/// Resolve the Memory Brain portion of a prepared context without creating a
/// second source of truth.
///
/// Invariants:
/// - omitted pinned memory is a hard preparation failure;
/// - legacy `memory.md` is compatibility-only and is read lazily when there are
///   zero active structured records;
/// - ordinary budget exclusions never reactivate the legacy file;
/// - the bounded slice keeps its selection/provenance metadata for the context
///   pipeline while the rendered markdown is moved out without cloning it.
pub(crate) fn prepare_memory_for_context<F>(
    mut slice: SliceRender,
    token_budget: usize,
    legacy_memory: F,
) -> RepoDeskResult<(Option<SliceRender>, String)>
where
    F: FnOnce() -> String,
{
    if !slice.pinned_overflow_ids.is_empty() {
        let ids = slice
            .pinned_overflow_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(RepoDeskError::Api(format!(
            "context preparation blocked: pinned project memory does not fit the {token_budget}-token memory budget (omitted ids: {ids})"
        )));
    }

    if slice.total_active == 0 {
        return Ok((None, legacy_memory()));
    }

    let memory = if slice.is_empty() {
        String::new()
    } else {
        std::mem::take(&mut slice.markdown)
    };

    Ok((Some(slice), memory))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::cell::Cell;

    fn memory_slice(
        total_active: usize,
        included_ids: Vec<i64>,
        pinned_overflow_ids: Vec<i64>,
        markdown: &str,
    ) -> SliceRender {
        SliceRender {
            estimated_tokens: crate::tokens::estimate_text(markdown).estimated_tokens,
            markdown: markdown.to_string(),
            included_ids,
            excluded_ids: Vec::new(),
            pinned_overflow_ids,
            total_active,
            budget_exhausted: false,
            observed_at: Some(Utc.with_ymd_and_hms(2026, 8, 14, 9, 0, 0).unwrap()),
        }
    }

    #[test]
    fn zero_structured_memory_is_the_only_legacy_fallback_case() {
        let fallback_called = Cell::new(false);
        let (slice, memory) = prepare_memory_for_context(
            memory_slice(0, Vec::new(), Vec::new(), "No project memory recorded yet."),
            800,
            || {
                fallback_called.set(true);
                "legacy memory.md".to_string()
            },
        )
        .unwrap();

        assert!(slice.is_none());
        assert_eq!(memory, "legacy memory.md");
        assert!(fallback_called.get());
    }

    #[test]
    fn active_structured_memory_never_falls_back_when_budget_excludes_it() {
        let fallback_called = Cell::new(false);
        let (slice, memory) = prepare_memory_for_context(
            memory_slice(3, Vec::new(), Vec::new(), "No project memory recorded yet."),
            800,
            || {
                fallback_called.set(true);
                "must not be used".to_string()
            },
        )
        .unwrap();

        assert!(slice.is_some());
        assert!(memory.is_empty());
        assert!(!fallback_called.get());
    }

    #[test]
    fn pinned_memory_overflow_blocks_prepare_before_legacy_fallback() {
        let fallback_called = Cell::new(false);
        let error = prepare_memory_for_context(
            memory_slice(2, vec![1], vec![2], "included memory"),
            800,
            || {
                fallback_called.set(true);
                "must not be used".to_string()
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("pinned project memory"));
        assert!(error.to_string().contains("800-token"));
        assert!(error.to_string().contains("2"));
        assert!(!fallback_called.get());
    }

    #[test]
    fn active_structured_memory_keeps_bounded_payload_and_provenance() {
        let observed_at = Utc.with_ymd_and_hms(2026, 8, 14, 9, 0, 0).unwrap();
        let (slice, memory) = prepare_memory_for_context(
            memory_slice(1, vec![7], Vec::new(), "structured memory"),
            800,
            || "legacy".to_string(),
        )
        .unwrap();

        assert_eq!(memory, "structured memory");
        assert_eq!(slice.unwrap().observed_at, Some(observed_at));
    }
}
