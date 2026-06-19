//! Desktop bridge for the orchestrator: plan, run (with dry-run + cost ceiling),
//! and read back the latest / a specific run. Provider keys come from the
//! environment; the Ollama endpoint/model come from saved desktop settings.

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::executors::{self, ExecutorAvailability};
use repodesk_core::orchestrator::{
    self, LoopOptions, LoopRun, OrchestrationPlan, OrchestrationRun, RunOptions, RunSummary,
};
use repodesk_core::persistence::event_journal::{self, EventEntry};
use repodesk_core::tasks::show_active_task;

use super::ErrorPayload;

const MAX_RUNS: usize = 50;
const MAX_TIMELINE_EVENTS: usize = 100;

const MAX_GOAL_LEN: usize = 2_000;
/// Upper bound on autonomous-loop attempts, so the UI can never request a runaway.
const MAX_LOOP_ITERATIONS: usize = 10;

/// Provider settings for a run: API keys from env, Ollama url/model from the
/// saved desktop provider settings when available.
fn orchestrator_settings() -> ProviderSettings {
    let mut settings = ProviderSettings::from_env();
    if let Ok(saved) = crate::store::read_provider_settings() {
        settings.ollama.base_url = Some(saved.ollama_url);
        settings.ollama.default_model = Some(saved.ollama_model);
        settings.lm_studio.base_url = Some(saved.lm_studio_url);
        // Keys pasted into the app take precedence over environment variables.
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
pub async fn orchestrate_plan(goal: Option<String>) -> Result<OrchestrationPlan, ErrorPayload> {
    let goal = clean_goal(goal)?;
    Ok(orchestrator::build_plan(goal, &orchestrator_settings())?)
}

#[tauri::command]
pub async fn orchestrate_run(
    goal: Option<String>,
    dry_run: bool,
    max_cost: Option<f64>,
    approve_coding_agents: bool,
) -> Result<OrchestrationRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let settings = orchestrator_settings();
    let plan = orchestrator::build_plan(goal, &settings)?;
    let opts = RunOptions {
        dry_run,
        max_cost,
        settings,
        approve_coding_agents,
        coding_agent_timeout_secs: 600,
    };
    Ok(orchestrator::run_plan(&plan, &opts).await?)
}

/// Autonomously attempt the active task: plan → run → re-plan/retry under
/// guardrails. `approve_paid` and `approve_coding_agents` are separate
/// human-in-the-loop gates; the loop pauses before either gated route type.
#[tauri::command]
pub async fn orchestrate_loop(
    goal: Option<String>,
    max_iterations: Option<usize>,
    max_cost: Option<f64>,
    dry_run: bool,
    approve_paid: bool,
    approve_coding_agents: bool,
) -> Result<LoopRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let opts = LoopOptions {
        max_iterations: max_iterations.unwrap_or(3).clamp(1, MAX_LOOP_ITERATIONS),
        max_total_cost: max_cost,
        dry_run,
        approve_paid,
        approve_coding_agents,
        coding_agent_timeout_secs: 600,
        settings: orchestrator_settings(),
    };
    Ok(orchestrator::run_loop(goal, &opts).await?)
}

#[tauri::command]
pub fn coding_agent_executors() -> Result<Vec<ExecutorAvailability>, ErrorPayload> {
    executors::coding_agent_specs()
        .iter()
        .map(|spec| executors::coding_agent_availability(&spec.id).map_err(ErrorPayload::from))
        .collect()
}

#[tauri::command]
pub async fn orchestrate_status() -> Result<Option<OrchestrationRun>, ErrorPayload> {
    Ok(orchestrator::load_latest_run()?)
}

#[tauri::command]
pub async fn orchestrate_show(run_id: String) -> Result<Option<OrchestrationRun>, ErrorPayload> {
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
