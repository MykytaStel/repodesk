use crate::cli::{Cli, Command};
use anyhow::Result;

pub mod access;
pub mod agents;
pub mod ai;
pub mod brain;
pub mod budget;
pub mod capabilities;
pub mod checks;
pub mod context;
pub mod cost;
pub mod dashboard;
pub mod desktop;
pub mod doctor;
pub mod events;
pub mod git;
pub mod guard;
pub mod init;
pub mod inspect;
pub mod judge;
pub mod memory;
pub mod modules;
pub mod orchestrate;
pub mod outcomes;
pub mod peripherals;
pub mod project;
pub mod prompt;
pub mod receipts;
pub mod runtime;
pub mod safety;
pub mod sandbox;
pub mod security;
pub mod session;
pub mod smart_context;
pub mod task;
pub mod tokens;
pub mod ui;
pub mod workflow;
pub mod ci;

pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init => init::handle_init(),
        Command::Project { command } => project::handle_project_command(command),
        Command::Task { command } => task::handle_task_command(command),
        Command::Context { command } => context::handle_context_command(command),
        Command::Prompt { command } => prompt::handle_prompt_command(command),
        Command::Checks { command } => checks::handle_checks_command(command),
        Command::Agents { command } => agents::handle_agents_command(command),
        Command::Guard { command } => guard::handle_guard_command(command),
        Command::Brain { command } => brain::handle_brain_command(command),
        Command::Capabilities { command } => capabilities::handle_capabilities_command(command),
        Command::Security { command } => security::handle_security_command(command),
        Command::Doctor { command } => doctor::handle_doctor_command(command),
        Command::Ai { command } => ai::handle_ai_command(command),
        Command::Ui { command } => ui::handle_ui_command(command),
        Command::Desktop { command } => desktop::handle_desktop_command(command),
        Command::Cost { command } => cost::handle_cost_command(command),
        Command::Safety { command } => safety::handle_safety_command(command),
        Command::Peripherals { command } => peripherals::handle_peripherals_command(command),
        Command::Workflow { command } => workflow::handle_workflow_command(command),
        Command::Events { command } => events::handle_events_command(command),
        Command::Judge { command } => judge::handle_judge_command(command),
        Command::Access { command } => access::handle_access_command(command),
        Command::Modules { command } => modules::handle_modules_command(command),
        Command::Session { command } => session::handle_session_command(command),
        Command::Inspect { command } => inspect::handle_inspect_command(command),
        Command::SmartContext { command } => smart_context::handle_smart_context_command(command),
        Command::Receipts { command } => receipts::handle_receipts_command(command),
        Command::Git { command } => git::handle_git_command(command),
        Command::Dashboard { command } => dashboard::handle_dashboard_command(command),
        Command::Runtime { command } => runtime::handle_runtime_command(command),
        Command::Sandbox { command } => sandbox::handle_sandbox_command(command),
        Command::Tokens { command } => tokens::handle_tokens_command(command),
        Command::Budget { command } => budget::handle_budget_command(command),
        Command::Memory { command } => memory::handle_memory_command(command),
        Command::Orchestrate { command } => orchestrate::handle_orchestrate_command(command),
        Command::Outcomes { command } => outcomes::handle_outcomes_command(command),
        Command::Ci { command } => ci::handle_ci_command(command),
    }
}
