use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub ok: bool,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopAction {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub risk: String,
    pub command_preview: String,
    #[serde(skip_serializing)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRunResult {
    pub id: String,
    pub title: String,
    pub risk: String,
    pub category: String,
    pub started_at_ms: u128,
    pub finished_at_ms: u128,
    pub result: CommandResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub action_id: Option<String>,
    pub artifact_kind: Option<String>,
    pub command_preview: Option<String>,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStatus {
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub exists: bool,
    pub size_bytes: u64,
}

/// Minimal Git facts the workflow engine needs to reason about commit readiness.
/// Kept as plain scalars so the engine stays deterministic and unit-testable
/// without shelling out to `git`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitCommitContext {
    pub is_repo: bool,
    pub is_dirty: bool,
    pub changed_count: usize,
    pub has_conflicts: bool,
    pub branch: Option<String>,
}

/// Verdict on whether the working tree is safe to commit right now.
///
/// This is the culmination of the product workflow: checks passed, no secrets
/// in context, and no new High/Critical findings from RepoPilot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitReadiness {
    /// One of: `ready`, `blocked`, `warning`, `nothing_to_commit`, `not_a_repo`.
    pub status: String,
    /// Short human-readable summary suitable for a CTA/banner.
    pub headline: String,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub is_dirty: bool,
    pub changed_count: usize,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductWorkflowStateParams {
    pub generated_at_ms: u128,
    pub project_info: CommandResult,
    pub task_status: CommandResult,
    pub workflow_hint: CommandResult,
    pub security_verdict: CommandResult,
    pub context: ArtifactStatus,
    pub smart_context: ArtifactStatus,
    pub prompt_codex: ArtifactStatus,
    pub prompt_chatgpt: ArtifactStatus,
    pub prompt_review: ArtifactStatus,
    pub checks_summary: ArtifactStatus,
    pub token_estimate: ArtifactStatus,
    pub checks_summary_preview: Option<String>,
    #[serde(default)]
    pub git: GitCommitContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductWorkflowState {
    pub generated_at_ms: u128,
    pub overall_status: String,
    pub primary_cta: String,
    pub recommended_action_id: Option<String>,
    pub recommended_action_title: Option<String>,
    pub steps: Vec<WorkflowStep>,
    pub artifacts: Vec<ArtifactStatus>,
    pub project_ok: bool,
    pub task_ok: bool,
    pub context_ok: bool,
    pub smart_context_ok: bool,
    pub prompts_ok: bool,
    pub checks_ok: bool,
    pub safety_ok: bool,
    pub commit_readiness: CommitReadiness,
    pub project_info: CommandResult,
    pub task_status: CommandResult,
    pub workflow_hint: CommandResult,
    pub security_verdict: CommandResult,
    pub checks_summary_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactContent {
    pub kind: String,
    pub title: String,
    pub path: String,
    pub exists: bool,
    pub content: String,
    pub size_bytes: u64,
}
