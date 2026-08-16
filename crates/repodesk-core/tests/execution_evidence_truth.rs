use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::orchestrator::RunStatus;
use repodesk_core::orchestrator::SubAgentStatus;
use repodesk_core::workflow::{ExecutionReceipt, StepReceipt};

#[test]
fn legacy_change_evidence_defaults_to_unknown() {
    let status: ChangeEvidenceStatus = serde_json::from_str("null").unwrap_or_default();
    assert_eq!(status, ChangeEvidenceStatus::LegacyUnknown);

    let step: StepReceipt = serde_json::from_value(serde_json::json!({
        "task_id": "implement",
        "status": "ok",
        "allow_write": true,
        "changed_files": []
    }))
    .expect("historical receipts must remain loadable");

    assert_eq!(
        step.change_evidence_status,
        ChangeEvidenceStatus::LegacyUnknown
    );
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
fn successful_read_only_receipt_does_not_require_changeset_provenance() {
    let receipt = ExecutionReceipt {
        status: RunStatus::Completed,
        required_steps: vec![StepReceipt {
            task_id: "analyze".into(),
            status: SubAgentStatus::Ok,
            allow_write: false,
            changed_files: Vec::new(),
            change_evidence_status: ChangeEvidenceStatus::LegacyUnknown,
        }],
        changeset_digest: None,
    };

    assert!(receipt.succeeded());
}

#[test]
fn evidence_status_has_distinct_incomplete_state() {
    assert_eq!(
        serde_json::to_string(&repodesk_core::orchestrator::ExecutionEvidenceStatus::Incomplete)
            .expect("status serializes"),
        "\"incomplete\""
    );
}

#[test]
fn historical_subagent_result_defaults_to_unknown_evidence() {
    let result: repodesk_core::orchestrator::SubAgentResult =
        serde_json::from_value(serde_json::json!({
            "task_id": "legacy",
            "agent": "manual",
            "provider": "manual",
            "model": "external",
            "status": "ok",
            "output": "done",
            "input_tokens": 0,
            "output_tokens": 0,
            "cost_units": 0.0,
            "captured_proposals": 0,
            "changed_files": [],
            "notes": []
        }))
        .expect("historical run results must remain loadable");

    assert_eq!(
        result.change_evidence_status,
        ChangeEvidenceStatus::LegacyUnknown
    );
    assert!(result.execution_issues.is_empty());
}
