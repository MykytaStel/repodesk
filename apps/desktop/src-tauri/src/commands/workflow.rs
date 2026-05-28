use super::*;

use std::fs;
use std::path::{Path, PathBuf};

pub use repodesk_core::workflow::{
    ActionRunResult, ArtifactContent, ArtifactStatus, DesktopAction, ProductWorkflowState,
    WorkflowStep,
};

pub(crate) fn action_catalog() -> Vec<DesktopAction> {
    repodesk_core::workflow::action_catalog()
}

pub(crate) fn find_action(action_id: &str) -> Option<DesktopAction> {
    repodesk_core::workflow::find_action(action_id)
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

    let checks_summary_preview = artifact_path("checks_summary")
        .ok()
        .and_then(|(_, path)| read_file_if_exists(&path, 5000));

    repodesk_core::workflow::build_product_workflow_state(
        generated_at_ms,
        project_info,
        task_status,
        workflow_hint,
        security_verdict,
        context,
        smart_context,
        prompt_codex,
        prompt_chatgpt,
        prompt_review,
        checks_summary,
        token_estimate,
        checks_summary_preview,
    )
}
