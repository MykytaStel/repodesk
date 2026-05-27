use crate::store;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod diagnostic;
pub mod journal;
pub mod project;
pub mod settings;
pub mod task;
pub mod memory;

pub use diagnostic::*;
pub use journal::*;
pub use project::*;
pub use settings::*;
pub use task::*;
pub use memory::*;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub ok: bool,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopAction {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub risk: String,
    pub command_preview: String,
    #[serde(skip_serializing)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRunResult {
    pub id: String,
    pub title: String,
    pub risk: String,
    pub category: String,
    pub started_at_ms: u128,
    pub finished_at_ms: u128,
    pub result: CommandResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAddInput {
    pub name: String,
    pub path: String,
    pub project_type: String,
    pub main_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub action_id: Option<String>,
    pub artifact_kind: Option<String>,
    pub command_preview: Option<String>,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStatus {
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub exists: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductWorkflowState {
    pub generated_at_ms: u128,
    pub overall_status: String,
    pub primary_cta: String,
    pub recommended_action_id: Option<String>,
    pub recommended_action_title: Option<String>,
    pub steps: Vec<WorkflowStep>,
    pub artifacts: Vec<ArtifactStatus>,
    pub project_ok: bool,
    pub task_ok: bool,
    pub context_ok: bool,
    pub smart_context_ok: bool,
    pub prompts_ok: bool,
    pub checks_ok: bool,
    pub safety_ok: bool,
    pub project_info: CommandResult,
    pub task_status: CommandResult,
    pub workflow_hint: CommandResult,
    pub security_verdict: CommandResult,
    pub checks_summary_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactContent {
    pub kind: String,
    pub title: String,
    pub path: String,
    pub exists: bool,
    pub content: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTotals {
    pub entries_count: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_tokens: usize,
    pub today_total_tokens: usize,
    pub remaining_daily_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageItem {
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub estimated_cost_units: Option<f64>,
    pub currency_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenArtifactEstimate {
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub exists: bool,
    pub size_bytes: u64,
    pub estimated_tokens: Option<usize>,
    pub status: String,
    pub recommendation: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCostSummary {
    pub estimated_total_units: f64,
    pub currency_label: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageSnapshot {
    pub generated_at_ms: u128,
    pub totals: TokenTotals,
    pub by_provider: Vec<TokenUsageItem>,
    pub by_model: Vec<TokenUsageItem>,
    pub active_artifacts: Vec<TokenArtifactEstimate>,
    pub cost_estimate: TokenCostSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTokenUsageInput {
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub id: String,
    pub provider: String,
    pub available: bool,
    pub loaded: Option<bool>,
    pub context_window: Option<usize>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub auth_status: String,
    pub reachability: String,
    pub models: Vec<ModelStatus>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHealthSnapshot {
    pub generated_at_ms: u128,
    pub providers: Vec<ProviderHealth>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvDiagnostic {
    pub openai_api_key_set: bool,
    pub gemini_api_key_set: bool,
    pub anthropic_api_key_set: bool,
}

pub(crate) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(crate) fn workspace_root() -> PathBuf {
    let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for _ in 0..8 {
        if current.join("Cargo.toml").exists() && current.join("crates/repodesk-cli").exists() {
            return current;
        }

        if !current.pop() {
            break;
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(workspace_root)
}

pub(crate) fn history_file() -> PathBuf {
    home_dir()
        .join(".repodesk")
        .join("desktop")
        .join("action-history.jsonl")
}

pub(crate) fn truncate_text(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push_str("\n\n[RepoDesk truncated output to keep the UI responsive]");
    truncated
}

pub(crate) fn validate_short_id(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    if trimmed.len() > 80 {
        return Err(format!("{label} is too long"));
    }

    let safe = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'));

    if !safe {
        return Err(format!(
            "{label} may only contain letters, numbers, dash, underscore, dot or slash"
        ));
    }

    Ok(())
}

pub(crate) fn validate_text(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    if trimmed.len() > max_len {
        return Err(format!("{label} is too long"));
    }

    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(format!("{label} contains unsupported characters"));
    }

    Ok(())
}

pub(crate) fn validate_path(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Path cannot be empty".into());
    }

    if trimmed.len() > 512 {
        return Err("Path is too long".into());
    }

    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("Path contains unsupported characters".into());
    }

    Ok(())
}

pub(crate) fn action_catalog() -> Vec<DesktopAction> {
    vec![
        DesktopAction {
            id: "workflow-next".into(),
            title: "Decide next workflow step".into(),
            description:
                "Ask the workflow brain what should happen next based on current project state."
                    .into(),
            category: "Brain".into(),
            risk: "safe".into(),
            command_preview: "repodesk workflow next".into(),
            args: vec!["workflow".into(), "next".into()],
        },
        DesktopAction {
            id: "doctor-workflow".into(),
            title: "Run workflow doctor".into(),
            description: "Check project, task, context, prompts, checks, and guard state.".into(),
            category: "Brain".into(),
            risk: "safe".into(),
            command_preview: "repodesk doctor workflow".into(),
            args: vec!["doctor".into(), "workflow".into()],
        },
        DesktopAction {
            id: "context-build".into(),
            title: "Build context".into(),
            description: "Build the full active task context.md artifact.".into(),
            category: "Context".into(),
            risk: "safe".into(),
            command_preview: "repodesk context build".into(),
            args: vec!["context".into(), "build".into()],
        },
        DesktopAction {
            id: "smart-context-build".into(),
            title: "Build smart context".into(),
            description: "Build a smaller task-focused context pack to reduce token waste.".into(),
            category: "Context".into(),
            risk: "safe".into(),
            command_preview: "repodesk smart-context build".into(),
            args: vec!["smart-context".into(), "build".into()],
        },
        DesktopAction {
            id: "prompt-all".into(),
            title: "Generate agent prompts".into(),
            description: "Generate Codex, ChatGPT, and review prompts for the active task.".into(),
            category: "Agents".into(),
            risk: "safe".into(),
            command_preview: "repodesk prompt all".into(),
            args: vec!["prompt".into(), "all".into()],
        },
        DesktopAction {
            id: "safety-scan-context".into(),
            title: "Scan context for secrets".into(),
            description: "Scan active context for token/secret/password/private key risk signals."
                .into(),
            category: "Security".into(),
            risk: "safe".into(),
            command_preview: "repodesk safety scan-context".into(),
            args: vec!["safety".into(), "scan-context".into()],
        },
        DesktopAction {
            id: "security-audit".into(),
            title: "Run security audit".into(),
            description: "Show RepoDesk security policy and risk warnings.".into(),
            category: "Security".into(),
            risk: "safe".into(),
            command_preview: "repodesk security audit".into(),
            args: vec!["security".into(), "audit".into()],
        },
        DesktopAction {
            id: "judge-codex".into(),
            title: "Judge Codex readiness".into(),
            description: "Run guard, safety, and budget judgement for Codex.".into(),
            category: "Judge".into(),
            risk: "safe".into(),
            command_preview: "repodesk judge agent --agent codex".into(),
            args: vec![
                "judge".into(),
                "agent".into(),
                "--agent".into(),
                "codex".into(),
            ],
        },
        DesktopAction {
            id: "judge-chatgpt".into(),
            title: "Judge ChatGPT readiness".into(),
            description: "Run guard, safety, and budget judgement for ChatGPT.".into(),
            category: "Judge".into(),
            risk: "safe".into(),
            command_preview: "repodesk judge agent --agent chatgpt".into(),
            args: vec![
                "judge".into(),
                "agent".into(),
                "--agent".into(),
                "chatgpt".into(),
            ],
        },
        DesktopAction {
            id: "runtime-route-patch".into(),
            title: "Route patch work".into(),
            description: "Ask runtime router which provider should handle patch work.".into(),
            category: "Runtime".into(),
            risk: "safe".into(),
            command_preview: "repodesk runtime route --need patch".into(),
            args: vec![
                "runtime".into(),
                "route".into(),
                "--need".into(),
                "patch".into(),
            ],
        },
        DesktopAction {
            id: "runtime-route-compression".into(),
            title: "Route compression work".into(),
            description: "Ask runtime router which provider should compress context.".into(),
            category: "Runtime".into(),
            risk: "safe".into(),
            command_preview: "repodesk runtime route --need compression".into(),
            args: vec![
                "runtime".into(),
                "route".into(),
                "--need".into(),
                "compression".into(),
            ],
        },
        DesktopAction {
            id: "checks-run".into(),
            title: "Run configured checks".into(),
            description: "Run active project checks and save checks-summary.md.".into(),
            category: "Checks".into(),
            risk: "bounded-write".into(),
            command_preview: "repodesk checks run".into(),
            args: vec!["checks".into(), "run".into()],
        },
        DesktopAction {
            id: "git-audit".into(),
            title: "Audit git backup state".into(),
            description: "Review branch, remotes, and backup plan before pushing.".into(),
            category: "Backup".into(),
            risk: "safe".into(),
            command_preview: "repodesk git audit".into(),
            args: vec!["git".into(), "audit".into()],
        },
    ]
}

pub(crate) fn find_action(action_id: &str) -> Option<DesktopAction> {
    action_catalog()
        .into_iter()
        .find(|action| action.id == action_id)
}

pub(crate) fn run_cli(args: &[String]) -> CommandResult {
    let root = workspace_root();
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("-q")
        .arg("-p")
        .arg("repodesk-cli")
        .arg("--")
        .args(args)
        .current_dir(&root);

    match command.output() {
        Ok(output) => CommandResult {
            ok: output.status.success(),
            command: format!("repodesk {}", args.join(" ")),
            stdout: truncate_text(&String::from_utf8_lossy(&output.stdout), 18_000),
            stderr: truncate_text(&String::from_utf8_lossy(&output.stderr), 18_000),
            exit_code: output.status.code(),
        },
        Err(error) => CommandResult {
            ok: false,
            command: format!("repodesk {}", args.join(" ")),
            stdout: String::new(),
            stderr: error.to_string(),
            exit_code: None,
        },
    }
}

pub(crate) fn run_cli_str(args: &[&str]) -> CommandResult {
    let owned = args
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    run_cli(&owned)
}

pub(crate) fn save_action_history(result: &ActionRunResult) {
    let file = history_file();
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(line) = serde_json::to_string(result) {
        if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(handle, "{line}");
        }
    }
}

fn active_task_run_dir() -> Option<PathBuf> {
    repodesk_core::tasks::show_active_task()
        .ok()
        .map(|task| task.config.run_dir)
}

pub(crate) fn read_file_if_exists(path: &Path, max_chars: usize) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|content| truncate_text(&content, max_chars))
}

pub(crate) fn artifact_path(kind: &str) -> Result<(String, PathBuf), String> {
    let run_dir = active_task_run_dir().ok_or_else(|| "Active task is not set".to_string())?;
    let (title, file_name) = match kind {
        "context" => ("Context", "context.md"),
        "smart_context" => ("Smart Context", "smart-context.md"),
        "prompt_codex" => ("Codex Prompt", "prompt.codex.md"),
        "prompt_chatgpt" => ("ChatGPT Prompt", "prompt.chatgpt.md"),
        "prompt_review" => ("Review Prompt", "prompt.review.md"),
        "checks_summary" => ("Checks Summary", "checks-summary.md"),
        "token_estimate" => ("Token Estimate", "token-estimate.txt"),
        _ => return Err(format!("Unsupported artifact kind: {kind}")),
    };

    Ok((title.to_string(), run_dir.join(file_name)))
}

pub(crate) fn artifact_status(kind: &str) -> ArtifactStatus {
    match artifact_path(kind) {
        Ok((title, path)) => {
            let metadata = fs::metadata(&path).ok();
            ArtifactStatus {
                kind: kind.to_string(),
                title,
                path: Some(path.display().to_string()),
                exists: metadata.is_some(),
                size_bytes: metadata.map(|value| value.len()).unwrap_or_default(),
            }
        }
        Err(_) => ArtifactStatus {
            kind: kind.to_string(),
            title: kind.to_string(),
            path: None,
            exists: false,
            size_bytes: 0,
        },
    }
}

pub(crate) fn validate_model_name(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if trimmed.len() > 160 || trimmed.contains('\0') || trimmed.contains('\n') {
        return Err(format!("{label} is not safe"));
    }

    let safe = trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '@' | '+')
    });

    if !safe {
        return Err(format!("{label} contains unsupported characters"));
    }

    Ok(())
}

pub(crate) fn validate_optional_notes(value: &Option<String>) -> Result<(), String> {
    if let Some(notes) = value {
        if notes.len() > 1_000 || notes.contains('\0') {
            return Err("Notes are too long or unsafe".into());
        }

        let lower = notes.to_lowercase();
        if notes.contains("-----BEGIN") || lower.contains("api_key") || lower.contains("token=") {
            return Err("Notes must not contain secrets".into());
        }
    }

    Ok(())
}

fn token_artifact_estimate(kind: &str) -> TokenArtifactEstimate {
    match artifact_path(kind) {
        Ok((title, path)) => {
            let metadata = fs::metadata(&path).ok();
            let exists = metadata.is_some();
            let size_bytes = metadata
                .as_ref()
                .map(|value| value.len())
                .unwrap_or_default();

            if !exists {
                return TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: None,
                    status: "missing".into(),
                    recommendation: "Generate this artifact before sending context to an agent."
                        .into(),
                    error: None,
                };
            }

            match repodesk_core::tokens::estimate_file(&path) {
                Ok(estimate) => TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: Some(estimate.estimated_tokens),
                    status: estimate.status.as_label().to_string(),
                    recommendation: estimate.status.recommendation().to_string(),
                    error: None,
                },
                Err(error) => TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: None,
                    status: "unreadable".into(),
                    recommendation: "Open the artifact directly or rebuild it.".into(),
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => TokenArtifactEstimate {
            kind: kind.to_string(),
            title: kind.to_string(),
            path: None,
            exists: false,
            size_bytes: 0,
            estimated_tokens: None,
            status: "missing_task".into(),
            recommendation: "Create or select an active task first.".into(),
            error: Some(error),
        },
    }
}

pub(crate) fn build_token_usage_snapshot() -> TokenUsageSnapshot {
    let report = repodesk_core::token_ledger::read_token_report().unwrap_or(
        repodesk_core::token_ledger::TokenReport {
            entries_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            today_tokens: 0,
            by_agent: Vec::new(),
            by_model: Vec::new(),
        },
    );
    let cost_config = repodesk_core::cost::load_cost_config().unwrap_or_default();
    let budget_config = repodesk_core::budget::load_budget_config().unwrap_or_default();

    let daily_hard_limit = budget_config.daily_hard_limit;
    let today_total_tokens = report.today_tokens;
    let remaining_daily_tokens = if today_total_tokens >= daily_hard_limit {
        0
    } else {
        daily_hard_limit - today_total_tokens
    };

    let mut estimated_total_units = 0.0;
    let by_provider = report
        .by_agent
        .iter()
        .map(|item| {
            let estimate = repodesk_core::cost::estimate_agent_cost(
                &cost_config,
                &item.agent,
                item.input_tokens,
                item.output_tokens,
            );
            estimated_total_units += estimate.estimated_cost_units;
            TokenUsageItem {
                provider: item.agent.clone(),
                model: None,
                input_tokens: item.input_tokens,
                output_tokens: item.output_tokens,
                total_tokens: item.total_tokens,
                estimated_cost_units: Some(estimate.estimated_cost_units),
                currency_label: Some(estimate.currency_label),
            }
        })
        .collect::<Vec<_>>();

    let by_model = report
        .by_model
        .iter()
        .map(|item| {
            let estimate = repodesk_core::cost::estimate_agent_cost(
                &cost_config,
                &item.agent,
                item.input_tokens,
                item.output_tokens,
            );
            TokenUsageItem {
                provider: item.agent.clone(),
                model: Some(item.model.clone()),
                input_tokens: item.input_tokens,
                output_tokens: item.output_tokens,
                total_tokens: item.total_tokens,
                estimated_cost_units: Some(estimate.estimated_cost_units),
                currency_label: Some(estimate.currency_label),
            }
        })
        .collect::<Vec<_>>();

    TokenUsageSnapshot {
        generated_at_ms: now_ms(),
        totals: TokenTotals {
            entries_count: report.entries_count,
            total_input_tokens: report.total_input_tokens,
            total_output_tokens: report.total_output_tokens,
            total_tokens: report.total_tokens,
            today_total_tokens,
            remaining_daily_tokens,
        },
        by_provider,
        by_model,
        active_artifacts: vec![
            token_artifact_estimate("context"),
            token_artifact_estimate("smart_context"),
            token_artifact_estimate("prompt_codex"),
            token_artifact_estimate("prompt_chatgpt"),
            token_artifact_estimate("prompt_review"),
            token_artifact_estimate("checks_summary"),
        ],
        cost_estimate: TokenCostSummary {
            estimated_total_units,
            currency_label: cost_config.currency_label,
            note: "Planning estimate from local RepoDesk cost config. Real billing depends on provider and model."
                .into(),
        },
    }
}

struct HttpJsonError {
    status: Option<u16>,
    summary: String,
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_secs(3))
        .timeout_write(Duration::from_secs(3))
        .build()
}

fn request_json(url: &str, headers: &[(&str, &str)]) -> Result<serde_json::Value, HttpJsonError> {
    let agent = http_agent();
    let mut request = agent.get(url).set("accept", "application/json");
    for (key, value) in headers {
        request = request.set(key, value);
    }

    match request.call() {
        Ok(response) => response.into_json().map_err(|error| HttpJsonError {
            status: None,
            summary: format!("Invalid JSON response: {error}"),
        }),
        Err(ureq::Error::Status(code, response)) => {
            let body = response
                .into_string()
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect::<String>();
            Err(HttpJsonError {
                status: Some(code),
                summary: if body.trim().is_empty() {
                    format!("HTTP {code}")
                } else {
                    format!("HTTP {code}: {body}")
                },
            })
        }
        Err(error) => Err(HttpJsonError {
            status: None,
            summary: error.to_string(),
        }),
    }
}

fn model_status(provider: &str, id: String, notes: Option<String>) -> ModelStatus {
    ModelStatus {
        id,
        provider: provider.to_string(),
        available: true,
        loaded: None,
        context_window: None,
        notes,
    }
}

fn disabled_provider(id: &str, label: &str) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: false,
        auth_status: "disabled".into(),
        reachability: "disabled".into(),
        models: Vec::new(),
        error_summary: None,
    }
}

fn provider_error(
    id: &str,
    label: &str,
    auth_status: &str,
    reachability: &str,
    error: String,
) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: true,
        auth_status: auth_status.into(),
        reachability: reachability.into(),
        models: Vec::new(),
        error_summary: Some(truncate_text(&error, 500)),
    }
}

fn provider_working(
    id: &str,
    label: &str,
    auth_status: &str,
    models: Vec<ModelStatus>,
) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: true,
        auth_status: auth_status.into(),
        reachability: "working".into(),
        models,
        error_summary: None,
    }
}

fn join_url(base: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base.trim().trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

fn ollama_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.ollama_enabled {
        return disabled_provider("ollama", "Ollama");
    }

    match request_json(&join_url(&settings.ollama_url, "/api/tags"), &[]) {
        Ok(value) => {
            let models = value
                .get("models")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("model")
                                .or_else(|| item.get("name"))
                                .and_then(|value| value.as_str())
                                .map(|name| {
                                    model_status(
                                        "ollama",
                                        name.to_string(),
                                        item.get("details")
                                            .and_then(|details| details.get("parameter_size"))
                                            .and_then(|value| value.as_str())
                                            .map(|value| format!("parameters: {value}")),
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("ollama", "Ollama", "not_required", models)
        }
        Err(error) => provider_error(
            "ollama",
            "Ollama",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn lm_studio_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.lm_studio_enabled {
        return disabled_provider("lm_studio", "LM Studio");
    }

    match request_json(&join_url(&settings.lm_studio_url, "/v1/models"), &[]) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "lm_studio",
                                    id.to_string(),
                                    Some("visible to LM Studio OpenAI-compatible server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("lm_studio", "LM Studio", "not_required", models)
        }
        Err(error) => provider_error(
            "lm_studio",
            "LM Studio",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn llamafile_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.llamafile_enabled {
        return disabled_provider("llamafile", "Llamafile");
    }

    match request_json(&join_url(&settings.llamafile_url, "/v1/models"), &[]) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "llamafile",
                                    id.to_string(),
                                    Some("visible to Llamafile server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("llamafile", "Llamafile", "not_required", models)
        }
        Err(error) => provider_error(
            "llamafile",
            "Llamafile",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn localai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.localai_enabled {
        return disabled_provider("localai", "LocalAI");
    }

    match request_json(&join_url(&settings.localai_url, "/v1/models"), &[]) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "localai",
                                    id.to_string(),
                                    Some("visible to LocalAI server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("localai", "LocalAI", "not_required", models)
        }
        Err(error) => provider_error(
            "localai",
            "LocalAI",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn openai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.openai_api_enabled {
        return disabled_provider("openai", "OpenAI API");
    }

    let env_name = settings.openai_api_key_env_var.trim();
    let Ok(api_key) = env::var(env_name) else {
        return provider_error(
            "openai",
            "OpenAI API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live OpenAI model discovery."),
        );
    };

    if api_key.trim().is_empty() {
        return provider_error(
            "openai",
            "OpenAI API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live OpenAI model discovery."),
        );
    }

    let authorization = format!("Bearer {api_key}");
    match request_json(
        "https://api.openai.com/v1/models",
        &[("authorization", authorization.as_str())],
    ) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "openai",
                                    id.to_string(),
                                    item.get("owned_by")
                                        .and_then(|value| value.as_str())
                                        .map(|owner| format!("owned by {owner}")),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("openai", "OpenAI API", "configured", models)
        }
        Err(error) => {
            let reachability = match error.status {
                Some(401 | 403) => "auth_missing",
                Some(429) => "rate_limited",
                _ => "unreachable",
            };
            let auth_status = if reachability == "auth_missing" {
                "auth_missing"
            } else {
                "configured"
            };
            provider_error(
                "openai",
                "OpenAI API",
                auth_status,
                reachability,
                error.summary,
            )
        }
    }
}

fn gemini_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.gemini_api_enabled {
        return disabled_provider("gemini", "Gemini API");
    }

    let env_name = settings.gemini_api_key_env_var.trim();
    let Ok(api_key) = env::var(env_name) else {
        return provider_error(
            "gemini",
            "Gemini API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live Gemini model discovery."),
        );
    };

    if api_key.trim().is_empty() {
        return provider_error(
            "gemini",
            "Gemini API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live Gemini model discovery."),
        );
    }

    match request_json(
        "https://generativelanguage.googleapis.com/v1beta/models",
        &[("x-goog-api-key", api_key.as_str())],
    ) {
        Ok(value) => {
            let models = value
                .get("models")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("name")
                                .and_then(|value| value.as_str())
                                .map(|name| {
                                    let id = name.strip_prefix("models/").unwrap_or(name);
                                    model_status(
                                        "gemini",
                                        id.to_string(),
                                        item.get("displayName")
                                            .and_then(|value| value.as_str())
                                            .map(str::to_string),
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("gemini", "Gemini API", "configured", models)
        }
        Err(error) => {
            let reachability = match error.status {
                Some(401 | 403) => "auth_missing",
                Some(429) => "rate_limited",
                _ => "unreachable",
            };
            let auth_status = if reachability == "auth_missing" {
                "auth_missing"
            } else {
                "configured"
            };
            provider_error(
                "gemini",
                "Gemini API",
                auth_status,
                reachability,
                error.summary,
            )
        }
    }
}

pub(crate) fn model_health_from_settings(
    settings: &store::ProviderSettings,
) -> ModelHealthSnapshot {
    let s = settings.clone();

    let t_ollama = std::thread::spawn({
        let s = s.clone();
        move || ollama_health(&s)
    });
    let t_lm = std::thread::spawn({
        let s = s.clone();
        move || lm_studio_health(&s)
    });
    let t_llamafile = std::thread::spawn({
        let s = s.clone();
        move || llamafile_health(&s)
    });
    let t_localai = std::thread::spawn({
        let s = s.clone();
        move || localai_health(&s)
    });
    let t_openai = std::thread::spawn({
        let s = s.clone();
        move || openai_health(&s)
    });
    let t_gemini = std::thread::spawn({
        let s = s.clone();
        move || gemini_health(&s)
    });

    let providers = vec![
        t_ollama
            .join()
            .unwrap_or_else(|_| disabled_provider("ollama", "Ollama")),
        t_lm.join()
            .unwrap_or_else(|_| disabled_provider("lm_studio", "LM Studio")),
        t_llamafile
            .join()
            .unwrap_or_else(|_| disabled_provider("llamafile", "Llamafile")),
        t_localai
            .join()
            .unwrap_or_else(|_| disabled_provider("localai", "LocalAI")),
        t_openai
            .join()
            .unwrap_or_else(|_| disabled_provider("openai", "OpenAI API")),
        t_gemini
            .join()
            .unwrap_or_else(|_| disabled_provider("gemini", "Gemini API")),
    ];
    let mut warnings = Vec::new();

    if providers
        .iter()
        .any(|provider| provider.reachability == "auth_missing")
    {
        warnings.push(
            "Some API providers are enabled but missing environment-based credentials.".into(),
        );
    }

    if providers
        .iter()
        .filter(|provider| provider.enabled)
        .all(|provider| provider.reachability != "working")
    {
        warnings.push("No enabled model provider is currently reachable.".into());
    }

    ModelHealthSnapshot {
        generated_at_ms: now_ms(),
        providers,
        warnings,
    }
}

pub(crate) fn build_model_health_snapshot() -> ModelHealthSnapshot {
    let settings = store::read_provider_settings().unwrap_or_default();
    model_health_from_settings(&settings)
}

fn artifact_token_estimate(snapshot: &TokenUsageSnapshot, kind: &str) -> Option<usize> {
    snapshot
        .active_artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .and_then(|artifact| artifact.estimated_tokens)
}

fn infer_route_task_kind(
    workflow: &ProductWorkflowState,
    git: &repodesk_core::git_workspace::GitWorkspaceSnapshot,
) -> repodesk_core::routing::TaskKind {
    let action = workflow
        .recommended_action_id
        .as_deref()
        .unwrap_or_default();

    if !workflow.project_ok || !workflow.task_ok {
        return repodesk_core::routing::TaskKind::Manual;
    }
    if action.contains("check") {
        return repodesk_core::routing::TaskKind::Checks;
    }
    if action.contains("smart-context") {
        return repodesk_core::routing::TaskKind::Compress;
    }
    if action.contains("safety") {
        return repodesk_core::routing::TaskKind::Review;
    }
    if workflow.smart_context_ok
        && workflow.prompts_ok
        && workflow.checks_ok
        && !git.changed_files.is_empty()
    {
        return repodesk_core::routing::TaskKind::Patch;
    }
    if workflow.smart_context_ok {
        return repodesk_core::routing::TaskKind::Review;
    }

    repodesk_core::routing::TaskKind::Plan
}

fn default_output_tokens(kind: &repodesk_core::routing::TaskKind) -> usize {
    match kind {
        repodesk_core::routing::TaskKind::Compress
        | repodesk_core::routing::TaskKind::Summarize => 1_200,
        repodesk_core::routing::TaskKind::Plan
        | repodesk_core::routing::TaskKind::Review
        | repodesk_core::routing::TaskKind::Debug => 1_800,
        repodesk_core::routing::TaskKind::Patch => 3_500,
        repodesk_core::routing::TaskKind::Checks | repodesk_core::routing::TaskKind::Manual => 0,
    }
}

pub(crate) fn build_default_route_request(
    workflow: &ProductWorkflowState,
    tokens: &TokenUsageSnapshot,
    git: &repodesk_core::git_workspace::GitWorkspaceSnapshot,
    economy_mode: Option<String>,
) -> repodesk_core::routing::RouteRequest {
    let task_kind = infer_route_task_kind(workflow, git);
    let estimated_input_tokens = artifact_token_estimate(tokens, "smart_context")
        .or_else(|| artifact_token_estimate(tokens, "context"))
        .unwrap_or(0);
    let risk_level = if has_block_signal(&workflow.security_verdict) {
        "block"
    } else if has_warn_signal(&workflow.security_verdict) {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let requires_write = task_kind == repodesk_core::routing::TaskKind::Patch;

    repodesk_core::routing::RouteRequest {
        estimated_output_tokens: default_output_tokens(&task_kind),
        task_kind,
        estimated_input_tokens,
        risk_level,
        changed_file_count: git.changed_files.len(),
        requires_write,
        context_safe: Some(workflow.safety_ok),
        checks_ok: Some(workflow.checks_ok),
        guard_allowed: Some(workflow.safety_ok),
        git_dirty: Some(git.is_dirty),
        max_cost_units: None,
        economy_mode,
    }
}

fn cost_agent_for_provider(provider: &str) -> &str {
    match provider {
        "openai" | "chatgpt" => "chatgpt",
        "codex" => "codex",
        "gemini" => "gemini",
        _ => "ollama",
    }
}

fn estimated_route_cost_units(
    cost_config: &repodesk_core::cost::CostConfig,
    provider: &str,
    kind: &repodesk_core::routing::ProviderKind,
    request: &repodesk_core::routing::RouteRequest,
) -> f64 {
    if matches!(
        kind,
        repodesk_core::routing::ProviderKind::Local
            | repodesk_core::routing::ProviderKind::CheckRunner
            | repodesk_core::routing::ProviderKind::Manual
    ) {
        return 0.0;
    }

    repodesk_core::cost::estimate_agent_cost(
        cost_config,
        cost_agent_for_provider(provider),
        request.estimated_input_tokens,
        request.estimated_output_tokens,
    )
    .estimated_cost_units
}

fn route_capacity_from_health(
    provider: &ProviderHealth,
    kind: repodesk_core::routing::ProviderKind,
    preferred_model: Option<String>,
    daily_remaining_tokens: usize,
    cost_config: &repodesk_core::cost::CostConfig,
    budget_config: &repodesk_core::budget::BudgetConfig,
    request: &repodesk_core::routing::RouteRequest,
    paid_agents_allowed: bool,
) -> repodesk_core::routing::ProviderCapacity {
    let models = provider
        .models
        .iter()
        .filter(|model| model.available)
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    let estimated_cost_units =
        estimated_route_cost_units(cost_config, &provider.id, &kind, request);

    repodesk_core::routing::ProviderCapacity {
        provider: provider.id.clone(),
        label: provider.label.clone(),
        kind,
        enabled: provider.enabled,
        auth_status: provider.auth_status.clone(),
        reachability: provider.reachability.clone(),
        models,
        preferred_model,
        daily_remaining_tokens,
        estimated_cost_units,
        quota_status: repodesk_core::routing::QuotaStatus::Available,
        paid_agents_allowed,
        max_patch_files: budget_config.max_files_for_patch_agent,
    }
}

fn manual_route_capacity(
    provider: &str,
    label: &str,
    kind: repodesk_core::routing::ProviderKind,
    enabled: bool,
    reachability: &str,
    model: Option<&str>,
    quota_status: repodesk_core::routing::QuotaStatus,
    daily_remaining_tokens: usize,
    cost_config: &repodesk_core::cost::CostConfig,
    budget_config: &repodesk_core::budget::BudgetConfig,
    request: &repodesk_core::routing::RouteRequest,
    paid_agents_allowed: bool,
) -> repodesk_core::routing::ProviderCapacity {
    let models = model
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let estimated_cost_units = estimated_route_cost_units(cost_config, provider, &kind, request);

    repodesk_core::routing::ProviderCapacity {
        provider: provider.to_string(),
        label: label.to_string(),
        kind,
        enabled,
        auth_status: if enabled { "manual" } else { "disabled" }.into(),
        reachability: if enabled { reachability } else { "disabled" }.into(),
        models,
        preferred_model: model.map(str::to_string),
        daily_remaining_tokens,
        estimated_cost_units,
        quota_status,
        paid_agents_allowed,
        max_patch_files: budget_config.max_files_for_patch_agent,
    }
}

fn build_routing_capacities(
    settings: &store::ProviderSettings,
    model_health: &ModelHealthSnapshot,
    tokens: &TokenUsageSnapshot,
    budget_config: &repodesk_core::budget::BudgetConfig,
    cost_config: &repodesk_core::cost::CostConfig,
    request: &repodesk_core::routing::RouteRequest,
) -> Vec<repodesk_core::routing::ProviderCapacity> {
    let mut capacities = Vec::new();
    let daily_remaining_tokens = tokens.totals.remaining_daily_tokens;

    for provider in &model_health.providers {
        let capacity = match provider.id.as_str() {
            "ollama" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Local,
                Some(settings.ollama_model.clone()),
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "lm_studio" | "llamafile" | "localai" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Local,
                None,
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "openai" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Paid,
                None,
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "gemini" if settings.gemini_api_enabled => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Paid,
                None,
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            _ => None,
        };

        if let Some(capacity) = capacity {
            capacities.push(capacity);
        }
    }

    capacities.push(manual_route_capacity(
        "chatgpt",
        "ChatGPT manual",
        repodesk_core::routing::ProviderKind::Paid,
        settings.chatgpt_enabled,
        "unknown",
        Some("user-configured"),
        repodesk_core::routing::QuotaStatus::Unknown,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        settings.allow_paid_agents,
    ));

    if settings.gemini_enabled && !settings.gemini_api_enabled {
        capacities.push(manual_route_capacity(
            "gemini",
            "Gemini manual",
            repodesk_core::routing::ProviderKind::Paid,
            true,
            "unknown",
            Some("user-configured"),
            repodesk_core::routing::QuotaStatus::Unknown,
            daily_remaining_tokens,
            cost_config,
            budget_config,
            request,
            settings.allow_paid_agents,
        ));
    }

    capacities.push(manual_route_capacity(
        "codex",
        "Codex",
        repodesk_core::routing::ProviderKind::PatchAgent,
        settings.codex_enabled,
        "unknown",
        Some("codex-plan"),
        repodesk_core::routing::QuotaStatus::from_label(&settings.codex_quota_status),
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        settings.allow_paid_agents,
    ));

    capacities.push(manual_route_capacity(
        "local_checks",
        "Local checks",
        repodesk_core::routing::ProviderKind::CheckRunner,
        true,
        "working",
        Some("allowlisted-shell"),
        repodesk_core::routing::QuotaStatus::Available,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        true,
    ));

    capacities.push(manual_route_capacity(
        "manual",
        "Manual",
        repodesk_core::routing::ProviderKind::Manual,
        true,
        "manual",
        None,
        repodesk_core::routing::QuotaStatus::Available,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        true,
    ));

    capacities
}

pub(crate) fn build_routing_decision_for_request(
    input: &repodesk_core::routing::RouteRequest,
) -> repodesk_core::routing::RouteDecision {
    let settings = store::read_provider_settings().unwrap_or_default();
    let tokens = build_token_usage_snapshot();
    let model_health = model_health_from_settings(&settings);
    let budget_config = repodesk_core::budget::load_budget_config().unwrap_or_default();
    let cost_config = repodesk_core::cost::load_cost_config().unwrap_or_default();
    let capacities = build_routing_capacities(
        &settings,
        &model_health,
        &tokens,
        &budget_config,
        &cost_config,
        input,
    );

    repodesk_core::routing::route_request(input, &capacities, &budget_config)
}

pub(crate) fn build_routing_snapshot(economy_mode: Option<String>) -> repodesk_core::routing::RoutingSnapshot {
    let settings = store::read_provider_settings().unwrap_or_default();
    let tokens = build_token_usage_snapshot();
    let model_health = model_health_from_settings(&settings);
    let workflow = build_product_workflow_state();
    let git = repodesk_core::git_workspace::build_git_workspace_snapshot();
    let budget_config = repodesk_core::budget::load_budget_config().unwrap_or_default();
    let cost_config = repodesk_core::cost::load_cost_config().unwrap_or_default();
    let request = build_default_route_request(&workflow, &tokens, &git, economy_mode);
    let capacities = build_routing_capacities(
        &settings,
        &model_health,
        &tokens,
        &budget_config,
        &cost_config,
        &request,
    );
    let decision = repodesk_core::routing::route_request(&request, &capacities, &budget_config);

    repodesk_core::routing::RoutingSnapshot {
        generated_at_ms: now_ms(),
        request,
        decision,
        capacities,
    }
}

pub(crate) fn has_block_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("block") || text.contains("secret") || text.contains("private key")
}

pub(crate) fn has_warn_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("warn") || text.contains("warning") || text.contains("risk")
}

pub(crate) fn build_product_workflow_state() -> ProductWorkflowState {
    let generated_at_ms = now_ms();
    let project_info = run_cli_str(&["project", "info"]);
    let task_status = run_cli_str(&["task", "status"]);
    let workflow_hint = run_cli_str(&["workflow", "next"]);
    let security_verdict = run_cli_str(&["judge", "agent", "--agent", "codex"]);

    let context = artifact_status("context");
    let smart_context = artifact_status("smart_context");
    let prompt_codex = artifact_status("prompt_codex");
    let prompt_chatgpt = artifact_status("prompt_chatgpt");
    let prompt_review = artifact_status("prompt_review");
    let checks_summary = artifact_status("checks_summary");
    let token_estimate = artifact_status("token_estimate");

    let project_ok = project_info.ok;
    let task_ok = task_status.ok;
    let context_ok = context.exists;
    let smart_context_ok = smart_context.exists;
    let prompts_ok = prompt_codex.exists && prompt_chatgpt.exists && prompt_review.exists;
    let checks_ok = checks_summary.exists;
    let safety_ok = security_verdict.ok && !has_block_signal(&security_verdict);

    let mut recommended_action_id = None;
    let mut recommended_action_title = None;
    let mut primary_cta = "Review workflow".to_string();
    let mut overall_status = "ready".to_string();

    let mut choose = |action_id: &str, cta: &str, status: &str| {
        if let Some(action) = find_action(action_id) {
            recommended_action_id = Some(action.id);
            recommended_action_title = Some(action.title);
            primary_cta = cta.to_string();
            overall_status = status.to_string();
        }
    };

    if !project_ok {
        primary_cta = "Add or select a project".into();
        overall_status = "setup_required".into();
    } else if !task_ok {
        primary_cta = "Create an active task".into();
        overall_status = "setup_required".into();
    } else if !context_ok {
        choose("context-build", "Build context", "needs_context");
    } else if !smart_context_ok {
        choose(
            "smart-context-build",
            "Build smart context",
            "needs_smart_context",
        );
    } else if !safety_ok {
        choose(
            "safety-scan-context",
            "Scan context safety",
            "needs_safety_review",
        );
    } else if !prompts_ok {
        choose("prompt-all", "Generate prompts", "needs_prompts");
    } else if !checks_ok {
        choose("checks-run", "Run checks", "needs_checks");
    } else if has_warn_signal(&security_verdict) {
        choose("judge-codex", "Review AI readiness", "warning");
    } else {
        choose("workflow-next", "Ask brain for next step", "ready");
    }

    let steps = vec![
        WorkflowStep {
            id: "project".into(),
            title: "Project".into(),
            description:
                "RepoDesk needs an active project before it can build context or run checks.".into(),
            status: if project_ok { "done" } else { "current" }.into(),
            action_id: None,
            artifact_kind: None,
            command_preview: Some("repodesk project info".into()),
            blocker: if project_ok {
                None
            } else {
                Some("No active project".into())
            },
        },
        WorkflowStep {
            id: "task".into(),
            title: "Task".into(),
            description: "Every AI workflow should be scoped to one concrete task.".into(),
            status: if task_ok {
                "done"
            } else if project_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: None,
            artifact_kind: None,
            command_preview: Some("repodesk task status".into()),
            blocker: if task_ok {
                None
            } else {
                Some("No active task".into())
            },
        },
        WorkflowStep {
            id: "context".into(),
            title: "Context".into(),
            description: "Build the base task context from project state and task metadata.".into(),
            status: if context_ok {
                "done"
            } else if task_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("context-build".into()),
            artifact_kind: Some("context".into()),
            command_preview: Some("repodesk context build".into()),
            blocker: if task_ok {
                None
            } else {
                Some("Requires active task".into())
            },
        },
        WorkflowStep {
            id: "smart_context".into(),
            title: "Smart Context".into(),
            description:
                "Compress the working context so paid agents do not read unnecessary files.".into(),
            status: if smart_context_ok {
                "done"
            } else if context_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("smart-context-build".into()),
            artifact_kind: Some("smart_context".into()),
            command_preview: Some("repodesk smart-context build".into()),
            blocker: if context_ok {
                None
            } else {
                Some("Requires context.md".into())
            },
        },
        WorkflowStep {
            id: "safety".into(),
            title: "Safety".into(),
            description: "Judge the active context before handing it to AI.".into(),
            status: if safety_ok {
                "done"
            } else if smart_context_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("safety-scan-context".into()),
            artifact_kind: None,
            command_preview: Some("repodesk safety scan-context".into()),
            blocker: if smart_context_ok {
                None
            } else {
                Some("Requires smart context".into())
            },
        },
        WorkflowStep {
            id: "prompts".into(),
            title: "Prompts".into(),
            description: "Generate bounded prompts for Codex, ChatGPT, and review.".into(),
            status: if prompts_ok {
                "done"
            } else if safety_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("prompt-all".into()),
            artifact_kind: Some("prompt_codex".into()),
            command_preview: Some("repodesk prompt all".into()),
            blocker: if safety_ok {
                None
            } else {
                Some("Requires safety pass".into())
            },
        },
        WorkflowStep {
            id: "checks".into(),
            title: "Checks".into(),
            description: "Run configured project checks and keep only the useful summary for AI."
                .into(),
            status: if checks_ok {
                "done"
            } else if prompts_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("checks-run".into()),
            artifact_kind: Some("checks_summary".into()),
            command_preview: Some("repodesk checks run".into()),
            blocker: if prompts_ok {
                None
            } else {
                Some("Generate prompts first".into())
            },
        },
        WorkflowStep {
            id: "review".into(),
            title: "Review".into(),
            description:
                "Review prompts, checks, and action history before using an external agent.".into(),
            status: if project_ok && task_ok && smart_context_ok && prompts_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("judge-codex".into()),
            artifact_kind: Some("prompt_codex".into()),
            command_preview: Some("repodesk judge agent --agent codex".into()),
            blocker: None,
        },
    ];

    let checks_summary_preview = artifact_path("checks_summary")
        .ok()
        .and_then(|(_, path)| read_file_if_exists(&path, 5000));

    ProductWorkflowState {
        generated_at_ms,
        overall_status,
        primary_cta,
        recommended_action_id,
        recommended_action_title,
        steps,
        artifacts: vec![
            context,
            smart_context,
            prompt_codex,
            prompt_chatgpt,
            prompt_review,
            checks_summary,
            token_estimate,
        ],
        project_ok,
        task_ok,
        context_ok,
        smart_context_ok,
        prompts_ok,
        checks_ok,
        safety_ok,
        project_info,
        task_status,
        workflow_hint,
        security_verdict,
        checks_summary_preview,
    }
}
