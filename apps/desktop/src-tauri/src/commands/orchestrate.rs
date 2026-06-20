//! Desktop bridge for the orchestrator: plan, run (with dry-run + cost ceiling),
//! and read back the latest / a specific run. Provider keys come from the
//! environment; the Ollama endpoint/model come from saved desktop settings.

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::checks;
use repodesk_core::executors::{self, ExecutorAvailability};
use repodesk_core::orchestrator::{
    self, AgentWorkspacePolicy, LoopOptions, LoopRun, OrchestrationPlan, OrchestrationRun,
    ReviewAction, RunOptions, RunReview, RunSummary, review_run,
};
use repodesk_core::persistence::event_journal::{self, EventEntry};
use repodesk_core::projects::get_active_project;
use repodesk_core::tasks::show_active_task;
use repodesk_core::worktree::{RunWorktreeCleanup, RunWorktreeStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::ErrorPayload;

const MAX_RUNS: usize = 50;
const MAX_TIMELINE_EVENTS: usize = 100;

const MAX_GOAL_LEN: usize = 2_000;
const MAX_DIFF_CHARS: usize = 120_000;
const MAX_PROOF_CHARS: usize = 24_000;
/// Upper bound on autonomous-loop attempts, so the UI can never request a runaway.
const MAX_LOOP_ITERATIONS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDiff {
    pub task_id: String,
    pub provider: String,
    pub model: String,
    pub changed_files: Vec<String>,
    pub diff_path: Option<String>,
    pub diff: String,
    pub exists: bool,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepProof {
    pub task_id: String,
    pub status: String,
    pub changed_files: Vec<String>,
    pub verification_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckProof {
    pub run_id: String,
    pub ran_checks: bool,
    pub success: Option<bool>,
    pub summary_path: Option<String>,
    pub log_path: Option<String>,
    pub summary: String,
    pub step_proofs: Vec<StepProof>,
    pub warnings: Vec<String>,
}

/// Provider settings for a run: API keys from env, Ollama url/model from the
/// saved desktop provider settings when available.
fn orchestrator_settings() -> ProviderSettings {
    let mut settings = ProviderSettings::from_env();
    if let Ok(saved) = crate::store::read_provider_settings() {
        settings.ollama.base_url = Some(saved.ollama_url);
        settings.ollama.default_model = Some(saved.ollama_model);
        settings.lm_studio.base_url = Some(saved.lm_studio_url);
        // `read_provider_settings` already resolves secrets keychain-first (with
        // a legacy plaintext fallback), so a configured key here takes precedence
        // over the environment. Secrets are never logged or returned to the UI.
        if !saved.anthropic_api_key.trim().is_empty() {
            settings.anthropic.api_key = Some(saved.anthropic_api_key);
        }
        if !saved.openai_api_key.trim().is_empty() {
            settings.openai.api_key = Some(saved.openai_api_key);
        }
        if !saved.gemini_api_key.trim().is_empty() {
            settings.gemini.api_key = Some(saved.gemini_api_key);
        }
    }
    settings
}

fn clean_goal(goal: Option<String>) -> Result<Option<String>, ErrorPayload> {
    if let Some(value) = &goal
        && value.len() > MAX_GOAL_LEN
    {
        return Err(ErrorPayload::resource_limit(format!(
            "goal is too long ({} > {MAX_GOAL_LEN} chars)",
            value.len()
        )));
    }
    Ok(goal.filter(|value| !value.trim().is_empty()))
}

#[tauri::command]
pub async fn orchestrate_plan(
    goal: Option<String>,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> Result<OrchestrationPlan, ErrorPayload> {
    let goal = clean_goal(goal)?;
    Ok(orchestrator::build_plan(
        goal,
        &orchestrator_settings(),
        override_provider,
        override_model,
    )?)
}

#[tauri::command]
pub async fn orchestrate_run(
    goal: Option<String>,
    dry_run: bool,
    max_cost: Option<f64>,
    approve_coding_agents: bool,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> Result<OrchestrationRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let settings = orchestrator_settings();
    let plan = orchestrator::build_plan(goal, &settings, override_provider, override_model)?;
    let opts = RunOptions {
        dry_run,
        max_cost,
        settings,
        approve_coding_agents,
        coding_agent_timeout_secs: 600,
        agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
    };
    Ok(orchestrator::run_plan(&plan, &opts).await?)
}

/// Autonomously attempt the active task: plan → run → re-plan/retry under
/// guardrails. `approve_paid` and `approve_coding_agents` are separate
/// human-in-the-loop gates; the loop pauses before either gated route type.
// A Tauri command's parameters map 1:1 to named JS args, so the flat signature
// is intentional rather than a config struct.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn orchestrate_loop(
    goal: Option<String>,
    max_iterations: Option<usize>,
    max_cost: Option<f64>,
    dry_run: bool,
    approve_paid: bool,
    approve_coding_agents: bool,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> Result<LoopRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let opts = LoopOptions {
        max_iterations: max_iterations.unwrap_or(3).clamp(1, MAX_LOOP_ITERATIONS),
        max_total_cost: max_cost,
        dry_run,
        approve_paid,
        approve_coding_agents,
        coding_agent_timeout_secs: 600,
        override_provider,
        override_model,
        settings: orchestrator_settings(),
        agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
    };
    Ok(orchestrator::run_loop(goal, &opts).await?)
}

#[tauri::command]
pub fn coding_agent_executors() -> Result<Vec<ExecutorAvailability>, ErrorPayload> {
    executors::coding_agent_specs()
        .iter()
        .map(|spec| {
            executors::coding_agent_availability_probed(&spec.id).map_err(ErrorPayload::from)
        })
        .collect()
}

/// Accept or reject the files a coding-agent run changed.
/// `action` is `accept` or `reject`. Bounded to the run's recorded changeset;
/// never commits or pushes.
#[tauri::command]
pub fn orchestrate_review(run_id: String, action: String) -> Result<RunReview, ErrorPayload> {
    validate_run_id(&run_id)?;
    let action = ReviewAction::from_label(&action)?;
    Ok(review_run(&run_id, action)?)
}

#[tauri::command]
pub fn orchestrate_run_diffs(run_id: String) -> Result<Vec<RunDiff>, ErrorPayload> {
    validate_run_id(&run_id)?;
    let run = orchestrator::load_run(&run_id)?.ok_or_else(|| {
        ErrorPayload::configuration(format!(
            "No orchestration run '{run_id}' for the active task"
        ))
    })?;
    let task = show_active_task()?;
    let run_dir = task.config.run_dir;

    Ok(run
        .results
        .iter()
        .filter(|result| !result.changed_files.is_empty() || result.diff_path.is_some())
        .map(|result| {
            let (diff, exists, truncated, mut warnings) =
                read_recorded_diff(&run_dir, result.diff_path.as_deref());
            if result.changed_files.iter().any(|path| path.trim().is_empty()) {
                warnings.push("run reported an empty changed path".to_string());
            }
            if diff.trim().is_empty() && !result.changed_files.is_empty() {
                warnings.push(
                    "No tracked unified diff was captured; changed files may be new, binary, or already moved."
                        .to_string(),
                );
            }
            RunDiff {
                task_id: result.task_id.clone(),
                provider: result.provider.clone(),
                model: result.model.clone(),
                changed_files: result.changed_files.clone(),
                diff_path: result.diff_path.clone(),
                diff,
                exists,
                truncated,
                warnings,
            }
        })
        .collect())
}

#[tauri::command]
pub fn orchestrate_check_proof(
    run_id: String,
    run_checks: bool,
) -> Result<CheckProof, ErrorPayload> {
    validate_run_id(&run_id)?;
    let run = orchestrator::load_run(&run_id)?.ok_or_else(|| {
        ErrorPayload::configuration(format!(
            "No orchestration run '{run_id}' for the active task"
        ))
    })?;

    let mut warnings = Vec::new();
    let mut success = None;
    let mut summary_path = None;
    let mut log_path = None;
    let summary = if run_checks {
        match checks::run_checks() {
            Ok(result) => {
                success = Some(result.success);
                summary_path = Some(result.summary_file.display().to_string());
                log_path = Some(result.log_file.display().to_string());
                std::fs::read_to_string(&result.summary_file)
                    .unwrap_or_else(|_| "Checks ran, but summary could not be read.".to_string())
            }
            Err(error) => {
                warnings.push(format!("failed to run checks: {error}"));
                String::new()
            }
        }
    } else {
        match checks::last_checks() {
            Ok(result) => {
                success = parse_check_success(&result.summary);
                summary_path = Some(result.summary_file.display().to_string());
                log_path = Some(result.log_file.display().to_string());
                result.summary
            }
            Err(error) => {
                warnings.push(format!("no check summary available: {error}"));
                String::new()
            }
        }
    };

    let step_proofs = run
        .results
        .iter()
        .map(|result| StepProof {
            task_id: result.task_id.clone(),
            status: format!("{:?}", result.status),
            changed_files: result.changed_files.clone(),
            verification_notes: result
                .notes
                .iter()
                .filter(|note| {
                    let lower = note.to_ascii_lowercase();
                    lower.contains("verify")
                        || lower.contains("duration_ms")
                        || lower.contains("changed files")
                        || lower.contains("timed out")
                })
                .cloned()
                .collect(),
        })
        .collect();

    Ok(CheckProof {
        run_id,
        ran_checks: run_checks,
        success,
        summary_path,
        log_path,
        summary: crate::commands::truncate_text(&summary, MAX_PROOF_CHARS),
        step_proofs,
        warnings,
    })
}

#[tauri::command]
pub fn orchestrate_worktrees() -> Result<Vec<RunWorktreeStatus>, ErrorPayload> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let parent = repodesk_core::worktree::worktrees_parent(&task.config.run_dir);
    Ok(repodesk_core::worktree::list_run_worktree_statuses(
        project.path.as_path(),
        &parent,
    )?)
}

#[tauri::command]
pub fn orchestrate_cleanup_worktree(
    workspace_id: String,
) -> Result<RunWorktreeCleanup, ErrorPayload> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let parent = repodesk_core::worktree::worktrees_parent(&task.config.run_dir);
    Ok(repodesk_core::worktree::cleanup_run_worktree(
        project.path.as_path(),
        &parent,
        &workspace_id,
    )?)
}

#[tauri::command]
pub async fn orchestrate_status() -> Result<Option<OrchestrationRun>, ErrorPayload> {
    Ok(orchestrator::load_latest_run()?)
}

#[tauri::command]
pub async fn orchestrate_show(run_id: String) -> Result<Option<OrchestrationRun>, ErrorPayload> {
    validate_run_id(&run_id)?;
    Ok(orchestrator::load_run(&run_id)?)
}

/// Every persisted run for the active task, newest-first, as lightweight
/// summaries for the history list.
#[tauri::command]
pub async fn orchestration_runs() -> Result<Vec<RunSummary>, ErrorPayload> {
    let mut runs = orchestrator::list_runs()?;
    runs.truncate(MAX_RUNS);
    Ok(runs)
}

/// The active task's recent activity timeline (event journal, newest-first).
#[tauri::command]
pub async fn task_timeline() -> Result<Vec<EventEntry>, ErrorPayload> {
    let task_id = show_active_task()?.config.id;
    Ok(event_journal::read_task_events(
        &task_id,
        MAX_TIMELINE_EVENTS,
    )?)
}

fn validate_run_id(run_id: &str) -> Result<(), ErrorPayload> {
    let safe = run_id.starts_with("run-")
        && run_id.len() <= 120
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    if safe {
        Ok(())
    } else {
        Err(ErrorPayload::configuration("Invalid run id"))
    }
}

fn read_recorded_diff(
    run_dir: &Path,
    diff_path: Option<&str>,
) -> (String, bool, bool, Vec<String>) {
    let Some(diff_path) = diff_path else {
        return (
            String::new(),
            false,
            false,
            vec!["no diff receipt path recorded".to_string()],
        );
    };
    let candidate = PathBuf::from(diff_path);
    let safe_path = match safe_receipt_path(run_dir, &candidate) {
        Ok(path) => path,
        Err(warning) => return (String::new(), false, false, vec![warning]),
    };
    match std::fs::read_to_string(&safe_path) {
        Ok(content) => {
            let truncated = content.chars().count() > MAX_DIFF_CHARS;
            (
                crate::commands::truncate_text(&content, MAX_DIFF_CHARS),
                true,
                truncated,
                Vec::new(),
            )
        }
        Err(error) => (
            String::new(),
            false,
            false,
            vec![format!("diff receipt could not be read: {error}")],
        ),
    }
}

fn safe_receipt_path(run_dir: &Path, candidate: &Path) -> Result<PathBuf, String> {
    if candidate.as_os_str().is_empty()
        || candidate
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("diff receipt path is empty or escapes the task run dir".to_string());
    }
    if !candidate.is_absolute() {
        return Err("diff receipt path is not absolute".to_string());
    }
    let base = run_dir
        .canonicalize()
        .map_err(|error| format!("task run dir could not be resolved: {error}"))?;
    let resolved = candidate
        .canonicalize()
        .map_err(|error| format!("diff receipt path could not be resolved: {error}"))?;
    if resolved.starts_with(&base) {
        Ok(resolved)
    } else {
        Err("diff receipt is outside the active task run dir".to_string())
    }
}

fn parse_check_success(summary: &str) -> Option<bool> {
    if summary.contains("Overall status: `passed`") {
        Some(true)
    } else if summary.contains("Overall status: `failed`") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_validation_rejects_path_traversal() {
        assert!(validate_run_id("run-20260101-000000-123-0").is_ok());
        assert!(validate_run_id("../run-1").is_err());
        assert!(validate_run_id("run-1/../../x").is_err());
        assert!(validate_run_id("latest.json").is_err());
    }

    #[test]
    fn check_summary_status_is_parsed() {
        assert_eq!(parse_check_success("Overall status: `passed`"), Some(true));
        assert_eq!(parse_check_success("Overall status: `failed`"), Some(false));
        assert_eq!(parse_check_success("No checks yet"), None);
    }
}
