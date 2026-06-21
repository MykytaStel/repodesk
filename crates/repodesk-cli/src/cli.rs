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
    Orchestrate {
        #[command(subcommand)]
        command: OrchestrateCommand,
    },
    Outcomes {
        #[command(subcommand)]
        command: OutcomesCommand,
    },
    Ci {
        #[command(subcommand)]
        command: CiCommand,
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
    /// Register a check command on the active project
    Add {
        /// The check command, e.g. `repopilot review . --fail-on new-high`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
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
    /// Print the six-phase progression (Scope → … → Finish) for the active task.
    Phase {
        /// Record a transition for the latest run: `reviewed` or `committed`.
        #[arg(long)]
        mark: Option<String>,
    },
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
    /// Search entries by content/category/tag substring.
    Search {
        #[arg(long)]
        project: Option<String>,
        query: String,
    },
    Consolidate {
        #[arg(long)]
        project: Option<String>,
    },
    /// Capture memory candidates from an AI response (creates proposals).
    Capture {
        #[arg(long)]
        project: Option<String>,
        /// Which AI produced the text (codex, chatgpt, gemini, ollama, …).
        #[arg(long)]
        agent: String,
        /// Read the response from a file (otherwise --content or stdin).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Inline response text.
        #[arg(long)]
        content: Option<String>,
    },
    /// Scan active memory for duplicates / merges / conflicts (creates proposals).
    Scan {
        #[arg(long)]
        project: Option<String>,
    },
    /// List proposals awaiting review (pending by default).
    Review {
        #[arg(long)]
        project: Option<String>,
        /// Include accepted/rejected proposals too.
        #[arg(long)]
        all: bool,
    },
    /// Accept a proposal, applying it to the brain.
    Accept {
        id: i64,
        /// For dedup/conflict: the entry id to keep.
        #[arg(long)]
        keep: Option<i64>,
    },
    /// Reject a proposal (brain left unchanged).
    Reject {
        id: i64,
    },
    /// Pin an entry so it is always included in context.
    Pin {
        id: i64,
    },
    Unpin {
        id: i64,
    },
    /// Archive an entry (kept, but excluded from context).
    Archive {
        id: i64,
    },
    Delete {
        id: i64,
    },
}

#[derive(Debug, Subcommand)]
pub enum OrchestrateCommand {
    /// Show the sub-agent plan for the active task (routes each step; no execution).
    Plan {
        #[arg(long)]
        goal: Option<String>,
    },
    /// Execute the plan: each sub-agent runs on its own context, routed model.
    Run {
        #[arg(long)]
        goal: Option<String>,
        /// Preview only — route and gate every step, but make no provider calls.
        #[arg(long)]
        dry_run: bool,
        /// Cost ceiling in cost units; halts before a step that would exceed it.
        #[arg(long = "max-cost")]
        max_cost: Option<f64>,
        /// Confirm execution of paid provider and coding-agent CLI steps.
        #[arg(long)]
        yes: bool,
        /// Deprecated no-op: coding-agent steps now always require isolated workspaces.
        #[arg(long, hide = true)]
        worktree: bool,
    },
    /// Autonomously attempt the task: plan → run → re-plan/retry under guardrails.
    Loop {
        #[arg(long)]
        goal: Option<String>,
        /// Maximum attempts before giving up.
        #[arg(long = "max-iterations", default_value_t = 3)]
        max_iterations: usize,
        /// Total cost ceiling across all attempts (cost units).
        #[arg(long = "max-cost")]
        max_cost: Option<f64>,
        /// Preview only — a single pass, no provider calls.
        #[arg(long)]
        dry_run: bool,
        /// Approve execution of paid provider and coding-agent CLI steps.
        #[arg(long)]
        yes: bool,
    },
    /// Show the most recent orchestration run for the active task.
    Status,
    /// Show a specific run by id.
    Show { run_id: String },
    /// Accept or reject a coding-agent run's changed files.
    Review {
        run_id: String,
        /// `accept` to stage/apply the changes, `reject` to discard or leave them isolated.
        action: String,
    },
    /// List RepoDesk-managed isolated worktrees for the active task.
    Worktrees,
    /// Remove a RepoDesk-managed isolated worktree by workspace id.
    CleanupWorktree { workspace_id: String },
}

#[derive(Debug, Subcommand)]
pub enum OutcomesCommand {
    /// List recent recorded step outcomes (the learning signal), newest first.
    List {
        /// Maximum rows to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Aggregate outcomes for the active project per (task kind, provider).
    Stats,
    /// Confirm or override a verdict for one outcome row (human-in-the-loop).
    Confirm {
        /// The outcome row id (from `outcomes list`).
        id: i64,
        /// The verdict: good, bad, or neutral.
        verdict: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum CiCommand {
    /// Run the orchestrator in headless mode for CI/CD environments.
    RunOrchestrator {
        #[arg(long)]
        task: String,
    },
}
