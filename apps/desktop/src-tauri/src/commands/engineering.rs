use repodesk_core::engineering::{
    AcceptanceEvidenceReport, ChangeGovernanceSnapshot, ContextInspectorReport,
    EngineeringIntelligence, EngineeringKnowledgeProposalInput, EngineeringKnowledgeSnapshot,
    RunEvidenceSnapshot, WorkItemContractSnapshot, WorkItemContractUpdate,
    accept_active_engineering_knowledge, archive_active_engineering_knowledge,
    capture_active_verified_command, derive_change_governance, derive_context_inspector,
    derive_engineering_intelligence, derive_work_item_contract_snapshot,
    link_active_acceptance_evidence, load_active_engineering_knowledge, load_active_run_evidence,
    load_active_run_evidence_from_events, propose_active_engineering_knowledge,
    read_context_manifest, read_events, read_work_item_contract, record_active_scope_override,
    save_active_work_item_contract,
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
    pub run_evidence: Option<RunEvidenceSnapshot>,
}

/// Deterministic, task-local engineering aggregate. During the RepoDesk 2
/// migration this registered IPC command also carries typed Work Item evidence
/// mutations so we do not grow parallel transport plumbing faster than the
/// domain stabilizes. The normal Work/Inspector polling path stays lightweight;
/// run evidence is loaded only when `run_evidence_id` is explicitly requested.
/// All event-backed projections reuse the same ledger read.
#[tauri::command]
pub fn work_engineering_intelligence(
    contract_update: Option<WorkItemContractUpdate>,
    scope_override_reason: Option<String>,
    run_evidence_id: Option<String>,
    acceptance_criterion_id: Option<String>,
    acceptance_command: Option<String>,
) -> Result<WorkEngineeringSnapshot, ErrorPayload> {
    if let Some(update) = contract_update {
        save_active_work_item_contract(update).map_err(ErrorPayload::from)?;
    }
    if let Some(reason) = scope_override_reason {
        record_active_scope_override(&reason).map_err(ErrorPayload::from)?;
    }
    match (acceptance_criterion_id, acceptance_command) {
        (Some(criterion_id), Some(command)) => {
            link_active_acceptance_evidence(&criterion_id, &command).map_err(ErrorPayload::from)?;
        }
        (None, None) => {}
        _ => {
            return Err(ErrorPayload::configuration(
                "Acceptance evidence requires both criterion id and verification command"
                    .to_string(),
            ));
        }
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
    let run_evidence = match run_evidence_id {
        Some(run_id) => Some(
            load_active_run_evidence_from_events(&run_id, &events).map_err(ErrorPayload::from)?,
        ),
        None => None,
    };

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
        work_item_contract,
        change_governance,
        run_evidence,
    })
}

#[tauri::command]
pub fn engineering_knowledge_snapshot() -> Result<EngineeringKnowledgeSnapshot, ErrorPayload> {
    load_active_engineering_knowledge().map_err(ErrorPayload::from)
}

#[tauri::command]
pub fn engineering_knowledge_propose(
    input: EngineeringKnowledgeProposalInput,
) -> Result<EngineeringKnowledgeSnapshot, ErrorPayload> {
    propose_active_engineering_knowledge(input).map_err(ErrorPayload::from)
}

#[tauri::command]
pub fn engineering_knowledge_capture_command(
    command: String,
) -> Result<EngineeringKnowledgeSnapshot, ErrorPayload> {
    capture_active_verified_command(&command).map_err(ErrorPayload::from)
}

#[tauri::command]
pub fn engineering_knowledge_accept(
    knowledge_id: String,
) -> Result<EngineeringKnowledgeSnapshot, ErrorPayload> {
    accept_active_engineering_knowledge(&knowledge_id).map_err(ErrorPayload::from)
}

#[tauri::command]
pub fn engineering_knowledge_archive(
    knowledge_id: String,
) -> Result<EngineeringKnowledgeSnapshot, ErrorPayload> {
    archive_active_engineering_knowledge(&knowledge_id).map_err(ErrorPayload::from)
}

/// Direct helpers retained as a narrow Rust/Tauri boundary for future transport
/// cleanup. The current desktop UI uses the aggregate command above so adding
/// Runs does not require another large handler registry edit in this migration.
pub fn run_evidence_snapshot(run_id: String) -> Result<RunEvidenceSnapshot, ErrorPayload> {
    load_active_run_evidence(&run_id).map_err(ErrorPayload::from)
}

pub fn acceptance_evidence_link(
    criterion_id: String,
    command: String,
) -> Result<AcceptanceEvidenceReport, ErrorPayload> {
    link_active_acceptance_evidence(&criterion_id, &command).map_err(ErrorPayload::from)
}
