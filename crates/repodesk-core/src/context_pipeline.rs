//! Shared contracts for RepoDesk context selection.
//!
//! This module is intentionally **not** another context builder. `context.rs`,
//! `smart_context.rs`, and `agent_context_pack.rs` currently produce different
//! projections of task context. The types here define the common metadata model
//! those implementations will progressively converge on:
//!
//! `source/provenance -> candidate -> selection -> task context snapshot`.
//!
//! Persisted pipeline evidence must stay structural. Raw repository/source text
//! belongs in bounded context artifacts, never in this metadata contract.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const CONTEXT_PIPELINE_VERSION: u32 = 1;

/// Stable classes of material that may contribute to a task context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceKind {
    ProjectMetadata,
    TaskMetadata,
    TaskDocument,
    WorkItemContract,
    ScopedFile,
    EngineeringKnowledge,
    LegacyMemory,
    DecisionLog,
    RiskLog,
    Checks,
    GitState,
    RepositoryMap,
    SemanticSearch,
    AgentRules,
    Other,
}

/// How strongly RepoDesk may rely on a candidate when constructing context.
///
/// This is provenance/trust metadata, not a quality score and not permission to
/// bypass safety or scope checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTrust {
    /// Explicit project/task configuration or a typed Work Item Contract.
    Authoritative,
    /// Human-reviewed project engineering knowledge.
    Reviewed,
    /// Directly observed repository/git/task state.
    Observed,
    /// Derived or ranked material such as semantic search or repository hints.
    Heuristic,
    /// Compatibility material retained during migration.
    Legacy,
}

/// Structural origin of one candidate. `locator` identifies where the material
/// came from; `fingerprint` identifies the observed bytes without persisting them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextProvenance {
    pub kind: ContextSourceKind,
    pub locator: String,
    pub fingerprint: String,
    /// When RepoDesk observed the source, when that timestamp is meaningful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
}

/// A piece of context before budget/ranking decisions are applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCandidate {
    /// Stable within one snapshot. It is a reference key, not a database id.
    pub id: String,
    pub provenance: ContextProvenance,
    pub trust: ContextTrust,
    pub candidate_tokens: usize,
    /// Required candidates should be strongly preferred by selection. Security
    /// and policy may still exclude them; this flag is not an override.
    pub required: bool,
    /// Optional normalized ranking inputs. PRs after the contracts slice will
    /// populate these; absence means "not evaluated", never zero quality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_score: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSelectionState {
    Included,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextExclusionReason {
    Budget,
    Security,
    Missing,
    Unsupported,
    Stale,
    LowRelevance,
    Duplicate,
    Policy,
}

/// Final decision for one candidate. A complete snapshot contains exactly one
/// selection record for every candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSelection {
    pub candidate_id: String,
    pub state: ContextSelectionState,
    pub included_tokens: usize,
    pub trimmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusion_reason: Option<ContextExclusionReason>,
    /// Zero-based order in the rendered context when included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<usize>,
}

/// Durable structural description of one task-context build.
///
/// It intentionally stores counts, scores, locators, timestamps and hashes —
/// never raw source text. This is the contract that later provenance, budgeting,
/// memory and observability slices can share.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPipelineSnapshot {
    pub version: u32,
    pub project: String,
    pub task_id: String,
    pub generated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<usize>,
    pub candidate_tokens: usize,
    pub included_tokens: usize,
    pub context_fingerprint: String,
    pub candidates: Vec<ContextCandidate>,
    pub selections: Vec<ContextSelection>,
}

impl ContextPipelineSnapshot {
    pub fn new(
        project: impl Into<String>,
        task_id: impl Into<String>,
        context_fingerprint: impl Into<String>,
        token_budget: Option<usize>,
        candidates: Vec<ContextCandidate>,
        selections: Vec<ContextSelection>,
    ) -> Result<Self, ContextPipelineValidationError> {
        let candidate_tokens = candidates.iter().fold(0usize, |total, candidate| {
            total.saturating_add(candidate.candidate_tokens)
        });
        let included_tokens = selections.iter().fold(0usize, |total, selection| {
            total.saturating_add(selection.included_tokens)
        });

        let snapshot = Self {
            version: CONTEXT_PIPELINE_VERSION,
            project: project.into(),
            task_id: task_id.into(),
            generated_at: Utc::now(),
            token_budget,
            candidate_tokens,
            included_tokens,
            context_fingerprint: context_fingerprint.into(),
            candidates,
            selections,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Validate cross-record invariants without reading source content.
    pub fn validate(&self) -> Result<(), ContextPipelineValidationError> {
        if self.version != CONTEXT_PIPELINE_VERSION {
            return Err(validation_error(format!(
                "unsupported context pipeline version {}",
                self.version
            )));
        }
        if self.project.trim().is_empty() {
            return Err(validation_error("project cannot be empty"));
        }
        if self.task_id.trim().is_empty() {
            return Err(validation_error("task_id cannot be empty"));
        }
        if self.context_fingerprint.trim().is_empty() {
            return Err(validation_error("context_fingerprint cannot be empty"));
        }

        let mut candidate_ids = BTreeSet::new();
        let mut expected_candidate_tokens = 0usize;
        for candidate in &self.candidates {
            if candidate.id.trim().is_empty() {
                return Err(validation_error("candidate id cannot be empty"));
            }
            if !candidate_ids.insert(candidate.id.as_str()) {
                return Err(validation_error(format!(
                    "duplicate context candidate id '{}'",
                    candidate.id
                )));
            }
            if candidate.provenance.locator.trim().is_empty() {
                return Err(validation_error(format!(
                    "candidate '{}' has an empty provenance locator",
                    candidate.id
                )));
            }
            if candidate.provenance.fingerprint.trim().is_empty() {
                return Err(validation_error(format!(
                    "candidate '{}' has an empty provenance fingerprint",
                    candidate.id
                )));
            }
            validate_score(
                candidate.id.as_str(),
                "relevance",
                candidate.relevance_score,
            )?;
            validate_score(
                candidate.id.as_str(),
                "freshness",
                candidate.freshness_score,
            )?;
            expected_candidate_tokens =
                expected_candidate_tokens.saturating_add(candidate.candidate_tokens);
        }

        if self.candidate_tokens != expected_candidate_tokens {
            return Err(validation_error(format!(
                "candidate token total mismatch: snapshot={}, derived={expected_candidate_tokens}",
                self.candidate_tokens
            )));
        }

        let mut selected_ids = BTreeSet::new();
        let mut included_orders = BTreeSet::new();
        let mut expected_included_tokens = 0usize;
        for selection in &self.selections {
            if !candidate_ids.contains(selection.candidate_id.as_str()) {
                return Err(validation_error(format!(
                    "selection references unknown candidate '{}'",
                    selection.candidate_id
                )));
            }
            if !selected_ids.insert(selection.candidate_id.as_str()) {
                return Err(validation_error(format!(
                    "duplicate selection for candidate '{}'",
                    selection.candidate_id
                )));
            }

            let candidate = self
                .candidates
                .iter()
                .find(|candidate| candidate.id == selection.candidate_id)
                .expect("candidate id was checked above");

            match selection.state {
                ContextSelectionState::Included => {
                    if selection.exclusion_reason.is_some() {
                        return Err(validation_error(format!(
                            "included candidate '{}' cannot have an exclusion reason",
                            selection.candidate_id
                        )));
                    }
                    if selection.included_tokens > candidate.candidate_tokens {
                        return Err(validation_error(format!(
                            "included tokens exceed candidate tokens for '{}'",
                            selection.candidate_id
                        )));
                    }
                    let Some(order) = selection.order else {
                        return Err(validation_error(format!(
                            "included candidate '{}' must have a render order",
                            selection.candidate_id
                        )));
                    };
                    if !included_orders.insert(order) {
                        return Err(validation_error(format!(
                            "duplicate included render order {order}"
                        )));
                    }
                    expected_included_tokens =
                        expected_included_tokens.saturating_add(selection.included_tokens);
                }
                ContextSelectionState::Excluded => {
                    if selection.included_tokens != 0 {
                        return Err(validation_error(format!(
                            "excluded candidate '{}' must include zero tokens",
                            selection.candidate_id
                        )));
                    }
                    if selection.trimmed {
                        return Err(validation_error(format!(
                            "excluded candidate '{}' cannot be marked trimmed",
                            selection.candidate_id
                        )));
                    }
                    if selection.exclusion_reason.is_none() {
                        return Err(validation_error(format!(
                            "excluded candidate '{}' must have an exclusion reason",
                            selection.candidate_id
                        )));
                    }
                    if selection.order.is_some() {
                        return Err(validation_error(format!(
                            "excluded candidate '{}' cannot have a render order",
                            selection.candidate_id
                        )));
                    }
                }
            }
        }

        if selected_ids.len() != candidate_ids.len() {
            let missing = candidate_ids
                .difference(&selected_ids)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            return Err(validation_error(format!(
                "every candidate requires a selection; missing: {missing}"
            )));
        }

        if self.included_tokens != expected_included_tokens {
            return Err(validation_error(format!(
                "included token total mismatch: snapshot={}, derived={expected_included_tokens}",
                self.included_tokens
            )));
        }
        if let Some(budget) = self.token_budget
            && self.included_tokens > budget
        {
            return Err(validation_error(format!(
                "included tokens {} exceed token budget {budget}",
                self.included_tokens
            )));
        }

        Ok(())
    }
}

fn validate_score(
    candidate_id: &str,
    label: &str,
    score: Option<f32>,
) -> Result<(), ContextPipelineValidationError> {
    if let Some(score) = score
        && (!score.is_finite() || !(0.0..=1.0).contains(&score))
    {
        return Err(validation_error(format!(
            "candidate '{candidate_id}' has invalid {label} score {score}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPipelineValidationError {
    pub detail: String,
}

impl fmt::Display for ContextPipelineValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for ContextPipelineValidationError {}

fn validation_error(detail: impl Into<String>) -> ContextPipelineValidationError {
    ContextPipelineValidationError {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, tokens: usize) -> ContextCandidate {
        ContextCandidate {
            id: id.to_string(),
            provenance: ContextProvenance {
                kind: ContextSourceKind::ScopedFile,
                locator: format!("src/{id}.rs"),
                fingerprint: format!("sha256:{id}"),
                observed_at: None,
            },
            trust: ContextTrust::Observed,
            candidate_tokens: tokens,
            required: false,
            relevance_score: None,
            freshness_score: None,
        }
    }

    fn included(id: &str, tokens: usize, order: usize) -> ContextSelection {
        ContextSelection {
            candidate_id: id.to_string(),
            state: ContextSelectionState::Included,
            included_tokens: tokens,
            trimmed: false,
            exclusion_reason: None,
            order: Some(order),
        }
    }

    #[test]
    fn snapshot_derives_totals_and_validates_complete_selection() {
        let snapshot = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            Some(200),
            vec![candidate("lib", 100), candidate("auth", 80)],
            vec![included("lib", 100, 0), included("auth", 60, 1)],
        )
        .expect("valid snapshot");

        assert_eq!(snapshot.candidate_tokens, 180);
        assert_eq!(snapshot.included_tokens, 160);
        assert_eq!(snapshot.version, CONTEXT_PIPELINE_VERSION);
    }

    #[test]
    fn damaged_selection_relationships_fail_closed() {
        let mut snapshot = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            None,
            vec![candidate("lib", 100)],
            vec![included("lib", 80, 0)],
        )
        .expect("valid snapshot");

        snapshot.selections[0].candidate_id = "missing".to_string();
        let error = snapshot.validate().expect_err("orphan selection must fail");
        assert!(error.detail.contains("unknown candidate"));
    }

    #[test]
    fn excluded_candidates_require_explicit_reason() {
        let candidate = candidate("lib", 100);
        let selection = ContextSelection {
            candidate_id: "lib".to_string(),
            state: ContextSelectionState::Excluded,
            included_tokens: 0,
            trimmed: false,
            exclusion_reason: None,
            order: None,
        };

        let error = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            None,
            vec![candidate],
            vec![selection],
        )
        .expect_err("unexplained exclusion must fail");
        assert!(error.detail.contains("exclusion reason"));
    }

    #[test]
    fn normalized_scores_are_enforced() {
        let mut invalid = candidate("lib", 100);
        invalid.relevance_score = Some(1.5);

        let error = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            None,
            vec![invalid],
            vec![included("lib", 80, 0)],
        )
        .expect_err("out of range score must fail");
        assert!(error.detail.contains("relevance"));
    }

    #[test]
    fn serialized_contract_contains_metadata_not_source_content() {
        let snapshot = ContextPipelineSnapshot::new(
            "demo",
            "task-1",
            "sha256:context",
            None,
            vec![candidate("lib", 100)],
            vec![included("lib", 80, 0)],
        )
        .expect("valid snapshot");

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        assert!(json.contains("scoped_file"));
        assert!(json.contains("candidate_tokens"));
        assert!(!json.contains("raw_content"));
        assert!(!json.contains("source_text"));
    }
}
