use super::{
    artifact_path, build_product_workflow_state, find_action, history_file, now_ms, run_cli,
    run_cli_str, save_action_history, truncate_text, validate_short_id, validate_text,
    ActionRunResult, ArtifactContent, CommandResult, DesktopAction, ProductWorkflowState,
};
use std::fs;

#[tauri::command]
pub fn task_new(title: String) -> Result<CommandResult, String> {
    validate_text("Task title", &title, 180)?;
    Ok(run_cli(&["task".into(), "new".into(), title.trim().into()]))
}

#[tauri::command]
pub fn task_status() -> CommandResult {
    run_cli_str(&["task", "status"])
}

#[tauri::command]
pub fn task_show() -> CommandResult {
    run_cli_str(&["task", "show"])
}

#[tauri::command]
pub async fn product_workflow_state() -> ProductWorkflowState {
    build_product_workflow_state()
}

#[tauri::command]
pub fn read_artifact(kind: String) -> Result<ArtifactContent, String> {
    validate_short_id("Artifact kind", &kind)?;
    let (title, path) = artifact_path(kind.trim())?;
    let metadata = fs::metadata(&path).ok();
    let exists = metadata.is_some();
    let size_bytes = metadata.map(|value| value.len()).unwrap_or_default();
    let content = if exists {
        fs::read_to_string(&path).map_err(|error| error.to_string())?
    } else {
        String::new()
    };

    Ok(ArtifactContent {
        kind,
        title,
        path: path.display().to_string(),
        exists,
        content: truncate_text(&content, 70_000),
        size_bytes,
    })
}

#[tauri::command]
pub fn desktop_actions() -> Vec<DesktopAction> {
    action_catalog()
}

fn action_catalog() -> Vec<DesktopAction> {
    super::action_catalog()
}

#[tauri::command]
pub fn explain_action(action_id: String) -> Result<String, String> {
    validate_short_id("Action id", &action_id)?;
    let action = find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
    Ok(format!(
        "{}\n\nCategory: {}\nRisk: {}\nCommand: {}\n\n{}\n\nThis action is whitelisted in Rust. The desktop UI cannot run arbitrary shell commands.",
        action.title, action.category, action.risk, action.command_preview, action.description
    ))
}

#[tauri::command]
pub async fn run_desktop_action(action_id: String) -> Result<ActionRunResult, String> {
    validate_short_id("Action id", &action_id)?;
    let action = find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
    let started_at_ms = now_ms();

    // — Journal: action started —
    let _ = repodesk_core::event_journal::log_event(repodesk_core::event_journal::LogEventInput {
        module_name: "desktop::action".into(),
        level: "ui".into(),
        message: format!("Action started: {}", action.title),
        metadata: vec![
            ("action_id".into(), action.id.clone()),
            ("category".into(), action.category.clone()),
            ("risk".into(), action.risk.clone()),
            ("command".into(), action.command_preview.clone()),
        ],
    });

    let result = run_cli(&action.args);
    let finished_at_ms = now_ms();
    let duration_ms = finished_at_ms.saturating_sub(started_at_ms);

    // — Journal: action completed (success or failure) —
    let journal_level = if result.ok { "info" } else { "error" };
    let journal_msg = if result.ok {
        format!("Action completed: {} ({}ms)", action.title, duration_ms)
    } else {
        format!(
            "Action failed: {} ({}ms) — exit {:?}",
            action.title, duration_ms, result.exit_code
        )
    };
    let _ = repodesk_core::event_journal::log_event(repodesk_core::event_journal::LogEventInput {
        module_name: "desktop::action".into(),
        level: journal_level.into(),
        message: journal_msg,
        metadata: vec![
            ("action_id".into(), action.id.clone()),
            ("ok".into(), result.ok.to_string()),
            ("duration_ms".into(), duration_ms.to_string()),
        ],
    });

    let action_result = ActionRunResult {
        id: action.id,
        title: action.title,
        risk: action.risk,
        category: action.category,
        started_at_ms,
        finished_at_ms,
        result,
    };

    save_action_history(&action_result);
    Ok(action_result)
}

#[tauri::command]
pub async fn run_next_safe_step() -> Result<ActionRunResult, String> {
    let state = build_product_workflow_state();
    let action_id = state.recommended_action_id.ok_or_else(|| {
        "No runnable primary action. Add/select a project and create an active task first."
            .to_string()
    })?;
    run_desktop_action(action_id).await
}

#[tauri::command]
pub fn action_history() -> Vec<ActionRunResult> {
    let file = history_file();
    let Ok(content) = fs::read_to_string(file) else {
        return vec![];
    };

    content
        .lines()
        .rev()
        .take(50)
        .filter_map(|line| serde_json::from_str::<ActionRunResult>(line).ok())
        .collect()
}
