//! Deterministic token-budget packing for context candidates.
//!
//! The packer never reads source content and never calls an AI provider. It
//! consumes structural `ContextCandidate` metadata and returns one explicit
//! selection decision per candidate so later rendering and UI inspection can
//! explain exactly why material was retained or omitted.

use std::cmp::Ordering;

use crate::context_pipeline::{
    ContextCandidate, ContextExclusionReason, ContextSelection, ContextSelectionState, ContextTrust,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextPackingPolicy {
    pub token_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPackingResult {
    pub token_budget: usize,
    pub included_tokens: usize,
    pub excluded_tokens: usize,
    pub selections: Vec<ContextSelection>,
}

/// Pack candidates into a deterministic token budget.
///
/// Inclusion priority is intentionally stable and explainable:
/// 1. required material
/// 2. relevance score
/// 3. provenance trust
/// 4. freshness score
/// 5. smaller candidate cost (fits more useful evidence when otherwise tied)
/// 6. candidate id as a final stable tie-break
///
/// The packer selects whole candidates. Source-specific safe trimming happens
/// before this layer; this layer never invents partial content without owning the
/// corresponding renderer. Included `order` follows the original candidate
/// order, which is also the render order of the caller.
pub fn pack_context_candidates(
    candidates: &[ContextCandidate],
    policy: ContextPackingPolicy,
) -> ContextPackingResult {
    let mut ranked = candidates.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| compare_candidates(left, right));

    let mut remaining = policy.token_budget;
    let mut included_tokens = 0usize;
    let mut excluded_tokens = 0usize;
    let mut selections = Vec::with_capacity(candidates.len());

    for candidate in ranked {
        if candidate.candidate_tokens <= remaining {
            remaining = remaining.saturating_sub(candidate.candidate_tokens);
            included_tokens = included_tokens.saturating_add(candidate.candidate_tokens);
            selections.push(ContextSelection {
                candidate_id: candidate.id.clone(),
                state: ContextSelectionState::Included,
                included_tokens: candidate.candidate_tokens,
                trimmed: false,
                exclusion_reason: None,
                order: None,
            });
        } else {
            excluded_tokens = excluded_tokens.saturating_add(candidate.candidate_tokens);
            selections.push(ContextSelection {
                candidate_id: candidate.id.clone(),
                state: ContextSelectionState::Excluded,
                included_tokens: 0,
                trimmed: false,
                exclusion_reason: Some(ContextExclusionReason::Budget),
                order: None,
            });
        }
    }

    selections.sort_by_key(|selection| {
        candidates
            .iter()
            .position(|candidate| candidate.id == selection.candidate_id)
            .unwrap_or(usize::MAX)
    });

    let mut render_order = 0usize;
    for selection in &mut selections {
        if selection.state == ContextSelectionState::Included {
            selection.order = Some(render_order);
            render_order += 1;
        }
    }

    ContextPackingResult {
        token_budget: policy.token_budget,
        included_tokens,
        excluded_tokens,
        selections,
    }
}

fn compare_candidates(left: &ContextCandidate, right: &ContextCandidate) -> Ordering {
    right
        .required
        .cmp(&left.required)
        .then_with(|| compare_score(right.relevance_score, left.relevance_score))
        .then_with(|| trust_rank(right.trust).cmp(&trust_rank(left.trust)))
        .then_with(|| compare_score(right.freshness_score, left.freshness_score))
        .then_with(|| left.candidate_tokens.cmp(&right.candidate_tokens))
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_score(left: Option<f32>, right: Option<f32>) -> Ordering {
    left.unwrap_or(0.0)
        .partial_cmp(&right.unwrap_or(0.0))
        .unwrap_or(Ordering::Equal)
}

fn trust_rank(trust: ContextTrust) -> u8 {
    match trust {
        ContextTrust::Authoritative => 5,
        ContextTrust::Reviewed => 4,
        ContextTrust::Observed => 3,
        ContextTrust::Heuristic => 2,
        ContextTrust::Legacy => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_pipeline::{ContextProvenance, ContextSourceKind};

    fn candidate(
        id: &str,
        tokens: usize,
        relevance: f32,
        trust: ContextTrust,
        required: bool,
    ) -> ContextCandidate {
        ContextCandidate {
            id: id.into(),
            provenance: ContextProvenance {
                kind: ContextSourceKind::Other,
                locator: id.into(),
                fingerprint: format!("sha256:{id}"),
                observed_at: None,
            },
            trust,
            candidate_tokens: tokens,
            required,
            relevance_score: Some(relevance),
            freshness_score: None,
        }
    }

    fn selection<'a>(result: &'a ContextPackingResult, id: &str) -> &'a ContextSelection {
        result
            .selections
            .iter()
            .find(|selection| selection.candidate_id == id)
            .unwrap()
    }

    #[test]
    fn required_material_wins_before_higher_scored_optional_material() {
        let candidates = vec![
            candidate("required", 80, 0.4, ContextTrust::Observed, true),
            candidate("optional", 80, 1.0, ContextTrust::Authoritative, false),
        ];

        let result =
            pack_context_candidates(&candidates, ContextPackingPolicy { token_budget: 80 });

        assert_eq!(
            selection(&result, "required").state,
            ContextSelectionState::Included
        );
        assert_eq!(
            selection(&result, "optional").state,
            ContextSelectionState::Excluded
        );
    }

    #[test]
    fn relevance_breaks_non_required_ties_before_trust() {
        let candidates = vec![
            candidate("reviewed", 60, 0.6, ContextTrust::Reviewed, false),
            candidate("relevant", 60, 0.9, ContextTrust::Observed, false),
        ];

        let result =
            pack_context_candidates(&candidates, ContextPackingPolicy { token_budget: 60 });

        assert_eq!(
            selection(&result, "relevant").state,
            ContextSelectionState::Included
        );
    }

    #[test]
    fn smaller_candidate_wins_when_all_semantic_signals_are_equal() {
        let candidates = vec![
            candidate("large", 90, 0.7, ContextTrust::Observed, false),
            candidate("small", 40, 0.7, ContextTrust::Observed, false),
        ];

        let result =
            pack_context_candidates(&candidates, ContextPackingPolicy { token_budget: 60 });

        assert_eq!(
            selection(&result, "small").state,
            ContextSelectionState::Included
        );
        assert_eq!(result.included_tokens, 40);
        assert_eq!(result.excluded_tokens, 90);
    }

    #[test]
    fn every_candidate_gets_an_explicit_stable_decision() {
        let candidates = vec![
            candidate("b", 50, 0.8, ContextTrust::Observed, false),
            candidate("a", 50, 0.9, ContextTrust::Observed, false),
        ];

        let result =
            pack_context_candidates(&candidates, ContextPackingPolicy { token_budget: 50 });

        assert_eq!(result.selections.len(), 2);
        assert_eq!(result.selections[0].candidate_id, "b");
        assert_eq!(result.selections[1].candidate_id, "a");
        assert_eq!(selection(&result, "a").order, Some(0));
        assert_eq!(
            selection(&result, "b").exclusion_reason,
            Some(ContextExclusionReason::Budget)
        );
    }

    #[test]
    fn included_order_matches_candidate_render_order_not_priority_order() {
        let candidates = vec![
            candidate("first", 20, 0.5, ContextTrust::Observed, false),
            candidate("second", 20, 1.0, ContextTrust::Authoritative, false),
        ];

        let result =
            pack_context_candidates(&candidates, ContextPackingPolicy { token_budget: 40 });

        assert_eq!(selection(&result, "first").order, Some(0));
        assert_eq!(selection(&result, "second").order, Some(1));
    }
}
