//! Deterministic relevance scoring for context candidates.
//!
//! This layer deliberately does not read repository file contents and does not
//! call an embedding/LLM provider. It scores structural context metadata so the
//! same task and provenance inputs always produce the same result. Later budget
//! selection can therefore explain and reproduce why a candidate was preferred.

use std::collections::BTreeSet;

use crate::context_pipeline::{ContextCandidate, ContextSourceKind, ContextTrust};

#[derive(Debug, Clone, PartialEq)]
pub struct ContextRelevanceAssessment {
    pub score: f32,
    pub source_prior: f32,
    pub trust_adjustment: f32,
    pub required_adjustment: f32,
    pub changed_file_adjustment: f32,
    pub lexical_adjustment: f32,
}

/// Score one context candidate using only structural evidence.
///
/// The score is descriptive selection evidence, not a security decision. Scope,
/// secret scanning and policy gates still win even when relevance is `1.0`.
pub fn score_context_relevance(
    candidate: &ContextCandidate,
    task_text: &str,
    changed_files: &[String],
) -> ContextRelevanceAssessment {
    let source_prior = source_prior(candidate.provenance.kind);
    let trust_adjustment = trust_adjustment(candidate.trust);
    let required_adjustment = if candidate.required { 0.10 } else { 0.0 };
    let changed_file_adjustment = if candidate.provenance.kind == ContextSourceKind::ScopedFile
        && changed_files
            .iter()
            .any(|path| same_repo_path(path, &candidate.provenance.locator))
    {
        0.15
    } else {
        0.0
    };
    let lexical_adjustment = 0.15 * lexical_affinity(task_text, &candidate.provenance.locator);

    let score = rounded_score(
        source_prior
            + trust_adjustment
            + required_adjustment
            + changed_file_adjustment
            + lexical_adjustment,
    );

    ContextRelevanceAssessment {
        score,
        source_prior,
        trust_adjustment,
        required_adjustment,
        changed_file_adjustment,
        lexical_adjustment: rounded_score(lexical_adjustment),
    }
}

/// Populate the candidate's persisted normalized score while returning the full
/// ephemeral breakdown for diagnostics/observability.
pub fn apply_context_relevance(
    candidate: &mut ContextCandidate,
    task_text: &str,
    changed_files: &[String],
) -> ContextRelevanceAssessment {
    let assessment = score_context_relevance(candidate, task_text, changed_files);
    candidate.relevance_score = Some(assessment.score);
    assessment
}

fn source_prior(kind: ContextSourceKind) -> f32 {
    match kind {
        ContextSourceKind::TaskMetadata
        | ContextSourceKind::TaskDocument
        | ContextSourceKind::WorkItemContract => 0.75,
        ContextSourceKind::AgentRules => 0.70,
        ContextSourceKind::Checks => 0.65,
        ContextSourceKind::GitState => 0.60,
        ContextSourceKind::EngineeringKnowledge | ContextSourceKind::ScopedFile => 0.55,
        ContextSourceKind::ProjectMetadata | ContextSourceKind::RiskLog => 0.50,
        ContextSourceKind::DecisionLog => 0.48,
        ContextSourceKind::RepositoryMap => 0.42,
        ContextSourceKind::SemanticSearch => 0.40,
        ContextSourceKind::LegacyMemory => 0.30,
        ContextSourceKind::Other => 0.25,
    }
}

fn trust_adjustment(trust: ContextTrust) -> f32 {
    match trust {
        ContextTrust::Authoritative => 0.15,
        ContextTrust::Reviewed => 0.12,
        ContextTrust::Observed => 0.08,
        ContextTrust::Heuristic => 0.02,
        ContextTrust::Legacy => -0.08,
    }
}

fn lexical_affinity(task_text: &str, locator: &str) -> f32 {
    let task_terms = tokenize(task_text);
    let locator_terms = tokenize(locator);
    if task_terms.is_empty() || locator_terms.is_empty() {
        return 0.0;
    }

    let matches = locator_terms.intersection(&task_terms).count();
    matches as f32 / locator_terms.len() as f32
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() > 1 && !is_noise_token(term))
        .collect()
}

fn is_noise_token(value: &str) -> bool {
    matches!(
        value,
        "src"
            | "lib"
            | "mod"
            | "main"
            | "index"
            | "test"
            | "tests"
            | "spec"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "md"
    )
}

fn same_repo_path(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn rounded_score(value: f32) -> f32 {
    (value.clamp(0.0, 1.0) * 1_000.0).round() / 1_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_pipeline::{ContextProvenance, ContextSourceKind, ContextTrust};

    fn candidate(kind: ContextSourceKind, trust: ContextTrust, locator: &str) -> ContextCandidate {
        ContextCandidate {
            id: "candidate".into(),
            provenance: ContextProvenance {
                kind,
                locator: locator.into(),
                fingerprint: "sha256:abc".into(),
                observed_at: None,
            },
            trust,
            candidate_tokens: 100,
            required: false,
            relevance_score: None,
            freshness_score: None,
        }
    }

    #[test]
    fn authoritative_task_material_outranks_legacy_memory() {
        let task = candidate(
            ContextSourceKind::WorkItemContract,
            ContextTrust::Authoritative,
            "work-item-contract.json",
        );
        let legacy = candidate(
            ContextSourceKind::LegacyMemory,
            ContextTrust::Legacy,
            "memory.md",
        );

        let task_score = score_context_relevance(&task, "Fix auth redirect", &[]).score;
        let legacy_score = score_context_relevance(&legacy, "Fix auth redirect", &[]).score;
        assert!(task_score > legacy_score);
    }

    #[test]
    fn changed_scoped_file_receives_structural_boost() {
        let file = candidate(
            ContextSourceKind::ScopedFile,
            ContextTrust::Observed,
            "src/auth/session.rs",
        );
        let unchanged = score_context_relevance(&file, "Fix session refresh", &[]);
        let changed = score_context_relevance(
            &file,
            "Fix session refresh",
            &["src/auth/session.rs".into()],
        );

        assert_eq!(changed.changed_file_adjustment, 0.15);
        assert!(changed.score > unchanged.score);
    }

    #[test]
    fn task_terms_matching_locator_raise_relevance() {
        let auth = candidate(
            ContextSourceKind::ScopedFile,
            ContextTrust::Observed,
            "src/auth/session.rs",
        );
        let billing = candidate(
            ContextSourceKind::ScopedFile,
            ContextTrust::Observed,
            "src/billing/invoice.rs",
        );

        let task = "repair auth session refresh";
        assert!(
            score_context_relevance(&auth, task, &[]).score
                > score_context_relevance(&billing, task, &[]).score
        );
    }

    #[test]
    fn relevance_is_normalized_and_apply_only_sets_score() {
        let mut value = candidate(
            ContextSourceKind::WorkItemContract,
            ContextTrust::Authoritative,
            "auth/work-item-contract.json",
        );
        value.required = true;
        let provenance = value.provenance.clone();

        let assessment = apply_context_relevance(&mut value, "auth contract", &[]);
        assert!((0.0..=1.0).contains(&assessment.score));
        assert_eq!(value.relevance_score, Some(assessment.score));
        assert_eq!(value.provenance, provenance);
    }

    #[test]
    fn path_matching_is_separator_and_case_stable() {
        let file = candidate(
            ContextSourceKind::ScopedFile,
            ContextTrust::Observed,
            "src/auth/session.rs",
        );
        let assessment =
            score_context_relevance(&file, "session", &[".\\SRC\\AUTH\\SESSION.RS".into()]);
        assert_eq!(assessment.changed_file_adjustment, 0.15);
    }
}
