use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::domain::EvidenceRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDecisionKind {
    RunNow,
    RunTargeted,
    DeferWithDebt,
    AskForApproval,
    PauseAndReview,
    StopWithPartialResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationDebt {
    pub check_id: String,
    pub title: String,
    pub reason: String,
    pub required_before: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecisionReceiptError {
    #[error("{field} must not be empty")]
    EmptyRequiredField { field: &'static str },
}

fn validate_required(field: &'static str, value: &str) -> Result<(), DecisionReceiptError> {
    if value.trim().is_empty() {
        return Err(DecisionReceiptError::EmptyRequiredField { field });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionReceipt {
    pub decision_id: String,
    pub project: String,
    pub work_item_id: String,
    pub execution_id: Option<String>,
    pub tree_identity: String,
    pub changeset_identity: Option<String>,
    pub decision_kind: VerificationDecisionKind,
    pub policy_version: String,
    pub observed_facts: BTreeMap<String, Value>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub selected_actions: Vec<String>,
    pub skipped_actions: Vec<String>,
    pub estimated_cost_units: Option<f64>,
    pub estimated_wall_clock_ms: Option<u64>,
    pub risk_label: String,
    pub uncertainty_label: String,
    pub verification_debt: Vec<VerificationDebt>,
    pub human_override: Option<String>,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl DecisionReceipt {
    pub fn try_new(
        project: impl Into<String>,
        work_item_id: impl Into<String>,
        tree_identity: impl Into<String>,
        decision_kind: VerificationDecisionKind,
        policy_version: impl Into<String>,
    ) -> Result<Self, DecisionReceiptError> {
        let project = project.into();
        let work_item_id = work_item_id.into();
        let tree_identity = tree_identity.into();
        let policy_version = policy_version.into();
        validate_required("project", &project)?;
        validate_required("work_item_id", &work_item_id)?;
        validate_required("tree_identity", &tree_identity)?;
        validate_required("policy_version", &policy_version)?;

        let created_at = Utc::now();
        Ok(Self {
            decision_id: format!("decision-{}", created_at.timestamp_micros()),
            project,
            work_item_id,
            execution_id: None,
            tree_identity,
            changeset_identity: None,
            decision_kind,
            policy_version,
            observed_facts: BTreeMap::new(),
            evidence_refs: Vec::new(),
            selected_actions: Vec::new(),
            skipped_actions: Vec::new(),
            estimated_cost_units: None,
            estimated_wall_clock_ms: None,
            risk_label: "unknown".to_string(),
            uncertainty_label: "unknown".to_string(),
            verification_debt: Vec::new(),
            human_override: None,
            outcome: None,
            created_at,
        })
    }

    pub fn new(
        project: impl Into<String>,
        work_item_id: impl Into<String>,
        tree_identity: impl Into<String>,
        decision_kind: VerificationDecisionKind,
        policy_version: impl Into<String>,
    ) -> Self {
        Self::try_new(
            project,
            work_item_id,
            tree_identity,
            decision_kind,
            policy_version,
        )
        .expect("DecisionReceipt::new requires non-empty identity fields")
    }

    pub fn with_observed_fact(mut self, key: impl Into<String>, value: Value) -> Self {
        self.observed_facts.insert(key.into(), value);
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence_refs.push(evidence);
        self
    }

    pub fn with_debt(mut self, debt: VerificationDebt) -> Self {
        self.verification_debt.push(debt);
        self
    }
}
