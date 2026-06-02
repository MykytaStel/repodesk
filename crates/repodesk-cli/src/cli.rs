use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "repodesk")]
#[command(about = "Personal local AI operations hub for development workflows")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
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
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
    SmartContext {
        #[command(subcommand)]
        command: SmartContextCommand,
    },
    Receipts {
        #[command(subcommand)]
        command: ReceiptsCommand,
    },
    Git {
        #[command(subcommand)]
        command: GitCommand,
    },
    Dashboard {
        #[command(subcommand)]
        command: DashboardCommand,
    },
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommand,
    },
    Tokens {
        #[command(subcommand)]
        command: TokensCommand,
    },
    Budget {
        #[command(subcommand)]
        command: BudgetCommand,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
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
pub enum TaskCommand {
    New { title: String },
    Show,
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    Build,
    Estimate,
}

#[derive(Debug, Subcommand)]
pub enum PromptCommand {
    Codex,
    Chatgpt,
    Review,
    All,
}

#[derive(Debug, Subcommand)]
pub enum ChecksCommand {
    Run,
    Last,
    Summarize,
}

#[derive(Debug, Subcommand)]
pub enum AgentsCommand {
    Show,
    Recommend {
        #[arg(long)]
        category: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GuardCommand {
    Preflight {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum BrainCommand {
    Status,
}

#[derive(Debug, Subcommand)]
pub enum CapabilitiesCommand {
    Show,
    Recommend {
        #[arg(long)]
        need: String,
    },
    Audit,
}

#[derive(Debug, Subcommand)]
pub enum SecurityCommand {
    Show,
    Audit,
    Explain {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum DoctorCommand {
    Workflow,
}

#[derive(Debug, Subcommand)]
pub enum AiCommand {
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
pub enum UiCommand {
    Snapshot,
    SnapshotJson,
    Routes,
}

#[derive(Debug, Subcommand)]
pub enum DesktopCommand {
    Plan,
    BridgeSpec,
    Events,
    ScaffoldHint,
}

#[derive(Debug, Subcommand)]
pub enum CostCommand {
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
pub enum SafetyCommand {
    ScanContext,
    ScanFile { file: PathBuf },
    Rules,
}

#[derive(Debug, Subcommand)]
pub enum PeripheralsCommand {
    Show,
    Audit,
    Explain {
        #[arg(long)]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkflowCommand {
    Next,
    Plan,
    Show,
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
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
pub enum JudgeCommand {
    Agent {
        #[arg(long, default_value = "codex")]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum AccessCommand {
    Matrix,
    Check {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        peripheral: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModulesCommand {
    Show,
    Audit,
    Recommend {
        #[arg(long)]
        need: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    Begin {
        #[arg(long)]
        name: String,
    },
    Show,
    End,
}

#[derive(Debug, Subcommand)]
pub enum InspectCommand {
    Repo,
    Hotspots,
}

#[derive(Debug, Subcommand)]
pub enum SmartContextCommand {
    Build,
    Sources,
}

#[derive(Debug, Subcommand)]
pub enum ReceiptsCommand {
    Add {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = "unknown")]
        outcome: String,
        #[arg(long)]
        summary: String,
    },
    Last,
}

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    Audit,
    BackupPlan,
}

#[derive(Debug, Subcommand)]
pub enum DashboardCommand {
    Summary,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum RuntimeCommand {
    Providers,
    Status {
        #[arg(long)]
        provider: String,
    },
    Route {
        #[arg(long)]
        need: String,
    },
    SnapshotJson,
}

#[derive(Debug, Subcommand)]
pub enum SandboxCommand {
    Policy,
    Plan {
        #[arg(long)]
        command: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TokensCommand {
    Estimate {
        file: PathBuf,
    },
    Log {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        model: Option<String>,
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
    Compare {
        #[arg(required = true, num_args = 1..)]
        snippets: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum BudgetCommand {
    Show,
    CheckContext,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    Add {
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        content: String,
        #[arg(long)]
        category: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    List {
        #[arg(long)]
        project: Option<String>,
    },
    Consolidate {
        #[arg(long)]
        project: Option<String>,
    },
}
