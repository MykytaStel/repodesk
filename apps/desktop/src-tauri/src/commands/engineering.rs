use repodesk_core::engineering::{
    AiUsageReport, ChangeGovernanceSnapshot, ContextInspectorReport, EngineeringIntelligence,
    EngineeringKnowledgeLifecycleReport, EngineeringKnowledgeProposalInput,
    EngineeringKnowledgeSnapshot, RunEvidenceSnapshot, RunObservabilityReport,
    WorkItemContractSnapshot, WorkItemContractUpdate, accept_active_engineering_knowledge,
    archive_active_engineering_knowledge, capture_active_verified_command,
    derive_ai_usage_report, derive_change_governance, derive_engineering_intelligence,
    derive_engineering_knowledge_lifecycle, derive_run_observability,
    derive_work_item_contract_snapshot, link_active_acceptance_evidence,
    load_active_engineering_knowledge, load_active_run_evidence_from_events,
    load_context_inspector, propose_active_engineering_knowledge, read_events,
    read_work_item_contract, reconfirm_active_engineering_knowledge,
    record_active_scope_override, save_active_work_item_contract,
};
use repodesk_core::tasks::show_active_task;
use serde::{Deserialize, Serialize};

use super::ErrorPayload;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineeringKnowledgeAction {
    Propose {
        input: EngineeringKnowledgeProposalInput,
    },
    CaptureCommand {
        command: String,
    },
    Accept {
        knowledge_id: String,
    },
    Reconfirm {
        knowledge_id: String,
    },
    Archive {
        knowledge_id: String,
    },
}

#[derive(Debug, Serialize)]
pub struct WorkEngineeringSnapshot {
    /// Task-local projections are optional only so the same temporary RepoDesk 2
    /// transport can serve project-level Knowledge when no Work Item is active.
    /// Normal Work/Changes/Inspector calls still receive every field populated.
    pub intelligence: Option<EngineeringIntelligence>,
    pub ai_usage_report: Option<AiUsageReport>,
    pub context_inspector: Option<ContextInspectorReport>,
    pub work_item_contract: Option<WorkItemContractSnapshot>,
    pub change_governance: Option<ChangeGovernanceSnapshot>,
    pub run_evidence: Option<RunEvidenceSnapshot>,
    pub run_observability: Option<RunObservabilityReport>,
    pub knowledge: Option<EngineeringKnowledgeSnapshot>,
    pub knowledge_lifecycle: Option<EngineeringKnowledgeLifecycleReport>,
}

/// Temporary RepoDesk 2 engineering transport. Task-local Work projections and
/// project-local Knowledge share one registered IPC boundary while the domain is
/// still migrating. This deliberately avoids adding parallel handler commands.
/// AI-usage and run-observability projections reuse the same event replay so the
/// frontend does not need to issue extra ledger reads for explanatory metrics.
#[tauri::command]
pub fn work_engineering_intelligence(
    contract_update: Option<WorkItemContractUpdate>,
    scope_override_reason: Option<String>,
    run_evidence_id: Option<String>,
    acceptance_criterion_id: Option<String>,
    acceptance_command: Option<String>,
    include_knowledge: Option<bool>,
    knowledge_action: Option<EngineeringKnowledgeAction>,
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

    let mut knowledge = match knowledge_action {
        Some(EngineeringKnowledgeAction::Propose { input }) => {
            Some(propose_active_engineering_knowledge(input).map_err(ErrorPayload::from)?)
        }
        Some(EngineeringKnowledgeAction::CaptureCommand { command }) => {
            Some(capture_active_verified_command(&command).map_err(ErrorPayload::from)?)
        }
        Some(EngineeringKnowledgeAction::Accept { knowledge_id }) => {
            Some(accept_active_engineering_knowledge(&knowledge_id).map_err(ErrorPayload::from)?)
        }
        Some(EngineeringKnowledgeAction::Reconfirm { knowledge_id }) => {
            Some(reconfirm_active_engineering_knowledge(&knowledge_id).map_err(ErrorPayload::from)?)
        }
        Some(EngineeringKnowledgeAction::Archive { knowledge_id }) => {
            Some(archive_active_engineering_knowledge(&knowledge_id).map_err(ErrorPayload::from)?)
        }
        None => None,
    };
    if include_knowledge.unwrap_or(false) && knowledge.is_none() {
        knowledge = Some(load_active_engineering_knowledge().map_err(ErrorPayload::from)?);
    }
    let knowledge_lifecycle = knowledge
        .as_ref()
        .map(derive_engineering_knowledge_lifecycle);

    let task = match show_active_task() {
        Ok(task) => task,
        Err(_) if knowledge.is_some() => {
            return Ok(WorkEngineeringSnapshot {
                intelligence: None,
                ai_usage_report: None,
                context_inspector: None,
                work_item_contract: None,
                change_governance: None,
                run_evidence: None,
                run_observability: None,
                knowledge,
                knowledge_lifecycle,
            });
        }
        Err(error) => return Err(ErrorPayload::from(error)),
    };

    let events = read_events(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let intelligence = derive_engineering_intelligence(&events);
    let ai_usage_report = derive_ai_usage_report(&events, &intelligence);
    let context_inspector = load_context_inspector(&task.config.run_dir).map_err(ErrorPayload::from)?;
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
    let run_observability = run_evidence
        .as_ref()
        .map(|evidence| derive_run_observability(evidence, &events));

    Ok(WorkEngineeringSnapshot {
        intelligence: Some(intelligence),
        ai_usage_report: Some(ai_usage_report),
        context_inspector: Some(context_inspector),
        work_item_contract: Some(work_item_contract),
        change_governance: Some(change_governance),
        run_evidence,
        run_observability,
        knowledge,
        knowledge_lifecycle,
    })
}
