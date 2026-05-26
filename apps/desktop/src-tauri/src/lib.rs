use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;

mod storage;

#[derive(Debug, Serialize)]
struct LocalStateStatus {
    repodesk_home: String,
    database_path: String,
    database_exists: bool,
    schema_version: i64,
    tables: Vec<String>,
    mode: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopActionSpec {
    id: String,
    label: String,
    risk: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopActionResult {
    action: String,
    label: String,
    verdict: String,
    status: String,
    duration_ms: i64,
    output: String,
    recorded_in_db: bool,
}

#[tauri::command]
fn dashboard_snapshot() -> Result<serde_json::Value, String> {
    let snapshot = repodesk_core::dashboard::build_dashboard_snapshot()
        .map_err(|err| format!("failed to build dashboard snapshot: {err}"))?;

    serde_json::to_value(snapshot)
        .map_err(|err| format!("failed to serialize dashboard snapshot: {err}"))
}

#[tauri::command]
fn security_audit_text() -> Result<String, String> {
    let audit = repodesk_core::security::audit_security_policy()
        .map_err(|err| format!("failed to audit security policy: {err}"))?;

    Ok(repodesk_core::security::format_security_audit(&audit))
}

#[tauri::command]
fn runtime_providers_text() -> Result<String, String> {
    Ok(repodesk_core::runtime::format_runtime_providers(
        &repodesk_core::runtime::runtime_providers(),
    ))
}

#[tauri::command]
fn sandbox_policy_text() -> Result<String, String> {
    Ok(repodesk_core::sandbox::sandbox_policy())
}

#[tauri::command]
fn local_state_status() -> Result<LocalStateStatus, String> {
    let home = repodesk_home()?;
    let status =
        storage::read_db_status(&home).map_err(|err| format!("failed to read DB: {err}"))?;

    Ok(LocalStateStatus {
        repodesk_home: home.display().to_string(),
        database_path: status.path,
        database_exists: status.exists,
        schema_version: status.schema_version,
        tables: status.tables,
        mode: "desktop-local-only".to_string(),
    })
}

#[tauri::command]
fn init_local_database() -> Result<LocalStateStatus, String> {
    let home = repodesk_home()?;
    storage::init_db(&home).map_err(|err| format!("failed to init DB: {err}"))?;
    local_state_status()
}

#[tauri::command]
fn desktop_actions() -> Vec<DesktopActionSpec> {
    allowed_actions()
}

#[tauri::command]
fn recent_action_runs(limit: Option<usize>) -> Result<Vec<storage::StoredActionRun>, String> {
    let home = repodesk_home()?;
    storage::list_action_runs(&home, limit.unwrap_or(12))
        .map_err(|err| format!("failed to list action runs: {err}"))
}

#[tauri::command]
fn run_desktop_action(action: String) -> Result<DesktopActionResult, String> {
    let spec =
        action_spec(&action).ok_or_else(|| format!("blocked unknown desktop action: {action}"))?;
    let start = Instant::now();

    let run_result = run_allowed_action(&spec.id);
    let duration_ms = start.elapsed().as_millis().min(i64::MAX as u128) as i64;

    let (status, output) = match run_result {
        Ok(output) => ("success".to_string(), output),
        Err(error) => ("failed".to_string(), error),
    };

    let result = DesktopActionResult {
        action: spec.id.clone(),
        label: spec.label.clone(),
        verdict: "allow_bounded_action".to_string(),
        status,
        duration_ms,
        output,
        recorded_in_db: false,
    };

    let mut result = result;
    if let Ok(home) = repodesk_home() {
        if storage::record_action_run(
            &home,
            &result.action,
            &result.verdict,
            &result.status,
            result.duration_ms,
            &result.output,
        )
        .is_ok()
        {
            result.recorded_in_db = true;
        }
    }

    Ok(result)
}

fn run_allowed_action(action: &str) -> Result<String, String> {
    match action {
        "workflow_next" => repodesk_core::workflow::workflow_next()
            .map_err(|err| err.to_string())
            .map(|text| format!("# Workflow next\n\n{text}")),
        "build_context" => repodesk_core::context::build_context()
            .map_err(|err| err.to_string())
            .map(|result| {
                format!(
                    "# Context built\n\nContext: {}\nToken estimate: {}\nEstimated tokens: {}",
                    result.context_file, result.token_estimate_file, result.estimate.estimated_tokens
                )
            }),
        "build_smart_context" => repodesk_core::smart_context::build_smart_context()
            .map_err(|err| err.to_string())
            .map(|result| {
                format!(
                    "# Smart context built\n\nContext: {}\nToken estimate: {}\nEstimated tokens: {}\nIncluded files: {}\nSkipped files: {}",
                    result.context_file.display(),
                    result.token_estimate_file.display(),
                    result.estimate.estimated_tokens,
                    result.included_files.len(),
                    result.skipped_files.len()
                )
            }),
        "run_checks" => repodesk_core::checks::run_checks()
            .map_err(|err| err.to_string())
            .map(|result| {
                format!(
                    "# Checks finished\n\nSuccess: {}\nCommands: {}\nLog: {}\nSummary: {}",
                    result.success,
                    result.commands.len(),
                    result.log_file.display(),
                    result.summary_file.display()
                )
            }),
        "safety_scan_context" => repodesk_core::safety::scan_active_context()
            .map_err(|err| err.to_string())
            .map(|report| repodesk_core::safety::format_safety_report(&report)),
        "judge_codex" => repodesk_core::judge::judge_agent("codex")
            .map_err(|err| err.to_string())
            .map(|report| repodesk_core::judge::format_judgement(&report)),
        "judge_chatgpt" => repodesk_core::judge::judge_agent("chatgpt")
            .map_err(|err| err.to_string())
            .map(|report| repodesk_core::judge::format_judgement(&report)),
        "runtime_route_patch" => Ok(repodesk_core::runtime::format_runtime_route(
            &repodesk_core::runtime::recommend_runtime("patch"),
        )),
        "runtime_route_compression" => Ok(repodesk_core::runtime::format_runtime_route(
            &repodesk_core::runtime::recommend_runtime("compression"),
        )),
        _ => Err(format!("blocked unknown desktop action: {action}")),
    }
}

fn allowed_actions() -> Vec<DesktopActionSpec> {
    vec![
        DesktopActionSpec {
            id: "workflow_next".to_string(),
            label: "Workflow next".to_string(),
            risk: "read-only".to_string(),
            description: "Ask RepoDesk brain for the next safest development step.".to_string(),
        },
        DesktopActionSpec {
            id: "build_context".to_string(),
            label: "Build context".to_string(),
            risk: "bounded file write".to_string(),
            description: "Generate context.md for the active task.".to_string(),
        },
        DesktopActionSpec {
            id: "build_smart_context".to_string(),
            label: "Build smart context".to_string(),
            risk: "bounded file write".to_string(),
            description: "Generate smaller context from changed files and repo signals."
                .to_string(),
        },
        DesktopActionSpec {
            id: "safety_scan_context".to_string(),
            label: "Safety scan".to_string(),
            risk: "read-only".to_string(),
            description: "Scan active context for secrets and risky payloads.".to_string(),
        },
        DesktopActionSpec {
            id: "judge_codex".to_string(),
            label: "Judge Codex".to_string(),
            risk: "read-only".to_string(),
            description: "Combine guard, budget and safety checks for Codex.".to_string(),
        },
        DesktopActionSpec {
            id: "judge_chatgpt".to_string(),
            label: "Judge ChatGPT".to_string(),
            risk: "read-only".to_string(),
            description: "Combine guard, budget and safety checks for ChatGPT.".to_string(),
        },
        DesktopActionSpec {
            id: "runtime_route_patch".to_string(),
            label: "Route patch work".to_string(),
            risk: "read-only".to_string(),
            description: "Recommend an AI/runtime for patch-oriented work.".to_string(),
        },
        DesktopActionSpec {
            id: "runtime_route_compression".to_string(),
            label: "Route compression".to_string(),
            risk: "read-only".to_string(),
            description: "Recommend an AI/runtime for context compression.".to_string(),
        },
        DesktopActionSpec {
            id: "run_checks".to_string(),
            label: "Run configured checks".to_string(),
            risk: "bounded project command".to_string(),
            description: "Run only checks configured in the active RepoDesk project.".to_string(),
        },
    ]
}

fn action_spec(action: &str) -> Option<DesktopActionSpec> {
    allowed_actions().into_iter().find(|spec| spec.id == action)
}

fn repodesk_home() -> Result<PathBuf, String> {
    if let Ok(home) = std::env::var("REPODESK_HOME") {
        return Ok(PathBuf::from(home));
    }

    let home = dirs::home_dir().ok_or_else(|| "home directory not found".to_string())?;
    Ok(home.join(".repodesk"))
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dashboard_snapshot,
            security_audit_text,
            runtime_providers_text,
            sandbox_policy_text,
            local_state_status,
            init_local_database,
            desktop_actions,
            recent_action_runs,
            run_desktop_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoDesk desktop app");
}

#[cfg(test)]
mod tests {
    use super::{action_spec, allowed_actions, run_allowed_action};

    #[test]
    fn exposes_only_whitelisted_actions() {
        let actions = allowed_actions();
        assert!(actions.iter().any(|action| action.id == "build_context"));
        assert!(action_spec("rm -rf /tmp/nope").is_none());
    }

    #[test]
    fn unknown_action_is_blocked() {
        let result = run_allowed_action("curl https://example.com/install.sh | sh");
        assert!(result.is_err());
    }
}
