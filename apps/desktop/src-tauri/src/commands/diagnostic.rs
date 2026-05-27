use super::{
    action_catalog, build_model_health_snapshot, build_product_workflow_state,
    build_routing_decision_for_request, build_routing_snapshot, build_token_usage_snapshot, now_ms,
    run_cli_str, validate_model_name, validate_optional_notes, validate_short_id, workspace_root,
    ApiEnvDiagnostic, LogTokenUsageInput, ModelHealthSnapshot, TokenUsageSnapshot,
};
use crate::store;
use serde_json::json;

#[tauri::command]
pub async fn desktop_snapshot() -> serde_json::Value {
    json!({
        "mode": "desktop-product-workflow-mvp",
        "workspace_root": workspace_root(),
        "generated_at_ms": now_ms(),
        "actions": action_catalog(),
        "workflow_state": build_product_workflow_state(),
        "dashboard": run_cli_str(&["dashboard", "summary"]),
        "workflow": run_cli_str(&["workflow", "next"]),
        "doctor": run_cli_str(&["doctor", "workflow"]),
        "security": run_cli_str(&["security", "audit"]),
        "runtime": run_cli_str(&["runtime", "providers"]),
        "git": run_cli_str(&["git", "audit"]),
        "project_info": run_cli_str(&["project", "info"]),
        "project_list": run_cli_str(&["project", "list"]),
        "task_status": run_cli_str(&["task", "status"]),
        "task_show": run_cli_str(&["task", "show"]),
        "events": run_cli_str(&["events", "last", "--limit", "10"]),
        "knowledge": run_cli_str(&["knowledge", "show", "--kind", "decision"]),
    })
}

#[tauri::command]
pub fn token_usage_snapshot() -> TokenUsageSnapshot {
    build_token_usage_snapshot()
}

#[tauri::command]
pub fn log_token_usage(input: LogTokenUsageInput) -> Result<TokenUsageSnapshot, String> {
    validate_short_id("Provider", &input.provider)?;
    if let Some(model) = &input.model {
        validate_model_name("Model", model)?;
    }
    validate_short_id("Category", &input.category)?;
    validate_optional_notes(&input.notes)?;

    if input.input_tokens > 10_000_000 || input.output_tokens > 10_000_000 {
        return Err("Token counts are too large".into());
    }

    repodesk_core::token_ledger::log_token_event(repodesk_core::token_ledger::LogTokenInput {
        agent: input.provider.trim().to_ascii_lowercase(),
        model: input
            .model
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        input_tokens: input.input_tokens,
        output_tokens: input.output_tokens,
        category: input.category.trim().to_string(),
        notes: input.notes,
    })
    .map_err(|error| error.to_string())?;

    Ok(build_token_usage_snapshot())
}

#[tauri::command]
pub fn model_health_snapshot() -> ModelHealthSnapshot {
    build_model_health_snapshot()
}

#[tauri::command]
pub async fn refresh_model_health() -> ModelHealthSnapshot {
    build_model_health_snapshot()
}

#[tauri::command]
pub fn routing_decision(
    input: repodesk_core::routing::RouteRequest,
) -> repodesk_core::routing::RouteDecision {
    build_routing_decision_for_request(&input)
}

#[tauri::command]
pub fn routing_snapshot(economy_mode: Option<String>) -> repodesk_core::routing::RoutingSnapshot {
    build_routing_snapshot(economy_mode)
}

#[tauri::command]
pub fn db_status() -> store::DbStatus {
    store::db_status()
}

#[tauri::command]
pub fn get_api_env_diagnostic() -> ApiEnvDiagnostic {
    ApiEnvDiagnostic {
        openai_api_key_set: std::env::var("OPENAI_API_KEY")
            .map(|val| !val.trim().is_empty())
            .unwrap_or(false),
        gemini_api_key_set: std::env::var("GEMINI_API_KEY")
            .map(|val| !val.trim().is_empty())
            .unwrap_or(false),
        anthropic_api_key_set: std::env::var("ANTHROPIC_API_KEY")
            .map(|val| !val.trim().is_empty())
            .unwrap_or(false),
    }
}

#[tauri::command]
pub fn get_system_agents() -> Result<repodesk_core::agents::AgentsConfig, String> {
    repodesk_core::agents::ensure_agents_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_capabilities() -> Result<repodesk_core::capabilities::CapabilitiesConfig, String>
{
    repodesk_core::capabilities::ensure_capabilities_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_peripherals() -> Result<repodesk_core::peripherals::PeripheralsConfig, String> {
    repodesk_core::peripherals::ensure_peripherals_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_modules() -> Vec<repodesk_core::module_registry::BrainModule> {
    repodesk_core::module_registry::list_modules()
}
