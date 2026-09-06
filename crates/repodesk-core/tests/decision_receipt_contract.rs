use std::collections::BTreeMap;

use chrono::Utc;
use repodesk_core::engineering::{
    DecisionReceipt, EvidenceKind, EvidenceRef, VerificationDebt, VerificationDecisionKind,
};
use serde_json::json;

#[test]
fn decision_kind_serializes_as_snake_case() {
    let value = serde_json::to_value(VerificationDecisionKind::RunTargeted).unwrap();
    assert_eq!(value, json!("run_targeted"));
}

#[test]
fn receipt_preserves_unknown_and_deferred_evidence() {
    let receipt = DecisionReceipt::new(
        "project",
        "work-item",
        "tree-sha",
        VerificationDecisionKind::DeferWithDebt,
        "policy-v1",
    )
    .with_observed_fact("test_count", json!(null))
    .with_evidence(EvidenceRef {
        kind: EvidenceKind::Verification,
        locator: "runs/check.log".into(),
    })
    .with_debt(VerificationDebt {
        check_id: "integration".into(),
        title: "Integration suite".into(),
        reason: "Too expensive for this low-risk change".into(),
        required_before: "commit".into(),
    });

    assert_eq!(receipt.observed_facts["test_count"], json!(null));
    assert_eq!(receipt.evidence_refs.len(), 1);
    assert_eq!(receipt.verification_debt.len(), 1);
}

#[test]
fn receipt_rejects_blank_required_identity() {
    let result = DecisionReceipt::try_new(
        "",
        "work-item",
        "tree-sha",
        VerificationDecisionKind::RunNow,
        "policy-v1",
    );

    assert!(result.is_err());
}

#[test]
fn receipt_keeps_optional_cost_and_outcome_unknown() {
    let receipt = DecisionReceipt::new(
        "project",
        "work-item",
        "tree-sha",
        VerificationDecisionKind::AskForApproval,
        "policy-v1",
    );

    assert_eq!(receipt.estimated_cost_units, None);
    assert_eq!(receipt.outcome, None);
    assert!(receipt.created_at <= Utc::now());
    assert!(BTreeMap::<String, serde_json::Value>::new().is_empty());
}
