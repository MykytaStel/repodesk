use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::orchestrator::{SubAgentResult, SubAgentStatus};
use repodesk_core::workflow::StepReceipt;
use repodesk_core::worktree::RunWorktree;

pub fn non_write_result(
    task_id: &str,
    provider: &str,
    model: &str,
    status: SubAgentStatus,
    input_tokens: usize,
    output_tokens: usize,
    cost_units: f64,
) -> SubAgentResult {
    SubAgentResult {
        task_id: task_id.to_string(),
        agent: provider.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        status,
        output: String::new(),
        input_tokens,
        output_tokens,
        cost_units,
        captured_proposals: 0,
        changed_files: Vec::new(),
        change_evidence_status: ChangeEvidenceStatus::LegacyUnknown,
        execution_issues: Vec::new(),
        diff_path: None,
        workspace: None,
        notes: Vec::new(),
    }
}

pub fn isolated_write_result(
    task_id: &str,
    agent: &str,
    changed_files: Vec<String>,
    workspace: RunWorktree,
    input_tokens: usize,
    output_tokens: usize,
) -> SubAgentResult {
    SubAgentResult {
        task_id: task_id.to_string(),
        agent: agent.to_string(),
        provider: agent.to_string(),
        model: String::new(),
        status: SubAgentStatus::Ok,
        output: String::new(),
        input_tokens,
        output_tokens,
        cost_units: 0.0,
        captured_proposals: 0,
        changed_files,
        change_evidence_status: ChangeEvidenceStatus::Complete,
        execution_issues: Vec::new(),
        diff_path: None,
        workspace: Some(workspace),
        notes: Vec::new(),
    }
}

pub fn complete_write_step_receipt(task_id: &str, changed_files: Vec<String>) -> StepReceipt {
    StepReceipt {
        task_id: task_id.to_string(),
        status: SubAgentStatus::Ok,
        allow_write: true,
        changed_files,
        change_evidence_status: ChangeEvidenceStatus::Complete,
    }
}
