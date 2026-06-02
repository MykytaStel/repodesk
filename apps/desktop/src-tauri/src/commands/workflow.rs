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

    if let Ok(line) = serde_json::to_string(result)
        && let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(file)
    {
        let _ = writeln!(handle, "{line}");
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

use std::sync::Mutex;
use std::time::Instant;

static WORKFLOW_CACHE: Mutex<Option<(ProductWorkflowState, Instant)>> = Mutex::new(None);

pub(crate) fn build_product_workflow_state() -> ProductWorkflowState {
    if let Ok(cache) = WORKFLOW_CACHE.lock()
        && let Some((ref state, ref last_updated)) = *cache
        && last_updated.elapsed() < Duration::from_secs(1)
    {
        return state.clone();
    }

    let generated_at_ms = now_ms();
    let project_info = match repodesk_core::projects::get_active_project() {
        Ok(config) => {
            let mut stdout = format!(
                "Active project:\n  name: {}\n  path: {}\n  type: {}",
                config.name,
                config.path.display(),
                config.project_type
            );
            if let Some(lang) = config.main_language {
                stdout.push_str(&format!("\n  main language: {}", lang));
            }
            if config.checks.is_empty() {
                stdout.push_str("\n  checks: none configured");
            } else {
                stdout.push_str("\n  checks:");
                for check in config.checks {
                    stdout.push_str(&format!("\n    - {}", check));
                }
            }
            if config.context_ignore.is_empty() {
                stdout.push_str("\n  context ignore: none configured");
            } else {
                stdout.push_str("\n  context ignore:");
                for item in config.context_ignore {
                    stdout.push_str(&format!("\n    - {}", item));
                }
            }
            CommandResult {
                ok: true,
                command: "repodesk project info".into(),
                stdout,
                stderr: "".into(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk project info".into(),
            stdout: "".into(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let task_status = match repodesk_core::tasks::task_status() {
        Ok(info) => {
            let mut stdout = format!(
                "Active task:\n  id: {}\n  title: {}\n  status: {:?}",
                info.config.id, info.config.title, info.config.status
            );
            stdout.push_str(&format!(
                "\n  run directory: {}",
                info.config.run_dir.display()
            ));
            CommandResult {
                ok: true,
                command: "repodesk task status".into(),
                stdout,
                stderr: "".into(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk task status".into(),
            stdout: "".into(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let workflow_hint = match repodesk_core::workflow::workflow_next() {
        Ok(text) => CommandResult {
            ok: true,
            command: "repodesk workflow next".into(),
            stdout: text,
            stderr: "".into(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk workflow next".into(),
            stdout: "".into(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let security_verdict = match repodesk_core::judge::judge_agent("codex") {
        Ok(report) => CommandResult {
            ok: true,
            command: "repodesk judge agent --agent codex".into(),
            stdout: repodesk_core::judge::format_judgement(&report),
            stderr: "".into(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk judge agent --agent codex".into(),
            stdout: "".into(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

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

    let state = repodesk_core::workflow::build_product_workflow_state(
        repodesk_core::workflow::ProductWorkflowStateParams {
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
        },
    );

    if let Ok(mut cache) = WORKFLOW_CACHE.lock() {
        *cache = Some((state.clone(), Instant::now()));
    }

    state
}
