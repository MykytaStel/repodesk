//! Import the result of a **manual handoff** back into the Work flow.
//!
//! Manual handoff is the escape hatch: RepoDesk builds a bounded context pack,
//! the human runs an external agent (ChatGPT, a local script, whatever) however
//! they like, then brings the result back. That loop used to be one-way — there
//! was no way to turn the returned changes into the run evidence the six-phase
//! flow needs, so Execute could never advance past "preview only".
//!
//! This closes it. An import produces the *same* evidence an agent run does — a
//! persisted [`OrchestrationRun`] plus a fresh [`TaskRunReceipt`] (mode =
//! `ManualHandoff`) — so the existing Review → Verify → Finish chain, with its
//! accept/reject and receipt-bound gating, works unchanged. It is bounded and
//! safety-gated by construction: changes are secret-scanned *before* they are
//! ever recorded as reviewable, and only paths that are actually dirty in the
//! working tree become the changeset.

use chrono::Utc;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::safety::{SafetyLevel, scan_file, scan_text};
use crate::workflow::ExecutionMode;
use crate::workflow::receipt::{
    ExecutionReceipt, StepReceipt, TaskRunReceipt, changeset_digest, head_sha, save_receipt,
};

use super::review::{git_apply_stdin, is_git_repo};
use super::types::{OrchestrationRun, RunStatus, SubAgentResult, SubAgentStatus};

/// Where the returned changes come from.
pub enum ManualImportSource {
    /// A unified diff/patch produced by the external agent; applied to the tree.
    Patch(String),
    /// The human already applied the edits; record the dirty working tree as-is.
    Worktree,
}

/// The outcome of importing manual changes: the synthetic run the Review phase
/// will act on, plus the files it captured.
#[derive(Debug, Clone)]
pub struct ManualImport {
    pub run_id: String,
    pub changed_files: Vec<String>,
    pub warnings: Vec<String>,
}

/// Import external-agent changes for the active task, producing run + receipt
/// evidence so the Work flow can advance Execute → Review.
///
/// Errors (and records nothing) when there is no git repo, when the patch fails
/// to apply, when no change is detected, or when a secret-scan **block** finding
/// is present in the imported changes.
pub fn import_manual_changes(source: ManualImportSource) -> RepoDeskResult<ManualImport> {
    let project = crate::projects::get_active_project()?;
    let task = crate::tasks::show_active_task()?;
    if !is_git_repo(project.path.as_path()) {
        return Err(RepoDeskError::RoutingFailed {
            detail: "active project is not a git repository; cannot import changes".to_string(),
        });
    }

    // A patch is scanned and applied first, so the working tree reflects the
    // import before we read the dirty set.
    if let ManualImportSource::Patch(patch) = &source {
        if patch.trim().is_empty() {
            return Err(RepoDeskError::RoutingFailed {
                detail: "no patch content to import".to_string(),
            });
        }
        let report = scan_text("manual-handoff patch", patch);
        if report.level == SafetyLevel::Block {
            return Err(block_error(&report));
        }
        // Verify the patch applies cleanly before mutating the tree, then apply.
        git_apply_stdin(project.path.as_path(), &["apply", "--check"], patch)?;
        git_apply_stdin(project.path.as_path(), &["apply"], patch)?;
    }

    // The changeset is whatever is now dirty in the working tree.
    let snapshot = crate::git_workspace::build_git_workspace_snapshot();
    let mut changed_files: Vec<String> = Vec::new();
    for change in &snapshot.changed_files {
        if !changed_files.contains(&change.path) {
            changed_files.push(change.path.clone());
        }
    }
    if changed_files.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "no changes detected in the working tree to import".to_string(),
        });
    }

    // Secret-scan the captured changes before recording them as reviewable. We
    // scan each changed file's current content (a superset of the diff), so a
    // block anywhere in a touched file refuses the import — the edits stay in the
    // working tree for the human to fix, but never become run evidence.
    let mut warnings: Vec<String> = Vec::new();
    let mut blocked: Vec<String> = Vec::new();
    for path in &changed_files {
        let full = project.path.join(path);
        if !full.exists() {
            continue; // deleted file — nothing to scan
        }
        if let Ok(report) = scan_file(&full) {
            match report.level {
                SafetyLevel::Block => blocked.push(path.clone()),
                SafetyLevel::Warning => {
                    warnings.push(format!("{path}: potential sensitive content"))
                }
                SafetyLevel::Ok => {}
            }
        }
    }
    if !blocked.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "secret scan blocked the import — sensitive content in: {}. \
                 The changes remain in your working tree; remove the secrets and re-import.",
                blocked.join(", ")
            ),
        });
    }

    // Build a single-step run + receipt mirroring an agent run, so Review works
    // unchanged. The step is write-capable and Ok, so the execution succeeded.
    let now = Utc::now();
    let run_id = format!("manual-{}", now.format("%Y%m%d-%H%M%S-%6f"));
    let timestamp = now.to_rfc3339();
    let result = SubAgentResult {
        task_id: "manual-handoff".to_string(),
        agent: "manual".to_string(),
        provider: "manual_handoff".to_string(),
        model: "external".to_string(),
        status: SubAgentStatus::Ok,
        output: format!(
            "Imported {} changed file(s) from a manual handoff.",
            changed_files.len()
        ),
        input_tokens: 0,
        output_tokens: 0,
        cost_units: 0.0,
        captured_proposals: 0,
        changed_files: changed_files.clone(),
        diff_path: None,
        workspace: None,
        notes: warnings.clone(),
    };
    let run = OrchestrationRun {
        run_id: run_id.clone(),
        project: project.name.clone(),
        task_id: task.config.id.clone(),
        goal: task.config.title.clone(),
        status: RunStatus::Completed,
        dry_run: false,
        started_at: timestamp.clone(),
        finished_at: timestamp,
        results: vec![result],
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost_units: 0.0,
    };
    persist_run(&task.config.run_dir, &run)?;
    write_manual_receipt(&project.path, &run)?;

    // Manual/external-agent work must live in the same engineering history as
    // managed agent runs. There is no internal dependency plan, so handoffs are
    // omitted while execution/change telemetry is still captured.
    let _ = crate::engineering::instrumentation::record_orchestration_run(None, &run);

    Ok(ManualImport {
        run_id,
        changed_files,
        warnings,
    })
}

fn block_error(report: &crate::safety::SafetyReport) -> RepoDeskError {
    let detail = report
        .findings
        .iter()
        .find(|f| f.level == SafetyLevel::Block)
        .map(|f| f.reason.clone())
        .unwrap_or_else(|| "sensitive content detected".to_string());
    RepoDeskError::RoutingFailed {
        detail: format!("secret scan blocked the import: {detail}"),
    }
}

/// Persist the synthetic run as `{run_id}.json` + `latest.json`, mirroring the
/// orchestrator runner's layout so `orchestrate_status` / `load_run` find it.
fn persist_run(run_dir: &std::path::Path, run: &OrchestrationRun) -> RepoDeskResult<()> {
    let dir = run_dir.join("orchestrate");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(run)?;
    std::fs::write(dir.join(format!("{}.json", run.run_id)), &json)?;
    std::fs::write(dir.join("latest.json"), &json)?;
    Ok(())
}

/// Write a fresh manual-mode receipt — the execution evidence the phase gates
/// derive from. `execution_mode = ManualHandoff` is what lets `derive_signals`
/// honor this run in Manual mode (and only in Manual mode).
fn write_manual_receipt(
    project_path: &std::path::Path,
    run: &OrchestrationRun,
) -> RepoDeskResult<()> {
    let changed: Vec<String> = run
        .results
        .iter()
        .flat_map(|r| r.changed_files.clone())
        .collect();
    let digest = if changed.is_empty() {
        None
    } else {
        Some(changeset_digest(&changed))
    };
    let receipt = TaskRunReceipt {
        task_id: run.task_id.clone(),
        run_id: run.run_id.clone(),
        execution_mode: ExecutionMode::ManualHandoff,
        base_commit: head_sha(project_path),
        execution: ExecutionReceipt {
            status: run.status,
            required_steps: run
                .results
                .iter()
                .map(|r| StepReceipt {
                    task_id: r.task_id.clone(),
                    status: r.status,
                    allow_write: true,
                    changed_files: r.changed_files.clone(),
                    change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
                })
                .collect(),
            changeset_digest: digest,
        },
        review: None,
        verification: None,
        finish: None,
    };
    save_receipt(&receipt)
}
