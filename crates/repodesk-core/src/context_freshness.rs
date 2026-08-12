//! Deterministic freshness scoring for Context Pipeline candidates.
//!
//! Freshness describes how old the *observed evidence* is, not whether old code
//! or an old architectural decision is automatically bad. Different source
//! classes therefore decay at different rates. The curve intentionally matches
//! the existing Memory Brain recency shape:
//!
//! `score = half_life / (half_life + age_days)`
//!
//! At one half-life the score is `0.5`. Missing timestamps stay unevaluated
//! (`None`) rather than being treated as stale or fresh.

use chrono::{DateTime, Utc};

use crate::context_pipeline::{ContextCandidate, ContextSourceKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextFreshnessPolicy {
    pub half_life_days: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextFreshnessAssessment {
    pub score: Option<f32>,
    pub observed_at: Option<DateTime<Utc>>,
    pub age_days: Option<f64>,
    pub half_life_days: f64,
}

/// Source-specific decay policy.
///
/// Volatile runtime evidence decays quickly. Reviewed/durable engineering
/// knowledge decays slowly because age alone does not invalidate it.
pub fn context_freshness_policy(kind: ContextSourceKind) -> ContextFreshnessPolicy {
    let half_life_days = match kind {
        ContextSourceKind::GitState | ContextSourceKind::Checks => 1.0,
        ContextSourceKind::SemanticSearch => 3.0,
        ContextSourceKind::ScopedFile | ContextSourceKind::RepositoryMap => 7.0,
        ContextSourceKind::TaskMetadata
        | ContextSourceKind::TaskDocument
        | ContextSourceKind::WorkItemContract => 14.0,
        ContextSourceKind::ProjectMetadata | ContextSourceKind::AgentRules => 30.0,
        // Keep legacy memory aligned with the existing Memory Brain retrieval
        // recency half-life so the two layers do not disagree about age.
        ContextSourceKind::LegacyMemory => 30.0,
        ContextSourceKind::Other => 90.0,
        ContextSourceKind::EngineeringKnowledge => 180.0,
        ContextSourceKind::DecisionLog | ContextSourceKind::RiskLog => 365.0,
    };

    ContextFreshnessPolicy { half_life_days }
}

/// Score one observation relative to a supplied clock.
///
/// Passing `now` explicitly keeps tests and persisted evidence reproducible.
/// Future timestamps are treated as age zero so small clock skew cannot produce
/// scores above `1.0`.
pub fn score_context_freshness_at(
    kind: ContextSourceKind,
    observed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> ContextFreshnessAssessment {
    let policy = context_freshness_policy(kind);
    let Some(observed_at) = observed_at else {
        return ContextFreshnessAssessment {
            score: None,
            observed_at: None,
            age_days: None,
            half_life_days: policy.half_life_days,
        };
    };

    let age_seconds = (now - observed_at).num_seconds().max(0) as f64;
    let age_days = age_seconds / 86_400.0;
    let score = score_age_days(age_days, policy.half_life_days);

    ContextFreshnessAssessment {
        score: Some(score),
        observed_at: Some(observed_at),
        age_days: Some(age_days),
        half_life_days: policy.half_life_days,
    }
}

/// Convenience wrapper for live code paths.
pub fn score_context_freshness(
    kind: ContextSourceKind,
    observed_at: Option<DateTime<Utc>>,
) -> ContextFreshnessAssessment {
    score_context_freshness_at(kind, observed_at, Utc::now())
}

/// Apply freshness evidence to a candidate without touching relevance, trust,
/// token cost, required status, or provenance fingerprint.
pub fn apply_context_freshness_at(
    candidate: &mut ContextCandidate,
    observed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> ContextFreshnessAssessment {
    let assessment = score_context_freshness_at(candidate.provenance.kind, observed_at, now);
    candidate.provenance.observed_at = assessment.observed_at;
    candidate.freshness_score = assessment.score;
    assessment
}

pub fn apply_context_freshness(
    candidate: &mut ContextCandidate,
    observed_at: Option<DateTime<Utc>>,
) -> ContextFreshnessAssessment {
    apply_context_freshness_at(candidate, observed_at, Utc::now())
}

/// Shared decay primitive for adapters that aggregate several durable records
/// before they become one ContextCandidate (for example Memory Brain slices).
pub fn score_age_days(age_days: f64, half_life_days: f64) -> f32 {
    if !age_days.is_finite() || !half_life_days.is_finite() || half_life_days <= 0.0 {
        return 0.0;
    }

    let age_days = age_days.max(0.0);
    let score = half_life_days / (half_life_days + age_days);
    ((score.clamp(0.0, 1.0) * 1_000.0).round() / 1_000.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    use crate::context_pipeline::{ContextProvenance, ContextTrust};

    fn candidate(kind: ContextSourceKind) -> ContextCandidate {
        ContextCandidate {
            id: "candidate".into(),
            provenance: ContextProvenance {
                kind,
                locator: "source".into(),
                fingerprint: "sha256:source".into(),
                observed_at: None,
            },
            trust: ContextTrust::Observed,
            candidate_tokens: 100,
            required: false,
            relevance_score: Some(0.8),
            freshness_score: None,
        }
    }

    #[test]
    fn score_is_one_for_a_current_observation() {
        let now = Utc::now();
        let assessment = score_context_freshness_at(ContextSourceKind::GitState, Some(now), now);
        assert_eq!(assessment.score, Some(1.0));
        assert_eq!(assessment.age_days, Some(0.0));
    }

    #[test]
    fn score_is_half_at_one_source_half_life() {
        let now = Utc::now();
        let observed = now - Duration::days(30);
        let assessment =
            score_context_freshness_at(ContextSourceKind::LegacyMemory, Some(observed), now);
        assert_eq!(assessment.score, Some(0.5));
        assert_eq!(assessment.half_life_days, 30.0);
    }

    #[test]
    fn durable_knowledge_decays_slower_than_runtime_state() {
        let now = Utc::now();
        let observed = Some(now - Duration::days(7));
        let git = score_context_freshness_at(ContextSourceKind::GitState, observed, now);
        let knowledge =
            score_context_freshness_at(ContextSourceKind::EngineeringKnowledge, observed, now);

        assert!(knowledge.score.unwrap() > git.score.unwrap());
    }

    #[test]
    fn missing_timestamp_remains_unevaluated() {
        let assessment =
            score_context_freshness_at(ContextSourceKind::EngineeringKnowledge, None, Utc::now());
        assert_eq!(assessment.score, None);
        assert_eq!(assessment.age_days, None);
    }

    #[test]
    fn future_timestamp_is_clamped_to_current() {
        let now = Utc::now();
        let assessment = score_context_freshness_at(
            ContextSourceKind::TaskDocument,
            Some(now + Duration::minutes(10)),
            now,
        );
        assert_eq!(assessment.score, Some(1.0));
        assert_eq!(assessment.age_days, Some(0.0));
    }

    #[test]
    fn applying_freshness_preserves_other_candidate_signals() {
        let now = Utc::now();
        let mut value = candidate(ContextSourceKind::ScopedFile);
        let fingerprint = value.provenance.fingerprint.clone();
        let relevance = value.relevance_score;

        let assessment = apply_context_freshness_at(&mut value, Some(now), now);

        assert_eq!(value.freshness_score, assessment.score);
        assert_eq!(value.provenance.observed_at, Some(now));
        assert_eq!(value.provenance.fingerprint, fingerprint);
        assert_eq!(value.relevance_score, relevance);
    }

    #[test]
    fn memory_half_life_matches_existing_retrieval_policy() {
        assert_eq!(
            context_freshness_policy(ContextSourceKind::LegacyMemory).half_life_days,
            30.0
        );
    }
}
