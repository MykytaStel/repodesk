//! Deterministic lifecycle assessment for reviewed Project Engineering Knowledge.
//!
//! Engineering Knowledge is durable, but "accepted once" must not mean
//! "trusted forever". This module derives a review posture from existing record
//! metadata without mutating the store or calling an AI provider. The policy is
//! intentionally category-aware: executable/tooling knowledge ages faster than
//! architecture and durable decisions.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::knowledge::{
    EngineeringKnowledgeCategory, EngineeringKnowledgeOrigin, EngineeringKnowledgeRecord,
    EngineeringKnowledgeSnapshot, EngineeringKnowledgeStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringKnowledgeLifecycleState {
    PendingReview,
    Current,
    ReviewSoon,
    ReviewRequired,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeLifecyclePolicy {
    pub review_after_days: i64,
    pub warning_window_days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeLifecycleEntry {
    pub knowledge_id: String,
    pub state: EngineeringKnowledgeLifecycleState,
    pub age_days: i64,
    pub review_after_days: Option<i64>,
    pub review_due_at: Option<DateTime<Utc>>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EngineeringKnowledgeLifecycleCounts {
    pub pending_review: usize,
    pub current: usize,
    pub review_soon: usize,
    pub review_required: usize,
    pub archived: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringKnowledgeLifecycleReport {
    pub project: String,
    pub generated_at: DateTime<Utc>,
    pub counts: EngineeringKnowledgeLifecycleCounts,
    pub entries: Vec<EngineeringKnowledgeLifecycleEntry>,
}

/// Category-aware review cadence.
///
/// Commands, tests and tooling are coupled to executable project state and age
/// quickly. Architecture/decision records are intentionally slower-moving.
/// Verification-origin records are capped at 60 days because their evidence is
/// tied to a historical verified tree even when the category is durable.
pub fn engineering_knowledge_lifecycle_policy(
    category: EngineeringKnowledgeCategory,
    origin: EngineeringKnowledgeOrigin,
) -> EngineeringKnowledgeLifecyclePolicy {
    let category_days = match category {
        EngineeringKnowledgeCategory::Command
        | EngineeringKnowledgeCategory::Testing
        | EngineeringKnowledgeCategory::Tooling => 60,
        EngineeringKnowledgeCategory::Performance => 90,
        EngineeringKnowledgeCategory::Convention | EngineeringKnowledgeCategory::Hazard => 180,
        EngineeringKnowledgeCategory::Architecture | EngineeringKnowledgeCategory::Decision => 365,
    };
    let review_after_days = if origin == EngineeringKnowledgeOrigin::Verification {
        category_days.min(60)
    } else {
        category_days
    };
    let warning_window_days = (review_after_days / 4).max(14);

    EngineeringKnowledgeLifecyclePolicy {
        review_after_days,
        warning_window_days,
    }
}

pub fn assess_engineering_knowledge_at(
    record: &EngineeringKnowledgeRecord,
    now: DateTime<Utc>,
) -> EngineeringKnowledgeLifecycleEntry {
    let age_days = (now - record.updated_at).num_days().max(0);

    match record.status {
        EngineeringKnowledgeStatus::Candidate => EngineeringKnowledgeLifecycleEntry {
            knowledge_id: record.id.to_string(),
            state: EngineeringKnowledgeLifecycleState::PendingReview,
            age_days,
            review_after_days: None,
            review_due_at: None,
            reason: "Human review is required before this record can become durable knowledge."
                .to_string(),
        },
        EngineeringKnowledgeStatus::Archived => EngineeringKnowledgeLifecycleEntry {
            knowledge_id: record.id.to_string(),
            state: EngineeringKnowledgeLifecycleState::Archived,
            age_days,
            review_after_days: None,
            review_due_at: None,
            reason: "Archived knowledge is retained for auditability, not active guidance."
                .to_string(),
        },
        EngineeringKnowledgeStatus::Accepted => {
            let policy = engineering_knowledge_lifecycle_policy(record.category, record.origin);
            let review_due_at = record.updated_at + Duration::days(policy.review_after_days);
            let warning_at = review_due_at - Duration::days(policy.warning_window_days);
            let (state, reason) = if now >= review_due_at {
                (
                    EngineeringKnowledgeLifecycleState::ReviewRequired,
                    "The review cadence has expired; confirm this knowledge against current project state before relying on it again.",
                )
            } else if now >= warning_at {
                (
                    EngineeringKnowledgeLifecycleState::ReviewSoon,
                    "This accepted knowledge is approaching its review boundary.",
                )
            } else {
                (
                    EngineeringKnowledgeLifecycleState::Current,
                    "This accepted knowledge is within its category review cadence.",
                )
            };

            EngineeringKnowledgeLifecycleEntry {
                knowledge_id: record.id.to_string(),
                state,
                age_days,
                review_after_days: Some(policy.review_after_days),
                review_due_at: Some(review_due_at),
                reason: reason.to_string(),
            }
        }
    }
}

pub fn derive_engineering_knowledge_lifecycle_at(
    snapshot: &EngineeringKnowledgeSnapshot,
    now: DateTime<Utc>,
) -> EngineeringKnowledgeLifecycleReport {
    let entries = snapshot
        .records
        .iter()
        .map(|record| assess_engineering_knowledge_at(record, now))
        .collect::<Vec<_>>();
    let mut counts = EngineeringKnowledgeLifecycleCounts::default();
    for entry in &entries {
        match entry.state {
            EngineeringKnowledgeLifecycleState::PendingReview => counts.pending_review += 1,
            EngineeringKnowledgeLifecycleState::Current => counts.current += 1,
            EngineeringKnowledgeLifecycleState::ReviewSoon => counts.review_soon += 1,
            EngineeringKnowledgeLifecycleState::ReviewRequired => counts.review_required += 1,
            EngineeringKnowledgeLifecycleState::Archived => counts.archived += 1,
        }
    }

    EngineeringKnowledgeLifecycleReport {
        project: snapshot.project.clone(),
        generated_at: now,
        counts,
        entries,
    }
}

pub fn derive_engineering_knowledge_lifecycle(
    snapshot: &EngineeringKnowledgeSnapshot,
) -> EngineeringKnowledgeLifecycleReport {
    derive_engineering_knowledge_lifecycle_at(snapshot, Utc::now())
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::engineering::domain::EngineeringKnowledgeId;
    use crate::engineering::knowledge::{EngineeringKnowledgeCounts, EngineeringKnowledgeSuggestion};

    fn record(
        category: EngineeringKnowledgeCategory,
        origin: EngineeringKnowledgeOrigin,
        status: EngineeringKnowledgeStatus,
        updated_at: DateTime<Utc>,
    ) -> EngineeringKnowledgeRecord {
        EngineeringKnowledgeRecord {
            id: EngineeringKnowledgeId::try_new("knowledge-lifecycle-test").unwrap(),
            project: "demo".into(),
            category,
            title: "Rule".into(),
            content: "Keep this rule.".into(),
            status,
            origin,
            source_work_item_id: None,
            evidence: Vec::new(),
            created_at: updated_at,
            updated_at,
        }
    }

    #[test]
    fn executable_knowledge_ages_faster_than_architecture() {
        let command = engineering_knowledge_lifecycle_policy(
            EngineeringKnowledgeCategory::Command,
            EngineeringKnowledgeOrigin::Human,
        );
        let architecture = engineering_knowledge_lifecycle_policy(
            EngineeringKnowledgeCategory::Architecture,
            EngineeringKnowledgeOrigin::Human,
        );
        assert_eq!(command.review_after_days, 60);
        assert_eq!(architecture.review_after_days, 365);
    }

    #[test]
    fn verification_origin_is_never_reviewed_slower_than_sixty_days() {
        let policy = engineering_knowledge_lifecycle_policy(
            EngineeringKnowledgeCategory::Architecture,
            EngineeringKnowledgeOrigin::Verification,
        );
        assert_eq!(policy.review_after_days, 60);
    }

    #[test]
    fn accepted_record_moves_from_current_to_warning_to_review_required() {
        let now = Utc::now();
        let current = record(
            EngineeringKnowledgeCategory::Testing,
            EngineeringKnowledgeOrigin::Human,
            EngineeringKnowledgeStatus::Accepted,
            now - Duration::days(10),
        );
        let warning = record(
            EngineeringKnowledgeCategory::Testing,
            EngineeringKnowledgeOrigin::Human,
            EngineeringKnowledgeStatus::Accepted,
            now - Duration::days(50),
        );
        let expired = record(
            EngineeringKnowledgeCategory::Testing,
            EngineeringKnowledgeOrigin::Human,
            EngineeringKnowledgeStatus::Accepted,
            now - Duration::days(61),
        );

        assert_eq!(
            assess_engineering_knowledge_at(&current, now).state,
            EngineeringKnowledgeLifecycleState::Current
        );
        assert_eq!(
            assess_engineering_knowledge_at(&warning, now).state,
            EngineeringKnowledgeLifecycleState::ReviewSoon
        );
        assert_eq!(
            assess_engineering_knowledge_at(&expired, now).state,
            EngineeringKnowledgeLifecycleState::ReviewRequired
        );
    }

    #[test]
    fn candidate_and_archived_states_override_age_policy() {
        let now = Utc::now();
        let candidate = record(
            EngineeringKnowledgeCategory::Architecture,
            EngineeringKnowledgeOrigin::Human,
            EngineeringKnowledgeStatus::Candidate,
            now - Duration::days(800),
        );
        let archived = record(
            EngineeringKnowledgeCategory::Architecture,
            EngineeringKnowledgeOrigin::Human,
            EngineeringKnowledgeStatus::Archived,
            now,
        );
        assert_eq!(
            assess_engineering_knowledge_at(&candidate, now).state,
            EngineeringKnowledgeLifecycleState::PendingReview
        );
        assert_eq!(
            assess_engineering_knowledge_at(&archived, now).state,
            EngineeringKnowledgeLifecycleState::Archived
        );
    }

    #[test]
    fn report_counts_each_lifecycle_state() {
        let now = Utc::now();
        let records = vec![
            record(
                EngineeringKnowledgeCategory::Architecture,
                EngineeringKnowledgeOrigin::Human,
                EngineeringKnowledgeStatus::Accepted,
                now,
            ),
            record(
                EngineeringKnowledgeCategory::Testing,
                EngineeringKnowledgeOrigin::Human,
                EngineeringKnowledgeStatus::Accepted,
                now - Duration::days(61),
            ),
        ];
        let mut second = records[1].clone();
        second.id = EngineeringKnowledgeId::try_new("knowledge-lifecycle-test-2").unwrap();
        let snapshot = EngineeringKnowledgeSnapshot {
            project: "demo".into(),
            records: vec![records[0].clone(), second],
            counts: EngineeringKnowledgeCounts {
                candidates: 0,
                accepted: 2,
                archived: 0,
            },
            suggestions: Vec::<EngineeringKnowledgeSuggestion>::new(),
        };
        let report = derive_engineering_knowledge_lifecycle_at(&snapshot, now);
        assert_eq!(report.counts.current, 1);
        assert_eq!(report.counts.review_required, 1);
    }
}
