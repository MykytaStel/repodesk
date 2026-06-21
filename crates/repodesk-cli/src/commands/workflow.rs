#![allow(unused_imports)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use repodesk_core::command_sandbox::{format_sandbox_plan, plan_command, sandbox_policy};
use repodesk_core::dashboard::{dashboard_json, dashboard_summary};
use repodesk_core::git_audit::{backup_plan, git_audit};
use repodesk_core::persistence::receipts::{AddReceiptInput, add_receipt, read_receipts};
use repodesk_core::repo_map::{build_repo_map, format_hotspots, format_repo_map};
use repodesk_core::runtime::{
    format_provider_status, format_runtime_providers, format_runtime_route, provider_status,
    runtime_providers, runtime_snapshot_json,
};
use repodesk_core::smart_context::{
    build_smart_context, format_smart_context_result, list_smart_context_sources,
};

use repodesk_core::access::{evaluate_access, format_access_matrix, format_access_report};
use repodesk_core::agents::{ensure_agents_config, format_agents, recommend_agents};
use repodesk_core::ai_adapters::{
    check_ai_status, ensure_ai_adapters_config, format_ai_adapters, format_ai_recommendations,
    format_ai_status, recommend_ai_adapters,
};
use repodesk_core::brain::{format_brain_status, read_brain_status};
use repodesk_core::capabilities::{
    audit_capabilities, ensure_capabilities_config, format_capabilities, format_capability_audit,
    format_capability_recommendations, recommend_capabilities,
};
use repodesk_core::checks::{last_checks, run_checks, summarize_last_checks};
use repodesk_core::context::{build_context, estimate_active_context};
use repodesk_core::desktop::{
    desktop_events_spec, desktop_plan, desktop_scaffold_hint, tauri_bridge_spec,
};
use repodesk_core::guard::{format_guard_result, preflight};
use repodesk_core::init;
use repodesk_core::judge::{format_judgement, judge_agent};
use repodesk_core::persistence::event_journal::{
    LogEventInput, format_events, log_event, read_events,
};
// use repodesk_core::knowledge::{append_knowledge, format_knowledge, read_knowledge};
use repodesk_core::module_registry::{
    audit_modules, format_module_audit, format_module_recommendations, format_modules,
    list_modules, recommend_modules,
};
use repodesk_core::peripherals::{
    audit_peripherals, ensure_peripherals_config, explain_peripheral, format_peripheral_audit,
    format_peripherals,
};
use repodesk_core::projects::{
    AddProjectInput, add_project, get_active_project, list_projects, use_project,
};
use repodesk_core::prompts::{PromptKind, generate_prompt};
use repodesk_core::safety::{
    format_safety_report, safety_rules_text, scan_active_context, scan_file,
};
use repodesk_core::security::{
    audit_security_policy, ensure_security_policy, explain_agent_security, format_security_audit,
    format_security_policy,
};
use repodesk_core::sessions::{
    begin_session, end_session, format_session_record, show_active_session,
};
use repodesk_core::tasks::{NewTaskInput, create_task, show_active_task, task_status};
use repodesk_core::tokens::{estimate_file, format_estimate};
use repodesk_core::ui_snapshot::{read_ui_snapshot_json, ui_routes_text, write_ui_snapshot};
use repodesk_core::usage::budget::{
    ensure_budget_config, evaluate_context, format_budget_config, format_verdict,
    load_budget_config,
};
use repodesk_core::usage::cost::{
    ensure_cost_config, estimate_agent_cost, format_cost_config, format_cost_estimate,
    format_cost_report,
};
use repodesk_core::usage::token_ledger::{
    LogTokenInput, format_token_report, log_token_event, read_token_report,
};
use repodesk_core::workflow::{create_workflow_plan, read_or_create_workflow_plan, workflow_next};
use repodesk_core::workflow_doctor::{diagnose_workflow, format_workflow_doctor_report};

use crate::cli::WorkflowCommand;

pub fn handle_workflow_command(command: WorkflowCommand) -> Result<()> {
    match command {
        WorkflowCommand::Next => {
            print!("{}", workflow_next()?);
        }
        WorkflowCommand::Plan => {
            let path = create_workflow_plan()?;
            println!("Workflow plan written:");
            println!("  file: {}", path.display());
        }
        WorkflowCommand::Show => {
            print!("{}", read_or_create_workflow_plan()?);
        }
        WorkflowCommand::Phase => {
            print!("{}", format_phase_progress(&cli_progress()));
        }
        WorkflowCommand::Review { accept, reject } => {
            use repodesk_core::orchestrator::{ReviewAction, record_review, review_run};
            let action = match (accept, reject) {
                (true, false) => ReviewAction::Accept,
                (false, true) => ReviewAction::Reject,
                _ => return Err(anyhow::anyhow!("pass exactly one of --accept or --reject")),
            };
            let run_id = repodesk_core::orchestrator::load_latest_run()?
                .filter(|run| !run.dry_run)
                .map(|run| run.run_id)
                .ok_or_else(|| anyhow::anyhow!("no orchestration run for the active task"))?;
            let review = review_run(&run_id, action)?;
            record_review(&run_id, action, &review)?;
            for file in &review.processed {
                println!("  {} — {}", file.path, file.outcome);
            }
            for warning in &review.warnings {
                println!("  ! {warning}");
            }
            println!();
            print!("{}", format_phase_progress(&cli_progress()));
        }
        WorkflowCommand::Verify => {
            let outcome = repodesk_core::workflow::run_verification()?;
            println!(
                "Verification: {}",
                if outcome.success { "passed" } else { "FAILED" }
            );
            for check in &outcome.commands {
                println!(
                    "  {} {}",
                    if check.success { "[x]" } else { "[ ]" },
                    check.command
                );
            }
            println!();
            print!("{}", format_phase_progress(&cli_progress()));
        }
    }

    Ok(())
}

/// Build the active task's phase progression from evidence — the same
/// [`repodesk_core::workflow::Evidence`] the desktop builds, so the CLI and the
/// Work tab can never report different phases.
fn cli_progress() -> repodesk_core::workflow::PhaseProgress {
    use repodesk_core::safety::SafetyLevel;
    use repodesk_core::workflow::{self, Evidence};

    let project = get_active_project().ok();
    let project_path = project.as_ref().map(|p| p.path.clone());
    let task = show_active_task().ok();
    let run_dir = task.as_ref().map(|t| t.config.run_dir.clone());
    let exists = |name: &str| {
        run_dir
            .as_ref()
            .map(|dir| dir.join(name).exists())
            .unwrap_or(false)
    };

    let receipt = workflow::load_receipt().ok().flatten();
    let head_sha = project_path.as_deref().and_then(workflow::head_sha);
    let index_tree_sha = project_path.as_deref().and_then(workflow::index_tree_sha);
    let finish_commit_exists = match (
        project_path.as_deref(),
        receipt.as_ref().and_then(|r| r.finish.as_ref()),
    ) {
        (Some(path), Some(finish)) => workflow::commit_exists(path, &finish.commit_sha),
        _ => false,
    };
    let mode = workflow::load_phase_state()
        .map(|state| state.execution_mode)
        .unwrap_or_default();

    let evidence = Evidence {
        project_ok: project.is_some(),
        task_ok: task.is_some(),
        goal_defined: task.is_some(),
        context_ok: exists("context.md"),
        safety_ok: scan_active_context()
            .map(|report| report.level != SafetyLevel::Block)
            .unwrap_or(true),
        route_ready: exists("smart-context.md"),
        cost_estimated: exists("token-estimate.txt"),
        baseline_checks_ran: exists("checks-summary.md") && receipt.is_none(),
        mode,
        receipt,
        head_sha,
        index_tree_sha,
        finish_commit_exists,
    };
    workflow::derive_progress(&workflow::derive_signals(&evidence), mode)
}

fn format_phase_progress(progress: &repodesk_core::workflow::PhaseProgress) -> String {
    use repodesk_core::workflow::PhaseStatus;
    let mut out = String::from("Task phases:\n");
    for view in &progress.phases {
        let marker = match view.status {
            PhaseStatus::Done => "[x]",
            PhaseStatus::InProgress => "[~]",
            PhaseStatus::Available => "[>]",
            PhaseStatus::Locked => "[ ]",
        };
        let cursor = if view.phase == progress.current {
            " ←"
        } else {
            ""
        };
        out.push_str(&format!(
            "  {marker} {} — {}{cursor}\n",
            view.title, view.summary
        ));
    }
    out.push_str(&format!("\nNext: {}\n", progress.cta.label));
    if progress.complete {
        out.push_str("All phases complete.\n");
    }
    out
}
