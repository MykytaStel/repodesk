use super::{
    action_catalog, build_product_workflow_state, now_ms, run_cli_str, workspace_root,
    ApiEnvDiagnostic,
};
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
    })
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
