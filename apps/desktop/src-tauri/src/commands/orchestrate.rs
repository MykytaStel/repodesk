//! Desktop bridge for the orchestrator: plan, run (with dry-run + cost ceiling),
//! and read back the latest / a specific run. Provider keys come from the
//! environment; the Ollama endpoint/model come from saved desktop settings.

use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::orchestrator::{self, OrchestrationPlan, OrchestrationRun, RunOptions};

use super::ErrorPayload;

const MAX_GOAL_LEN: usize = 2_000;

/// Provider settings for a run: API keys from env, Ollama url/model from the
/// saved desktop provider settings when available.
fn orchestrator_settings() -> ProviderSettings {
    let mut settings = ProviderSettings::from_env();
    if let Ok(saved) = crate::store::read_provider_settings() {
        settings.ollama.base_url = Some(saved.ollama_url);
        settings.ollama.default_model = Some(saved.ollama_model);
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
) -> Result<OrchestrationRun, ErrorPayload> {
    let goal = clean_goal(goal)?;
    let settings = orchestrator_settings();
    let plan = orchestrator::build_plan(goal, &settings)?;
    let opts = RunOptions {
        dry_run,
        max_cost,
        settings,
    };
    Ok(orchestrator::run_plan(&plan, &opts).await?)
}

#[tauri::command]
pub async fn orchestrate_status() -> Result<Option<OrchestrationRun>, ErrorPayload> {
    Ok(orchestrator::load_latest_run()?)
}

#[tauri::command]
pub async fn orchestrate_show(run_id: String) -> Result<Option<OrchestrationRun>, ErrorPayload> {
    Ok(orchestrator::load_run(&run_id)?)
}
