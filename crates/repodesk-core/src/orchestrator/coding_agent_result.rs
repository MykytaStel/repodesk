//! Convert a low-level coding-agent execution into durable orchestration evidence.
//!
//! The executor owns process/resource safety; this module owns the projection into
//! a `SubAgentResult` that survives run persistence and review. Evidence quality
//! is first-class data, not something callers must infer from human-facing notes.

use crate::executors::CodingAgentExecution;
use crate::tokens::estimate_text;
use crate::usage::cost::{CostConfig, estimate_agent_cost};
use crate::usage::token_ledger::{LogTokenInput, log_token_event};
use crate::worktree::RunWorktree;

use super::types::{SubAgentResult, SubAgentStatus, SubAgentTask};

pub(super) struct CodingAgentFinalization<'a> {
    pub(super) step: &'a SubAgentTask,
    pub(super) project: &'a str,
    pub(super) task_id: &'a str,
    pub(super) execution: CodingAgentExecution,
    pub(super) workspace: Option<RunWorktree>,
    pub(super) verify_cmd_notes: Vec<String>,
    pub(super) verify_failed: bool,
    pub(super) combined_stdout: String,
    pub(super) combined_stderr: String,
    pub(super) input_tokens: usize,
    pub(super) cost_config: &'a CostConfig,
}

pub(super) struct FinalizedCodingAgent {
    pub(super) result: SubAgentResult,
    pub(super) cost_units: f64,
}

pub(super) fn finalize(input: CodingAgentFinalization<'_>) -> FinalizedCodingAgent {
    let CodingAgentFinalization {
        step,
        project,
        task_id,
        execution,
        workspace,
        verify_cmd_notes,
        verify_failed,
        combined_stdout,
        combined_stderr,
        input_tokens,
        cost_config,
    } = input;

    let output_tokens = estimate_text(&execution.stdout).estimated_tokens;
    let cost = estimate_agent_cost(
        cost_config,
        step.resolved_executor_id(),
        input_tokens,
        output_tokens,
    )
    .estimated_cost_units;
    let _ = log_token_event(LogTokenInput {
        agent: step.resolved_executor_id().to_string(),
        model: Some(execution.executor_id.clone()),
        input_tokens,
        output_tokens,
        category: "orchestrate".to_string(),
        notes: Some(step.id.clone()),
    });

    let final_status = final_status(&execution, verify_failed);
    let evidence_issues = evidence_issues(&execution);
    let mut notes = vec![
        format!("command: {}", execution.command_preview),
        format!("stdout: {}", execution.stdout_path),
        format!("stderr: {}", execution.stderr_path),
        format!("duration_ms: {}", execution.duration_ms),
    ];
    notes.extend(verify_cmd_notes);

    let captured_proposals = if final_status == SubAgentStatus::Ok {
        crate::memory::capture_from_text(
            project,
            task_id,
            step.resolved_executor_id(),
            &combined_stdout,
        )
        .map(|proposals| proposals.len())
        .unwrap_or(0)
    } else {
        0
    };

    if let Some(worktree) = &workspace {
        notes.push(format!(
            "isolated workspace: {} (id {}, base {}, metadata {}); review accept can apply it back",
            worktree.path,
            worktree.workspace_id,
            worktree.base_commit,
            worktree.metadata_path.as_deref().unwrap_or("(not recorded)")
        ));
    }

    let changed_files: Vec<String> = execution
        .changed_files
        .iter()
        .map(|change| change.path.clone())
        .collect();
    if changed_files.is_empty() {
        notes.push("changed files: none (no writes detected)".to_string());
    } else {
        notes.push(format!(
            "changed files ({}): {}",
            changed_files.len(),
            changed_files.join(", ")
        ));
        if let Some(diff_path) = &execution.diff_path {
            notes.push(format!(
                "diff: {diff_path}{}",
                if execution.diff_truncated {
                    " (truncated)"
                } else {
                    ""
                }
            ));
        }
    }

    if execution.timed_out {
        notes.push("coding-agent process timed out and was killed".to_string());
    }
    if !evidence_issues.is_empty() {
        notes.push(truncate_note(
            "evidence limitations",
            &evidence_issues.join("; "),
        ));
    }
    if !combined_stderr.trim().is_empty() {
        notes.push(truncate_note("stderr", &combined_stderr));
    }

    FinalizedCodingAgent {
        cost_units: cost,
        result: SubAgentResult {
            task_id: step.id.clone(),
            agent: step.resolved_executor_id().to_string(),
            provider: step
                .resolved_provider_id()
                .unwrap_or(&step.provider)
                .to_string(),
            model: step.model.clone().unwrap_or_default(),
            status: final_status,
            output: combined_stdout,
            input_tokens,
            output_tokens,
            cost_units: cost,
            captured_proposals,
            changed_files,
            diff_path: execution.diff_path.clone(),
            evidence_issues,
            workspace,
            notes,
        },
    }
}

fn final_status(execution: &CodingAgentExecution, verify_failed: bool) -> SubAgentStatus {
    if execution.status == "ok" && !verify_failed && execution.execution_issues.is_empty() {
        SubAgentStatus::Ok
    } else {
        SubAgentStatus::Failed
    }
}

fn evidence_issues(execution: &CodingAgentExecution) -> Vec<String> {
    let mut issues = Vec::with_capacity(
        execution.execution_issues.len() + execution.output_capture_issues.len() + 4,
    );
    issues.extend(execution.execution_issues.iter().cloned());
    issues.extend(execution.output_capture_issues.iter().cloned());
    if execution.stdout_truncated {
        issues.push("executor stdout run-record prefix was truncated at its hard budget".to_string());
    }
    if execution.stderr_truncated {
        issues.push("executor stderr run-record prefix was truncated at its hard budget".to_string());
    }
    if execution.stdout_log_truncated {
        issues.push("executor raw stdout diagnostic prefix was truncated or incomplete".to_string());
    }
    if execution.stderr_log_truncated {
        issues.push("executor raw stderr diagnostic prefix was truncated or incomplete".to_string());
    }
    issues.sort();
    issues.dedup();
    issues
}

fn truncate_note(label: &str, value: &str) -> String {
    const MAX_CHARS: usize = 500;
    let mut text = value.chars().take(MAX_CHARS).collect::<String>();
    if value.chars().count() > MAX_CHARS {
        text.push_str(" [truncated]");
    }
    format!("{label}: {text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution() -> CodingAgentExecution {
        CodingAgentExecution {
            executor_id: "codex_cli".to_string(),
            command_preview: "codex exec".to_string(),
            status: "ok".to_string(),
            exit_code: Some(0),
            duration_ms: 10,
            stdout: "done".to_string(),
            stderr: String::new(),
            stdout_path: "stdout.log".to_string(),
            stderr_path: "stderr.log".to_string(),
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_log_truncated: false,
            stderr_log_truncated: false,
            output_capture_issues: Vec::new(),
            execution_issues: Vec::new(),
            secrets_redacted: Vec::new(),
            timed_out: false,
            changed_files: Vec::new(),
            diff: String::new(),
            diff_truncated: false,
            diff_path: None,
        }
    }

    #[test]
    fn fatal_execution_issue_fails_step_even_if_process_exit_was_ok() {
        let mut execution = execution();
        execution
            .execution_issues
            .push("changeset capture failed after launch".to_string());

        assert_eq!(final_status(&execution, false), SubAgentStatus::Failed);
    }

    #[test]
    fn bounded_capture_limitations_are_durable_evidence() {
        let mut execution = execution();
        execution.stdout_truncated = true;
        execution.stdout_log_truncated = true;
        execution
            .output_capture_issues
            .push("stdout raw log persistence failed: disk full".to_string());

        let issues = evidence_issues(&execution);

        assert!(issues.iter().any(|issue| issue.contains("run-record")));
        assert!(issues.iter().any(|issue| issue.contains("raw stdout")));
        assert!(issues.iter().any(|issue| issue.contains("disk full")));
        assert_eq!(final_status(&execution, false), SubAgentStatus::Ok);
    }
}
