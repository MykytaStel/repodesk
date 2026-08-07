use repodesk_core::engineering::{
    AcceptanceEvidenceReport, ChangeGovernanceSnapshot, ContextInspectorReport,
    EngineeringIntelligence, RunEvidenceSnapshot, WorkItemContractSnapshot, WorkItemContractUpdate,
    derive_change_governance, derive_context_inspector, derive_engineering_intelligence,
    derive_work_item_contract_snapshot, link_active_acceptance_evidence,
    load_active_run_evidence, read_context_manifest, read_events, read_work_item_contract,
    record_active_scope_override, save_active_work_item_contract,
};
use repodesk_core::tasks::show_active_task;
use serde::Serialize;

use super::ErrorPayload;

#[derive(Debug, Serialize)]
pub struct WorkEngineeringSnapshot {
    pub intelligence: EngineeringIntelligence,
    pub context_inspector: ContextInspectorReport,
    pub work_item_contract: WorkItemContractSnapshot,
    pub change_governance: ChangeGovernanceSnapshot,
}

/// Deterministic, task-local engineering aggregate. During the RepoDesk 2
/// migration this registered IPC command also carries the two typed Work Item
/// mutations so we do not grow parallel transport plumbing faster than the
/// domain stabilizes. The frontend still exposes separate read/write functions;
/// validation, evidence and persistence remain in Rust core.
///
/// The ledger is replayed once per snapshot and shared by every projection.
/// This matters because Work, Inspector and Changes poll this aggregate while a
/// task is active; adding another evidence view must not multiply JSONL I/O.
#[tauri::command]
pub fn work_engineering_intelligence(
    contract_update: Option<WorkItemContractUpdate>,
    scope_override_reason: Option<String>,
) -> Result<WorkEngineeringSnapshot, ErrorPayload> {
    if let Some(update) = contract_update {
        save_active_work_item_contract(update).map_err(ErrorPayload::from)?;
    }
    if let Some(reason) = scope_override_reason {
        record_active_scope_override(&reason).map_err(ErrorPayload::from)?;
    }

    let task = show_active_task().map_err(ErrorPayload::from)?;
    let events = read_events(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let intelligence = derive_engineering_intelligence(&events);
    let manifest = read_context_manifest(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let context_inspector = derive_context_inspector(&events, manifest);
    let stored_contract =
        read_work_item_contract(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let work_item_contract = derive_work_item_contract_snapshot(&task, stored_contract, &events);
    let change_governance = derive_change_governance(&task.config.id, &events, &work_item_contract);

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
        work_item_contract,
        change_governance,
    })
}

/// Evidence-first detail for one persisted orchestration run of the active Work
/// Item. Canonical receipts win when available; historical event data is a
/// labeled fallback rather than fabricated proof.
#[tauri::command]
pub fn run_evidence_snapshot(run_id: String) -> Result<RunEvidenceSnapshot, ErrorPayload> {
    load_active_run_evidence(&run_id).map_err(ErrorPayload::from)
}

/// Link one Engineering Contract acceptance criterion to a concrete command in
/// the current canonical VerificationReceipt. The pass/fail value always comes
/// from that receipt, never from the frontend.
#[tauri::command]
pub fn acceptance_evidence_link(
    criterion_id: String,
    command: String,
) -> Result<AcceptanceEvidenceReport, ErrorPayload> {
    link_active_acceptance_evidence(&criterion_id, &command).map_err(ErrorPayload::from)
}
