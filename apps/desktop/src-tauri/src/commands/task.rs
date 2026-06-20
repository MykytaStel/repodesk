use super::{
    ActionRunResult, ArtifactContent, CommandResult, DesktopAction, ErrorPayload,
    ProductWorkflowState, action_service, artifact_path, build_product_workflow_state, find_action,
    now_ms, save_action_history, truncate_text, validate_short_id, validate_text,
};
use std::fs;

#[tauri::command]
pub fn task_new(
    title: String,
    verify_command: Option<String>,
) -> Result<CommandResult, ErrorPayload> {
    validate_text("Task title", &title, 180)?;
    match repodesk_core::tasks::create_task(repodesk_core::tasks::NewTaskInput {
        title: title.clone(),
        verify_command,
    }) {
        Ok(task) => {
            let stdout = format!(
                "Task created and set as active:\n  id: {}\n  project: {}\n  title: {}\n  run dir: {}\n  task file: {}",
                task.config.id,
                task.config.project_name,
                task.config.title,
                task.config.run_dir.display(),
                task.task_markdown_file.display()
            );
            Ok(CommandResult {
                ok: true,
                command: format!("repodesk task new {}", title),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
        Err(e) => Ok(CommandResult {
            ok: false,
            command: format!("repodesk task new {}", title),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        }),
    }
}

#[tauri::command]
pub fn task_status() -> CommandResult {
    match repodesk_core::tasks::task_status() {
        Ok(task) => {
            let stdout = format!(
                "Task status:\n  id: {}\n  project: {}\n  title: {}\n  status: {:?}\n  run dir: {}\n  task config: {}\n  task markdown: {}",
                task.config.id,
                task.config.project_name,
                task.config.title,
                task.config.status,
                task.config.run_dir.display(),
                task.task_file.display(),
                task.task_markdown_file.display()
            );
            CommandResult {
                ok: true,
                command: "repodesk task status".to_string(),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk task status".to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    }
}

#[tauri::command]
pub fn task_show() -> CommandResult {
    match repodesk_core::tasks::show_active_task() {
        Ok(task) => {
            let stdout = format!(
                "Active task:\n  id: {}\n  project: {}\n  title: {}\n  status: {:?}\n  run dir: {}\n  task config: {}\n  task markdown: {}",
                task.config.id,
                task.config.project_name,
                task.config.title,
                task.config.status,
                task.config.run_dir.display(),
                task.task_file.display(),
                task.task_markdown_file.display()
            );
            CommandResult {
                ok: true,
                command: "repodesk task show".to_string(),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk task show".to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    }
}

/// List every task in the active project, newest first, for the task switcher.
/// Returns structured data (not a CLI string) since the UI renders it directly.
#[tauri::command]
pub fn task_list() -> Result<Vec<repodesk_core::tasks::TaskSummary>, ErrorPayload> {
    repodesk_core::tasks::list_tasks().map_err(ErrorPayload::from)
}

/// Switch the active task to `task_id` within the active project.
#[tauri::command]
pub fn task_use(task_id: String) -> Result<CommandResult, ErrorPayload> {
    validate_short_id("Task id", &task_id)?;
    match repodesk_core::tasks::use_task(task_id.trim()) {
        Ok(task) => {
            let stdout = format!(
                "Active task set to '{}'\n  title: {}\n  run dir: {}",
                task.config.id,
                task.config.title,
                task.config.run_dir.display()
            );
            Ok(CommandResult {
                ok: true,
                command: format!("repodesk task use {}", task_id),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
        Err(e) => Ok(CommandResult {
            ok: false,
            command: format!("repodesk task use {}", task_id),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        }),
    }
}

#[tauri::command]
pub async fn product_workflow_state() -> ProductWorkflowState {
    build_product_workflow_state()
}

#[tauri::command]
pub fn read_artifact(kind: String) -> Result<ArtifactContent, ErrorPayload> {
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
pub async fn agent_context_pack() -> Result<ArtifactContent, ErrorPayload> {
    let pack = repodesk_core::agent_context_pack::build_agent_context_pack().await?;
    Ok(ArtifactContent {
        kind: "agent_context_pack".to_string(),
        title: "Agent Context Pack".to_string(),
        path: pack.path.display().to_string(),
        exists: true,
        content: truncate_text(&pack.content, 70_000),
        size_bytes: pack.size_bytes,
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
pub fn explain_action(action_id: String) -> Result<String, ErrorPayload> {
    validate_short_id("Action id", &action_id)?;
    let action = find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
    Ok(format!(
        "{}\n\nCategory: {}\nRisk: {}\nCommand: {}\n\n{}\n\nThis action is whitelisted in Rust. The desktop UI cannot run arbitrary shell commands.",
        action.title, action.category, action.risk, action.command_preview, action.description
    ))
}

#[tauri::command]
pub async fn run_desktop_action(action_id: String) -> Result<ActionRunResult, ErrorPayload> {
    validate_short_id("Action id", &action_id)?;
    let action = find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
    let started_at_ms = now_ms();

    // — Journal: action started —
    let _ = repodesk_core::persistence::event_journal::log_event(
        repodesk_core::persistence::event_journal::LogEventInput {
            module_name: "desktop::action".into(),
            level: "ui".into(),
            message: format!("Action started: {}", action.title),
            metadata: vec![
                ("action_id".into(), action.id.clone()),
                ("category".into(), action.category.clone()),
                ("risk".into(), action.risk.clone()),
                ("command".into(), action.command_preview.clone()),
            ],
        },
    );

    let result = action_service::run_action(&action).await;
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
    let _ = repodesk_core::persistence::event_journal::log_event(
        repodesk_core::persistence::event_journal::LogEventInput {
            module_name: "desktop::action".into(),
            level: journal_level.into(),
            message: journal_msg,
            metadata: vec![
                ("action_id".into(), action.id.clone()),
                ("ok".into(), result.ok.to_string()),
                ("duration_ms".into(), duration_ms.to_string()),
            ],
        },
    );

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
pub async fn run_next_safe_step() -> Result<ActionRunResult, ErrorPayload> {
    let state = build_product_workflow_state();
    let action_id = state.recommended_action_id.ok_or_else(|| {
        "No runnable primary action. Add/select a project and create an active task first."
            .to_string()
    })?;
    run_desktop_action(action_id).await
}

#[tauri::command]
pub fn action_history() -> Vec<ActionRunResult> {
    repodesk_core::persistence::recent_action_runs(50).unwrap_or_default()
}
