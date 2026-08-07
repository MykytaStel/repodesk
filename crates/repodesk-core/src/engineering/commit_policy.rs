//! Commit-time Engineering Contract policy.
//!
//! This layer deliberately uses the canonical workflow review receipt for the
//! exact reviewed path set. The append-only engineering ledger is consulted only
//! for an explicit HumanOverride, which is a user-authored governance action and
//! not best-effort telemetry.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, read_events};
use crate::engineering::work_item_contract::{
    ScopeComplianceStatus, WorkItemContract, derive_scope_compliance, read_work_item_contract,
};
use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitScopePolicyDecision {
    pub status: ScopeComplianceStatus,
    pub allowed: bool,
    pub overridden: bool,
    pub override_event_id: Option<String>,
    pub out_of_scope_files: Vec<String>,
    pub protected_files: Vec<String>,
}

impl CommitScopePolicyDecision {
    /// Explain only a real policy blocker. An unconfigured contract remains
    /// backward-compatible and is handled as a warning by the Changes read model.
    pub fn blocker_message(&self) -> Option<String> {
        if self.allowed {
            return None;
        }

        let mut details = Vec::new();
        if !self.out_of_scope_files.is_empty() {
            details.push(format!(
                "out of scope: {}",
                self.out_of_scope_files.join(", ")
            ));
        }
        if !self.protected_files.is_empty() {
            details.push(format!(
                "protected: {}",
                self.protected_files.join(", ")
            ));
        }

        Some(format!(
            "commit blocked by Engineering Contract: {}. Remove those files, update the contract, or record an explicit Human Override in Changes",
            details.join("; ")
        ))
    }
}

/// Commit-time policy for the active Work Item. `reviewed_paths` must come from
/// the canonical accepted ReviewReceipt, not from the working tree or event
/// telemetry. That keeps commit safety independent from best-effort event writes.
pub fn load_active_commit_scope_policy(
    run_id: &str,
    reviewed_paths: &[String],
) -> RepoDeskResult<CommitScopePolicyDecision> {
    let task = show_active_task()?;
    let contract = read_work_item_contract(&task.config.run_dir)?;

    // Preserve the pre-RepoDesk-2 workflow for tasks that have not opted into a
    // typed Engineering Contract yet.
    let Some(contract) = contract else {
        return Ok(derive_commit_scope_policy(
            None,
            reviewed_paths,
            &[],
            &format!("{run_id}-changeset"),
        ));
    };

    let compliance = derive_scope_compliance(&contract, reviewed_paths, true);
    if compliance.status != ScopeComplianceStatus::Violation {
        return Ok(decision_from_compliance(compliance, false, None));
    }

    // Only a real violation requires an event-ledger read. This keeps the normal
    // compliant commit path cheap and avoids making ordinary telemetry critical.
    let events = read_events(&task.config.run_dir)?;
    Ok(derive_commit_scope_policy(
        Some(&contract),
        reviewed_paths,
        &events,
        &format!("{run_id}-changeset"),
    ))
}

/// Pure policy derivation used by tests and future CLI/desktop surfaces.
pub fn derive_commit_scope_policy(
    contract: Option<&WorkItemContract>,
    reviewed_paths: &[String],
    events: &[EngineeringEvent],
    changeset_id: &str,
) -> CommitScopePolicyDecision {
    let Some(contract) = contract else {
        return CommitScopePolicyDecision {
            status: ScopeComplianceStatus::Unconfigured,
            allowed: true,
            overridden: false,
            override_event_id: None,
            out_of_scope_files: Vec::new(),
            protected_files: Vec::new(),
        };
    };

    let compliance = derive_scope_compliance(contract, reviewed_paths, true);
    if compliance.status != ScopeComplianceStatus::Violation {
        return decision_from_compliance(compliance, false, None);
    }

    let override_event = latest_valid_scope_override(events, changeset_id, contract);
    decision_from_compliance(
        compliance,
        override_event.is_some(),
        override_event.map(|event| event.id.to_string()),
    )
}

fn decision_from_compliance(
    compliance: crate::engineering::work_item_contract::ScopeComplianceReport,
    overridden: bool,
    override_event_id: Option<String>,
) -> CommitScopePolicyDecision {
    CommitScopePolicyDecision {
        status: compliance.status,
        allowed: compliance.status != ScopeComplianceStatus::Violation || overridden,
        overridden,
        override_event_id,
        out_of_scope_files: compliance.out_of_scope_files,
        protected_files: compliance.protected_changed_files,
    }
}

fn latest_valid_scope_override<'a>(
    events: &'a [EngineeringEvent],
    changeset_id: &str,
    contract: &WorkItemContract,
) -> Option<&'a EngineeringEvent> {
    events.iter().rev().find(|event| {
        event.kind == EngineeringEventKind::HumanOverride
            && event
                .changeset_id
                .as_ref()
                .is_some_and(|id| id.to_string() == changeset_id)
            && event
                .attributes
                .get("override_kind")
                .and_then(Value::as_str)
                == Some("scope_violation")
            && event
                .attributes
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| !reason.trim().is_empty())
            // The persisted contract timestamp is authoritative. A contract edit
            // after an override invalidates that exception even if best-effort
            // ScopeChanged telemetry was not appended successfully.
            && event.occurred_at >= contract.updated_at
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::Value;

    use super::*;
    use crate::engineering::domain::{ChangeSetId, WorkItemId};

    fn contract(updated_at: chrono::DateTime<Utc>) -> WorkItemContract {
        WorkItemContract {
            version: 1,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            goal: "Keep the change bounded".into(),
            allowed_paths: vec!["src".into()],
            protected_paths: vec!["src/security".into()],
            acceptance_criteria: vec!["tests pass".into()],
            updated_at,
        }
    }

    fn override_event(at: chrono::DateTime<Utc>, changeset_id: &str) -> EngineeringEvent {
        let mut event = EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            EngineeringEventKind::HumanOverride,
        )
        .with_changeset(ChangeSetId::try_new(changeset_id).unwrap())
        .with_attribute("override_kind", Value::String("scope_violation".into()))
        .with_attribute("reason", Value::String("Required cross-scope update".into()));
        event.occurred_at = at;
        event
    }

    #[test]
    fn unconfigured_contract_keeps_legacy_commit_path_available() {
        let decision = derive_commit_scope_policy(
            None,
            &["README.md".into()],
            &[],
            "run-1-changeset",
        );
        assert!(decision.allowed);
        assert_eq!(decision.status, ScopeComplianceStatus::Unconfigured);
    }

    #[test]
    fn violation_blocks_without_override() {
        let updated = Utc.with_ymd_and_hms(2026, 8, 7, 18, 0, 0).unwrap();
        let decision = derive_commit_scope_policy(
            Some(&contract(updated)),
            &["README.md".into(), "src/security/key.rs".into()],
            &[],
            "run-1-changeset",
        );
        assert!(!decision.allowed);
        assert_eq!(decision.out_of_scope_files, vec!["README.md"]);
        assert_eq!(decision.protected_files, vec!["src/security/key.rs"]);
        assert!(decision.blocker_message().is_some());
    }

    #[test]
    fn matching_override_after_contract_update_allows_commit_policy() {
        let updated = Utc.with_ymd_and_hms(2026, 8, 7, 18, 0, 0).unwrap();
        let override_at = Utc.with_ymd_and_hms(2026, 8, 7, 18, 5, 0).unwrap();
        let events = vec![override_event(override_at, "run-1-changeset")];
        let decision = derive_commit_scope_policy(
            Some(&contract(updated)),
            &["README.md".into()],
            &events,
            "run-1-changeset",
        );
        assert!(decision.allowed);
        assert!(decision.overridden);
        assert!(decision.override_event_id.is_some());
    }

    #[test]
    fn contract_edit_after_override_invalidates_exception() {
        let override_at = Utc.with_ymd_and_hms(2026, 8, 7, 18, 0, 0).unwrap();
        let updated = Utc.with_ymd_and_hms(2026, 8, 7, 18, 5, 0).unwrap();
        let events = vec![override_event(override_at, "run-1-changeset")];
        let decision = derive_commit_scope_policy(
            Some(&contract(updated)),
            &["README.md".into()],
            &events,
            "run-1-changeset",
        );
        assert!(!decision.allowed);
        assert!(!decision.overridden);
    }

    #[test]
    fn override_for_another_changeset_does_not_bypass_scope() {
        let updated = Utc.with_ymd_and_hms(2026, 8, 7, 18, 0, 0).unwrap();
        let override_at = Utc.with_ymd_and_hms(2026, 8, 7, 18, 5, 0).unwrap();
        let events = vec![override_event(override_at, "run-2-changeset")];
        let decision = derive_commit_scope_policy(
            Some(&contract(updated)),
            &["README.md".into()],
            &events,
            "run-1-changeset",
        );
        assert!(!decision.allowed);
    }
}
