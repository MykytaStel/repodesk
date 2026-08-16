use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::orchestrator::SubAgentStatus;
use repodesk_core::workflow::{ExecutionReceipt, StepReceipt};
use repodesk_core::orchestrator::RunStatus;

#[test]
fn legacy_change_evidence_defaults_to_unknown() {
    let status: ChangeEvidenceStatus = serde_json::from_str("null")
        .unwrap_or_default();
    assert_eq!(status, ChangeEvidenceStatus::LegacyUnknown);

    let step: StepReceipt = serde_json::from_value(serde_json::json!({
        "task_id": "implement",
        "status": "Ok",
        "allow_write": true,
        "changed_files": []
    }))
    .expect("historical receipts must remain loadable");

    assert_eq!(step.change_evidence_status, ChangeEvidenceStatus::LegacyUnknown);
}

#[test]
fn successful_write_receipt_requires_complete_change_evidence() {
    let receipt_with = |change_evidence_status| ExecutionReceipt {
        status: RunStatus::Completed,
        required_steps: vec![StepReceipt {
            task_id: "implement".into(),
            status: SubAgentStatus::Ok,
            allow_write: true,
            changed_files: Vec::new(),
            change_evidence_status,
        }],
        changeset_digest: None,
    };

    assert!(receipt_with(ChangeEvidenceStatus::Complete).succeeded());
    assert!(!receipt_with(ChangeEvidenceStatus::Unavailable).succeeded());
    assert!(!receipt_with(ChangeEvidenceStatus::LegacyUnknown).succeeded());
}

#[test]
fn evidence_status_has_distinct_incomplete_state() {
    assert_eq!(
        serde_json::to_string(&repodesk_core::orchestrator::ExecutionEvidenceStatus::Incomplete)
            .expect("status serializes"),
        "\"incomplete\""
    );
}
