use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use repodesk_core::access::{evaluate_access, format_access_matrix, format_access_report};
use repodesk_core::agents::{ensure_agents_config, format_agents, recommend_agents};
use repodesk_core::ai_adapters::{
    check_ai_status, ensure_ai_adapters_config, format_ai_adapters, format_ai_recommendations,
    format_ai_status, recommend_ai_adapters,
};
use repodesk_core::brain::{format_brain_status, read_brain_status};
use repodesk_core::budget::{
    ensure_budget_config, evaluate_context, format_budget_config, format_verdict,
    load_budget_config,
};
use repodesk_core::capabilities::{
    audit_capabilities, ensure_capabilities_config, format_capabilities, format_capability_audit,
    format_capability_recommendations, recommend_capabilities,
};
use repodesk_core::checks::{last_checks, run_checks, summarize_last_checks};
use repodesk_core::context::{build_context, estimate_active_context};
use repodesk_core::cost::{
    ensure_cost_config, estimate_agent_cost, format_cost_config, format_cost_estimate,
    format_cost_report,
};
use repodesk_core::desktop::{
    desktop_events_spec, desktop_plan, desktop_scaffold_hint, tauri_bridge_spec,
};
use repodesk_core::event_journal::{format_events, log_event, read_events, LogEventInput};
use repodesk_core::guard::{format_guard_result, preflight};
use repodesk_core::init;
use repodesk_core::judge::{format_judgement, judge_agent};
use repodesk_core::knowledge::{append_knowledge, format_knowledge, read_knowledge};
use repodesk_core::module_registry::{
    audit_modules, format_module_audit, format_module_recommendations, format_modules,
    list_modules, recommend_modules,
};
use repodesk_core::peripherals::{
    audit_peripherals, ensure_peripherals_config, explain_peripheral, format_peripheral_audit,
    format_peripherals,
};
use repodesk_core::projects::{
    add_project, get_active_project, list_projects, use_project, AddProjectInput,
};
use repodesk_core::prompts::{generate_prompt, PromptKind};
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
use repodesk_core::tasks::{create_task, show_active_task, task_status, NewTaskInput};
use repodesk_core::token_ledger::{
    format_token_report, log_token_event, read_token_report, LogTokenInput,
};
use repodesk_core::tokens::{estimate_file, format_estimate};
use repodesk_core::ui_snapshot::{read_ui_snapshot_json, ui_routes_text, write_ui_snapshot};
use repodesk_core::workflow::{create_workflow_plan, read_or_create_workflow_plan, workflow_next};
use repodesk_core::workflow_doctor::{diagnose_workflow, format_workflow_doctor_report};

#[derive(Debug, Parser)]
#[command(name = "repodesk")]
#[command(about = "Personal local AI operations hub for development workflows")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    Prompt {
        #[command(subcommand)]
        command: PromptCommand,
    },
    Checks {
        #[command(subcommand)]
        command: ChecksCommand,
    },
    Agents {
        #[command(subcommand)]
        command: AgentsCommand,
    },
    Guard {
        #[command(subcommand)]
        command: GuardCommand,
    },
    Brain {
        #[command(subcommand)]
        command: BrainCommand,
    },
    Capabilities {
        #[command(subcommand)]
        command: CapabilitiesCommand,
    },
    Security {
        #[command(subcommand)]
        command: SecurityCommand,
    },
    Doctor {
        #[command(subcommand)]
        command: DoctorCommand,
    },
    Ai {
        #[command(subcommand)]
        command: AiCommand,
    },
    Ui {
        #[command(subcommand)]
        command: UiCommand,
    },
    Desktop {
        #[command(subcommand)]
        command: DesktopCommand,
    },
    Cost {
        #[command(subcommand)]
        command: CostCommand,
    },
    Safety {
        #[command(subcommand)]
        command: SafetyCommand,
    },
    Peripherals {
        #[command(subcommand)]
        command: PeripheralsCommand,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
    Knowledge {
        #[command(subcommand)]
        command: KnowledgeCommand,
    },
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
    Judge {
        #[command(subcommand)]
        command: JudgeCommand,
    },
    Access {
        #[command(subcommand)]
        command: AccessCommand,
    },
    Modules {
        #[command(subcommand)]
        command: ModulesCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Tokens {
        #[command(subcommand)]
        command: TokensCommand,
    },
    Budget {
        #[command(subcommand)]
        command: BudgetCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Add {
        name: String,
        path: PathBuf,
        #[arg(long = "type")]
        project_type: String,
        #[arg(long)]
        main_language: Option<String>,
    },
    List,
    Use {
        name: String,
    },
    Info,
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    New { title: String },
    Show,
    Status,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    Build,
    Estimate,
}

#[derive(Debug, Subcommand)]
enum PromptCommand {
    Codex,
    Chatgpt,
    Review,
    All,
}

#[derive(Debug, Subcommand)]
enum ChecksCommand {
    Run,
    Last,
    Summarize,
}

#[derive(Debug, Subcommand)]
enum AgentsCommand {
    Show,
    Recommend {
        #[arg(long)]
        category: String,
    },
}

#[derive(Debug, Subcommand)]
enum GuardCommand {
    Preflight {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
enum BrainCommand {
    Status,
}

#[derive(Debug, Subcommand)]
enum CapabilitiesCommand {
    Show,
    Recommend {
        #[arg(long)]
        need: String,
    },
    Audit,
}

#[derive(Debug, Subcommand)]
enum SecurityCommand {
    Show,
    Audit,
    Explain {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
enum DoctorCommand {
    Workflow,
}

#[derive(Debug, Subcommand)]
enum AiCommand {
    Adapters,
    Recommend {
        #[arg(long)]
        need: String,
    },
    Status {
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum UiCommand {
    Snapshot,
    SnapshotJson,
    Routes,
}

#[derive(Debug, Subcommand)]
enum DesktopCommand {
    Plan,
    BridgeSpec,
    Events,
    ScaffoldHint,
}

#[derive(Debug, Subcommand)]
enum CostCommand {
    Show,
    Estimate {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        input: usize,
        #[arg(long, default_value_t = 0)]
        output: usize,
    },
    Report,
}

#[derive(Debug, Subcommand)]
enum SafetyCommand {
    ScanContext,
    ScanFile { file: PathBuf },
    Rules,
}

#[derive(Debug, Subcommand)]
enum PeripheralsCommand {
    Show,
    Audit,
    Explain {
        #[arg(long)]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    Next,
    Plan,
    Show,
}

#[derive(Debug, Subcommand)]
enum KnowledgeCommand {
    Show {
        #[arg(long, default_value = "memory")]
        kind: String,
    },
    Add {
        #[arg(long, default_value = "memory")]
        kind: String,
        #[arg(long)]
        text: String,
    },
}

#[derive(Debug, Subcommand)]
enum EventsCommand {
    Last {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Log {
        #[arg(long = "module")]
        module_name: String,
        #[arg(long, default_value = "info")]
        level: String,
        #[arg(long)]
        message: String,
    },
}

#[derive(Debug, Subcommand)]
enum JudgeCommand {
    Agent {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
enum AccessCommand {
    Matrix,
    Check {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        peripheral: String,
    },
}

#[derive(Debug, Subcommand)]
enum ModulesCommand {
    Show,
    Audit,
    Recommend {
        #[arg(long)]
        need: String,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    Begin {
        #[arg(long)]
        name: String,
    },
    Show,
    End,
}

#[derive(Debug, Subcommand)]
enum TokensCommand {
    Estimate {
        file: PathBuf,
    },
    Log {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        input: usize,
        #[arg(long, default_value_t = 0)]
        output: usize,
        #[arg(long, default_value = "general")]
        category: String,
        #[arg(long)]
        notes: Option<String>,
    },
    Report,
}

#[derive(Debug, Subcommand)]
enum BudgetCommand {
    Show,
    CheckContext,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => handle_init()?,
        Command::Project { command } => handle_project_command(command)?,
        Command::Task { command } => handle_task_command(command)?,
        Command::Context { command } => handle_context_command(command)?,
        Command::Prompt { command } => handle_prompt_command(command)?,
        Command::Checks { command } => handle_checks_command(command)?,
        Command::Agents { command } => handle_agents_command(command)?,
        Command::Guard { command } => handle_guard_command(command)?,
        Command::Brain { command } => handle_brain_command(command)?,
        Command::Capabilities { command } => handle_capabilities_command(command)?,
        Command::Security { command } => handle_security_command(command)?,
        Command::Doctor { command } => handle_doctor_command(command)?,
        Command::Ai { command } => handle_ai_command(command)?,
        Command::Ui { command } => handle_ui_command(command)?,
        Command::Desktop { command } => handle_desktop_command(command)?,
        Command::Cost { command } => handle_cost_command(command)?,
        Command::Safety { command } => handle_safety_command(command)?,
        Command::Peripherals { command } => handle_peripherals_command(command)?,
        Command::Workflow { command } => handle_workflow_command(command)?,
        Command::Knowledge { command } => handle_knowledge_command(command)?,
        Command::Events { command } => handle_events_command(command)?,
        Command::Judge { command } => handle_judge_command(command)?,
        Command::Access { command } => handle_access_command(command)?,
        Command::Modules { command } => handle_modules_command(command)?,
        Command::Session { command } => handle_session_command(command)?,
        Command::Tokens { command } => handle_tokens_command(command)?,
        Command::Budget { command } => handle_budget_command(command)?,
    }

    Ok(())
}

fn handle_init() -> Result<()> {
    let result = init::init_home()?;
    ensure_budget_config()?;
    ensure_agents_config()?;
    ensure_capabilities_config()?;
    ensure_security_policy()?;
    ensure_ai_adapters_config()?;
    ensure_cost_config()?;
    ensure_peripherals_config()?;

    println!("RepoDesk initialized at {}", result.home);

    if result.created_dirs.is_empty() {
        println!("No new directories were created.");
    } else {
        println!("Created directories:");
        for dir in result.created_dirs {
            println!("  - {dir}");
        }
    }

    Ok(())
}

fn handle_project_command(command: ProjectCommand) -> Result<()> {
    match command {
        ProjectCommand::Add {
            name,
            path,
            project_type,
            main_language,
        } => {
            let config = add_project(AddProjectInput {
                name,
                path,
                project_type,
                main_language,
            })?;

            println!("Project added:");
            println!("  name: {}", config.name);
            println!("  path: {}", config.path.display());
            println!("  type: {}", config.project_type);

            if let Some(language) = config.main_language {
                println!("  main language: {language}");
            }

            if config.checks.is_empty() {
                println!("  checks: none configured yet");
            } else {
                println!("  checks:");
                for check in config.checks {
                    println!("    - {check}");
                }
            }
        }
        ProjectCommand::List => {
            let projects = list_projects()?;

            if projects.is_empty() {
                println!("No projects registered yet.");
                println!("Add one with:");
                println!(
                    "  repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli"
                );
            } else {
                println!("Registered projects:");

                for project in projects {
                    let marker = if project.is_active { "*" } else { " " };

                    println!(
                        "{} {} ({})",
                        marker, project.config.name, project.config.project_type
                    );
                    println!("    path: {}", project.config.path.display());
                }
            }
        }
        ProjectCommand::Use { name } => {
            let config = use_project(&name)?;
            println!("Active project set to '{}'", config.name);
            println!("Path: {}", config.path.display());
        }
        ProjectCommand::Info => {
            let config = get_active_project()?;

            println!("Active project:");
            println!("  name: {}", config.name);
            println!("  path: {}", config.path.display());
            println!("  type: {}", config.project_type);

            if let Some(language) = config.main_language {
                println!("  main language: {language}");
            }

            if config.checks.is_empty() {
                println!("  checks: none configured");
            } else {
                println!("  checks:");
                for check in config.checks {
                    println!("    - {check}");
                }
            }

            if config.context_ignore.is_empty() {
                println!("  context ignore: none configured");
            } else {
                println!("  context ignore:");
                for item in config.context_ignore {
                    println!("    - {item}");
                }
            }
        }
    }

    Ok(())
}

fn handle_task_command(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::New { title } => {
            let task = create_task(NewTaskInput { title })?;

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

fn handle_context_command(command: ContextCommand) -> Result<()> {
    match command {
        ContextCommand::Build => {
            let result = build_context()?;
            let config = load_budget_config()?;
            let verdict = evaluate_context(&result.estimate, &config);

            println!("Context built:");
            println!("  context file: {}", result.context_file);
            println!("  token estimate file: {}", result.token_estimate_file);
            println!();
            print!("{}", format_estimate(&result.estimate));
            println!("{}", format_verdict(&verdict));
        }
        ContextCommand::Estimate => {
            let (context_file, estimate) = estimate_active_context()?;
            let config = load_budget_config()?;
            let verdict = evaluate_context(&estimate, &config);

            println!("Context estimate:");
            println!("  file: {context_file}");
            println!();
            print!("{}", format_estimate(&estimate));
            println!("{}", format_verdict(&verdict));
        }
    }

    Ok(())
}

fn handle_prompt_command(command: PromptCommand) -> Result<()> {
    match command {
        PromptCommand::Codex => print_prompt_result(generate_prompt(PromptKind::Codex)?),
        PromptCommand::Chatgpt => print_prompt_result(generate_prompt(PromptKind::ChatGpt)?),
        PromptCommand::Review => print_prompt_result(generate_prompt(PromptKind::Review)?),
        PromptCommand::All => {
            print_prompt_result(generate_prompt(PromptKind::Codex)?);
            print_prompt_result(generate_prompt(PromptKind::ChatGpt)?);
            print_prompt_result(generate_prompt(PromptKind::Review)?);
        }
    }

    Ok(())
}

fn handle_checks_command(command: ChecksCommand) -> Result<()> {
    match command {
        ChecksCommand::Run => {
            let result = run_checks()?;

            println!("Checks finished:");
            println!(
                "  status: {}",
                if result.success { "passed" } else { "failed" }
            );
            println!("  log: {}", result.log_file.display());
            println!("  summary: {}", result.summary_file.display());
            println!();

            for command in result.commands {
                println!(
                    "  - {} => {} ({:?})",
                    command.command, command.status, command.exit_code
                );
            }
        }
        ChecksCommand::Last => {
            let result = last_checks()?;
            println!("Last checks:");
            println!("  log: {}", result.log_file.display());
            println!("  summary: {}", result.summary_file.display());
            println!();
            print!("{}", result.summary);
        }
        ChecksCommand::Summarize => {
            let result = summarize_last_checks()?;
            println!("Checks summary updated:");
            println!("  log: {}", result.log_file.display());
            println!("  summary: {}", result.summary_file.display());
        }
    }

    Ok(())
}

fn handle_agents_command(command: AgentsCommand) -> Result<()> {
    match command {
        AgentsCommand::Show => {
            let config = ensure_agents_config()?;
            print!("{}", format_agents(&config));
        }
        AgentsCommand::Recommend { category } => {
            let agents = recommend_agents(&category)?;
            println!("Recommended agents for '{}':", category);
            println!();
            for agent in agents {
                println!("- {} ({})", agent.name, agent.kind);
                println!("  role: {}", agent.role);
                println!("  budget: {} tokens", agent.default_budget_tokens);
                println!("  preferred for: {}", agent.preferred_for.join(", "));
                println!();
            }
        }
    }

    Ok(())
}

fn handle_guard_command(command: GuardCommand) -> Result<()> {
    match command {
        GuardCommand::Preflight { agent } => {
            let result = preflight(&agent)?;
            print!("{}", format_guard_result(&result));
        }
    }

    Ok(())
}

fn handle_brain_command(command: BrainCommand) -> Result<()> {
    match command {
        BrainCommand::Status => {
            let status = read_brain_status()?;
            print!("{}", format_brain_status(&status));
        }
    }

    Ok(())
}

fn handle_capabilities_command(command: CapabilitiesCommand) -> Result<()> {
    match command {
        CapabilitiesCommand::Show => {
            let config = ensure_capabilities_config()?;
            print!("{}", format_capabilities(&config));
        }
        CapabilitiesCommand::Recommend { need } => {
            let capabilities = recommend_capabilities(&need)?;
            print!(
                "{}",
                format_capability_recommendations(&need, &capabilities)
            );
        }
        CapabilitiesCommand::Audit => {
            let audit = audit_capabilities()?;
            print!("{}", format_capability_audit(&audit));
        }
    }

    Ok(())
}

fn handle_security_command(command: SecurityCommand) -> Result<()> {
    match command {
        SecurityCommand::Show => {
            let policy = ensure_security_policy()?;
            print!("{}", format_security_policy(&policy));
        }
        SecurityCommand::Audit => {
            let audit = audit_security_policy()?;
            print!("{}", format_security_audit(&audit));
        }
        SecurityCommand::Explain { agent } => {
            let explanation = explain_agent_security(&agent)?;
            print!("{}", explanation);
        }
    }

    Ok(())
}

fn handle_doctor_command(command: DoctorCommand) -> Result<()> {
    match command {
        DoctorCommand::Workflow => {
            let report = diagnose_workflow()?;
            print!("{}", format_workflow_doctor_report(&report));
        }
    }

    Ok(())
}

fn handle_ai_command(command: AiCommand) -> Result<()> {
    match command {
        AiCommand::Adapters => {
            let config = ensure_ai_adapters_config()?;
            print!("{}", format_ai_adapters(&config));
        }
        AiCommand::Recommend { need } => {
            let adapters = recommend_ai_adapters(&need)?;
            print!("{}", format_ai_recommendations(&need, &adapters));
        }
        AiCommand::Status { name } => {
            let statuses = check_ai_status(name.as_deref())?;
            print!("{}", format_ai_status(&statuses));
        }
    }

    Ok(())
}

fn handle_ui_command(command: UiCommand) -> Result<()> {
    match command {
        UiCommand::Snapshot => {
            let path = write_ui_snapshot()?;
            println!("UI snapshot written:");
            println!("  file: {}", path.display());
        }
        UiCommand::SnapshotJson => {
            let json = read_ui_snapshot_json()?;
            println!("{}", json);
        }
        UiCommand::Routes => {
            print!("{}", ui_routes_text());
        }
    }

    Ok(())
}

fn handle_desktop_command(command: DesktopCommand) -> Result<()> {
    match command {
        DesktopCommand::Plan => print!("{}", desktop_plan()),
        DesktopCommand::BridgeSpec => print!("{}", tauri_bridge_spec()),
        DesktopCommand::Events => print!("{}", desktop_events_spec()),
        DesktopCommand::ScaffoldHint => print!("{}", desktop_scaffold_hint()?),
    }

    Ok(())
}

fn handle_cost_command(command: CostCommand) -> Result<()> {
    match command {
        CostCommand::Show => {
            let config = ensure_cost_config()?;
            print!("{}", format_cost_config(&config));
        }
        CostCommand::Estimate {
            agent,
            input,
            output,
        } => {
            let config = ensure_cost_config()?;
            let estimate = estimate_agent_cost(&config, &agent, input, output);
            print!("{}", format_cost_estimate(&estimate));
        }
        CostCommand::Report => {
            let config = ensure_cost_config()?;
            let report = read_token_report()?;
            print!("{}", format_cost_report(&config, &report));
        }
    }

    Ok(())
}

fn handle_safety_command(command: SafetyCommand) -> Result<()> {
    match command {
        SafetyCommand::ScanContext => {
            let report = scan_active_context()?;
            print!("{}", format_safety_report(&report));
        }
        SafetyCommand::ScanFile { file } => {
            let report = scan_file(&file)?;
            print!("{}", format_safety_report(&report));
        }
        SafetyCommand::Rules => {
            print!("{}", safety_rules_text());
        }
    }

    Ok(())
}

fn handle_peripherals_command(command: PeripheralsCommand) -> Result<()> {
    match command {
        PeripheralsCommand::Show => {
            let config = ensure_peripherals_config()?;
            print!("{}", format_peripherals(&config));
        }
        PeripheralsCommand::Audit => {
            let report = audit_peripherals()?;
            print!("{}", format_peripheral_audit(&report));
        }
        PeripheralsCommand::Explain { name } => {
            let config = ensure_peripherals_config()?;
            print!("{}", explain_peripheral(&config, &name));
        }
    }

    Ok(())
}

fn handle_workflow_command(command: WorkflowCommand) -> Result<()> {
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
    }

    Ok(())
}

fn handle_knowledge_command(command: KnowledgeCommand) -> Result<()> {
    match command {
        KnowledgeCommand::Show { kind } => {
            let content = read_knowledge(&kind)?;
            print!("{}", format_knowledge(&kind, &content));
        }
        KnowledgeCommand::Add { kind, text } => {
            let path = append_knowledge(&kind, &text)?;
            println!("Knowledge updated:");
            println!("  kind: {}", kind);
            println!("  file: {}", path.display());
        }
    }

    Ok(())
}

fn handle_events_command(command: EventsCommand) -> Result<()> {
    match command {
        EventsCommand::Last { limit } => {
            let events = read_events(limit)?;
            print!("{}", format_events(&events));
        }
        EventsCommand::Log {
            module_name,
            level,
            message,
        } => {
            let file = log_event(LogEventInput {
                module_name,
                level,
                message,
                metadata: Vec::new(),
            })?;

            println!("Event logged.");
            println!("Journal: {}", file.display());
        }
    }

    Ok(())
}

fn handle_judge_command(command: JudgeCommand) -> Result<()> {
    match command {
        JudgeCommand::Agent { agent } => {
            let judgement = judge_agent(&agent)?;
            print!("{}", format_judgement(&judgement));
        }
    }

    Ok(())
}

fn handle_access_command(command: AccessCommand) -> Result<()> {
    match command {
        AccessCommand::Matrix => {
            print!("{}", format_access_matrix());
        }
        AccessCommand::Check { agent, peripheral } => {
            let report = evaluate_access(&agent, &peripheral);
            print!("{}", format_access_report(&report));
        }
    }

    Ok(())
}

fn handle_modules_command(command: ModulesCommand) -> Result<()> {
    match command {
        ModulesCommand::Show => {
            let modules = list_modules();
            print!("{}", format_modules(&modules));
        }
        ModulesCommand::Audit => {
            let audit = audit_modules();
            print!("{}", format_module_audit(&audit));
        }
        ModulesCommand::Recommend { need } => {
            let recommendations = recommend_modules(&need);
            print!("{}", format_module_recommendations(&need, &recommendations));
        }
    }

    Ok(())
}

fn handle_session_command(command: SessionCommand) -> Result<()> {
    match command {
        SessionCommand::Begin { name } => {
            let session = begin_session(&name)?;
            print!("{}", format_session_record(&session));
        }
        SessionCommand::Show => {
            let session = show_active_session()?;
            print!("{}", format_session_record(&session));
        }
        SessionCommand::End => {
            let session = end_session()?;
            print!("{}", format_session_record(&session));
        }
    }

    Ok(())
}

fn handle_tokens_command(command: TokensCommand) -> Result<()> {
    match command {
        TokensCommand::Estimate { file } => {
            let estimate = estimate_file(&file)?;
            println!("Token estimate:");
            println!("  file: {}", file.display());
            println!();
            print!("{}", format_estimate(&estimate));
        }
        TokensCommand::Log {
            agent,
            input,
            output,
            category,
            notes,
        } => {
            let ledger_file = log_token_event(LogTokenInput {
                agent,
                input_tokens: input,
                output_tokens: output,
                category,
                notes,
            })?;

            println!("Token usage logged.");
            println!("Ledger: {ledger_file}");
        }
        TokensCommand::Report => {
            let report = read_token_report()?;
            print!("{}", format_token_report(&report));
        }
    }

    Ok(())
}

fn handle_budget_command(command: BudgetCommand) -> Result<()> {
    match command {
        BudgetCommand::Show => {
            let config = ensure_budget_config()?;
            print!("{}", format_budget_config(&config));
        }
        BudgetCommand::CheckContext => {
            let (context_file, estimate) = estimate_active_context()?;
            let config = load_budget_config()?;
            let verdict = evaluate_context(&estimate, &config);

            println!("Budget check:");
            println!("  context: {context_file}");
            println!();
            print!("{}", format_estimate(&estimate));
            println!("{}", format_verdict(&verdict));
        }
    }

    Ok(())
}

fn print_prompt_result(result: repodesk_core::prompts::PromptBuildResult) {
    println!("Prompt generated:");
    println!("  kind: {}", result.prompt_kind.label());
    println!("  file: {}", result.prompt_file.display());
    println!("  context tokens: {}", result.estimated_tokens);
}

fn print_task_info(label: &str, task: &repodesk_core::tasks::TaskInfo) {
    println!("{label}:");
    println!("  id: {}", task.config.id);
    println!("  project: {}", task.config.project_name);
    println!("  title: {}", task.config.title);
    println!("  status: {:?}", task.config.status);
    println!("  run dir: {}", task.config.run_dir.display());
    println!("  task config: {}", task.task_file.display());
    println!("  task markdown: {}", task.task_markdown_file.display());
}
