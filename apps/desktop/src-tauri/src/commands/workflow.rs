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
    // Action history now lives in SQLite (migration v2) so it persists with the
    // rest of local state and is covered by backup/restore.
    let _ = repodesk_core::persistence::record_action_run(result);
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

/// Commit the **already-staged, reviewed** changeset — never `git add -A`. The
/// bounded-commit gate lives in core ([`repodesk_core::workflow::commit_reviewed_index`]):
/// it refuses unless the run was accepted and verification is still fresh, and
/// refuses if the index holds anything outside the reviewed set.
#[tauri::command]
pub fn commit_ready_changes(message: String) -> Result<CommandResult, String> {
    let outcome = repodesk_core::workflow::commit_reviewed_index(&message)
        .map_err(|error| error.to_string())?;

    // Drop the cached workflow state so the UI re-reads the new clean tree.
    if let Ok(mut cache) = WORKFLOW_CACHE.lock() {
        *cache = None;
    }

    Ok(CommandResult {
        ok: true,
        command: "git commit".into(),
        stdout: format!(
            "Committed {} ({} file(s))",
            &outcome.commit_sha[..outcome.commit_sha.len().min(12)],
            outcome.committed_paths.len()
        ),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

use repodesk_core::orchestrator::{self, ReviewAction};
use repodesk_core::workflow::{
    Evidence, ExecutionMode, PhaseProgress, derive_progress, derive_signals, load_phase_state,
    set_execution_mode,
};

/// Gather the evidence the phase gates derive from. Scope/Prepare come from the
/// workflow engine's `*_ok` flags; everything past Prepare comes only from the
/// task run receipt + current git state — never a stale run or a stray index.
/// The CLI builds the same `Evidence`, so the two surfaces can't drift.
fn build_evidence() -> Evidence {
    let state = load_phase_state().unwrap_or_default();
    let wf = build_product_workflow_state();
    let receipt = repodesk_core::workflow::load_receipt().ok().flatten();
    let project_path = repodesk_core::projects::get_active_project()
        .ok()
        .map(|project| project.path);

    let head_sha = project_path
        .as_deref()
        .and_then(repodesk_core::workflow::head_sha);
    let index_tree_sha = project_path
        .as_deref()
        .and_then(repodesk_core::workflow::index_tree_sha);
    let finish_commit_exists = match (
        project_path.as_deref(),
        receipt.as_ref().and_then(|r| r.finish.as_ref()),
    ) {
        (Some(path), Some(finish)) => {
            repodesk_core::workflow::commit_exists(path, &finish.commit_sha)
        }
        _ => false,
    };

    Evidence {
        project_ok: wf.project_ok,
        task_ok: wf.task_ok,
        goal_defined: wf.task_ok,
        context_ok: wf.context_ok,
        safety_ok: wf.safety_ok,
        route_ready: wf.smart_context_ok,
        // Real artifact-backed signals (not proxied off smart-context).
        cost_estimated: artifact_status("token_estimate").exists,
        // Baseline checks ran *before* any execution; once a run receipt exists
        // the summary belongs to the final (receipt-bound) Verify phase instead.
        baseline_checks_ran: artifact_status("checks_summary").exists && receipt.is_none(),
        mode: state.execution_mode,
        receipt,
        head_sha,
        index_tree_sha,
        finish_commit_exists,
    }
}

fn current_progress() -> PhaseProgress {
    let evidence = build_evidence();
    let mode = evidence.mode;
    let signals = derive_signals(&evidence);
    derive_progress(&signals, mode)
}

/// The current six-phase progression for the active task: derived phase
/// statuses, the single actionable phase, the one primary CTA, and the chosen
/// execution mode. This is the Work tab's source of truth.
#[tauri::command]
pub async fn work_phase_state() -> Result<PhaseProgress, ErrorPayload> {
    Ok(current_progress())
}

/// Persist the Execute-phase mode (Agent run vs Manual handoff) and return the
/// refreshed progression.
#[tauri::command]
pub async fn work_set_execution_mode(mode: String) -> Result<PhaseProgress, ErrorPayload> {
    let mode = match mode.as_str() {
        "agent_run" => ExecutionMode::AgentRun,
        "manual_handoff" => ExecutionMode::ManualHandoff,
        other => {
            return Err(ErrorPayload::configuration(format!(
                "unknown execution mode '{other}'"
            )));
        }
    };
    set_execution_mode(mode)?;
    Ok(current_progress())
}

/// Review the run's changeset and record the decision atomically: accept stages
/// the run's files and records an Accepted receipt (Review → done, advance to
/// Verify); reject discards them and records a Rejected receipt (re-open
/// Execute). A skipped-file accept fails and Review stays open.
#[tauri::command]
pub async fn work_review(run_id: String, action: String) -> Result<PhaseProgress, ErrorPayload> {
    let action = ReviewAction::from_label(&action).map_err(ErrorPayload::from)?;
    orchestrator::review_run(&run_id, action).map_err(ErrorPayload::from)?;
    Ok(current_progress())
}

/// Import the result of a manual handoff (a pasted unified diff, or the changes
/// the human already applied in the working tree) as run evidence, so the Work
/// flow can advance Execute → Review. The import is secret-scanned before it is
/// recorded; a blocking finding refuses it and leaves the tree untouched-as-run.
#[tauri::command]
pub async fn work_import_manual_changes(
    patch: Option<String>,
) -> Result<PhaseProgress, ErrorPayload> {
    let source = match patch {
        Some(text) if !text.trim().is_empty() => orchestrator::ManualImportSource::Patch(text),
        _ => orchestrator::ManualImportSource::Worktree,
    };
    orchestrator::import_manual_changes(source).map_err(ErrorPayload::from)?;
    Ok(current_progress())
}

/// Run final verification and record a receipt bound to the current run, HEAD,
/// staged index tree, and reviewed changeset (Verify → done while fresh).
#[tauri::command]
pub async fn work_verify() -> Result<PhaseProgress, ErrorPayload> {
    repodesk_core::workflow::run_verification().map_err(ErrorPayload::from)?;
    Ok(current_progress())
}

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

    let snapshot = repodesk_core::git_workspace::build_git_workspace_snapshot();
    let git = repodesk_core::workflow::GitCommitContext {
        is_repo: snapshot.is_git_repo,
        is_dirty: snapshot.is_dirty,
        changed_count: snapshot.changed_files.len(),
        has_conflicts: snapshot
            .changed_files
            .iter()
            .any(|file| file.status_label == "conflict"),
        branch: snapshot.branch,
    };

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
            git,
        },
    );

    if let Ok(mut cache) = WORKFLOW_CACHE.lock() {
        *cache = Some((state.clone(), Instant::now()));
    }

    state
}
