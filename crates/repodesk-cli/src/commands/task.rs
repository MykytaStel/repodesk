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

use crate::cli::TaskCommand;

pub fn handle_task_command(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::New { title } => {
            let task = create_task(NewTaskInput {
                title,
                verify_command: None,
            })?;

            println!("Task created and set as active:");
            println!("  id: {}", task.config.id);
            println!("  project: {}", task.config.project_name);
            println!("  title: {}", task.config.title);
            println!("  run dir: {}", task.config.run_dir.display());
            println!("  task file: {}", task.task_markdown_file.display());
        }
        TaskCommand::Show => {
            let task = show_active_task()?;
            print_task_info("Active task", &task);
        }
        TaskCommand::Status => {
            let task = task_status()?;
            print_task_info("Task status", &task);
        }
    }

    Ok(())
}

pub fn print_task_info(label: &str, task: &repodesk_core::tasks::TaskInfo) {
    println!("{label}:");
    println!("  id: {}", task.config.id);
    println!("  project: {}", task.config.project_name);
    println!("  title: {}", task.config.title);
    println!("  status: {:?}", task.config.status);
    println!("  run dir: {}", task.config.run_dir.display());
    println!("  task config: {}", task.task_file.display());
    println!("  task markdown: {}", task.task_markdown_file.display());
}
