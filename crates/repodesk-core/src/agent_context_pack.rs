//! Agent context packs for Codex/Claude/Cursor handoffs.
//!
//! The default pack is now a projection of the canonical Context Pipeline. It
//! must not rediscover repository structure or inject extra context that is not
//! visible in Context Evidence. The legacy structural formatter remains public
//! for compatibility, but `build_agent_context_pack` no longer uses it.

use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::RepoDeskResult;
use crate::git_workspace::GitFileChange;
use crate::repo_map::{RepoMap, format_hotspots};
use crate::tasks::{TaskConfig, show_active_task};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContextPackResult {
    pub path: PathBuf,
    pub content: String,
    pub size_bytes: u64,
}

/// Legacy structural formatter input kept for compatibility with callers that
/// still render a standalone structural briefing. New agent execution must use
/// the canonical Context Pipeline via [`build_agent_context_pack`].
#[derive(Debug, Clone)]
pub struct AgentContextPackInput {
    pub project_name: String,
    pub project_path: String,
    pub project_type: String,
    pub main_language: Option<String>,
    pub checks: Vec<String>,
    pub task: TaskConfig,
    pub repo_map: RepoMap,
    pub branch: Option<String>,
    pub last_commit: Option<String>,
    pub changed_files: Vec<GitFileChange>,
    pub artifacts: Vec<PackArtifact>,
}

#[derive(Debug, Clone)]
pub struct PackArtifact {
    pub label: String,
    pub path: String,
    pub exists: bool,
}

/// Build a manual agent handoff from the exact same selected `context.md` that
/// orchestration consumes. `build_context` also persists and validates the
/// matching `context-pipeline.json`; this wrapper adds metadata only, never a
/// second repository/context discovery pass.
pub async fn build_agent_context_pack() -> RepoDeskResult<AgentContextPackResult> {
    crate::init::init_home()?;

    let task = show_active_task()?.config;
    let context = crate::context::build_context()?;
    let canonical = tokio::fs::read_to_string(&context.context_file).await?;
    let fingerprint = sha256_hex(&canonical);
    let pipeline_path = task.run_dir.join("context-pipeline.json");

    let content = format_canonical_agent_context_pack(
        &canonical,
        &context.context_file,
        &pipeline_path.display().to_string(),
        &fingerprint,
        context.estimate.estimated_tokens,
    );

    let path = task.run_dir.join("agent-context-pack.md");
    tokio::fs::write(&path, &content).await?;
    let size_bytes = tokio::fs::metadata(&path).await?.len();

    Ok(AgentContextPackResult {
        path,
        content,
        size_bytes,
    })
}

fn format_canonical_agent_context_pack(
    canonical: &str,
    context_path: &str,
    pipeline_path: &str,
    fingerprint: &str,
    estimated_tokens: usize,
) -> String {
    format!(
        "# RepoDesk Agent Context Pack\n\n\
Generated: `{generated}`\n\
Source: canonical RepoDesk Context Pipeline\n\
Context artifact: `{context_path}`\n\
Pipeline evidence: `{pipeline_path}`\n\
Context fingerprint: `{fingerprint}`\n\
Estimated canonical tokens: `{estimated_tokens}`\n\n\
Boundary: the section below is the exact selected context built by RepoDesk. \
No repository map, memory, file body, Git fact, or project rule is added outside \
that canonical selection.\n\n\
## Canonical selected context\n\n\
{canonical}",
        generated = Utc::now().to_rfc3339(),
    )
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// Legacy structural formatter. Kept so external/internal callers do not break
/// during the migration, but orchestration and the default handoff builder no
/// longer use this parallel context projection.
pub fn format_agent_context_pack(input: &AgentContextPackInput) -> String {
    let mut out = String::new();
    out.push_str("# RepoDesk Agent Context Pack\n\n");
    out.push_str(&format!("Generated: `{}`\n", Utc::now().to_rfc3339()));
    out.push_str(
        "Purpose: legacy structural briefing. Prefer the canonical Context Pipeline for execution.\n",
    );
    out.push_str(
        "Boundary: this pack is structural; it does not include raw repository file contents.\n\n",
    );

    out.push_str("## Task\n\n");
    out.push_str(&format!("- Task id: `{}`\n", input.task.id));
    out.push_str(&format!("- Title: {}\n", input.task.title));
    out.push_str(&format!("- Status: `{:?}`\n", input.task.status));
    out.push_str(&format!("- Run dir: `{}`\n", input.task.run_dir.display()));
    match &input.task.verify_command {
        Some(command) => out.push_str(&format!("- Verify command: `{command}`\n")),
        None => out.push_str("- Verify command: not configured\n"),
    }

    out.push_str("\n## Project\n\n");
    out.push_str(&format!("- Name: `{}`\n", input.project_name));
    out.push_str(&format!("- Path: `{}`\n", input.project_path));
    out.push_str(&format!("- Type: `{}`\n", input.project_type));
    if let Some(language) = &input.main_language {
        out.push_str(&format!("- Main language: `{language}`\n"));
    }

    out.push_str("\n## Repository Map\n\n");
    out.push_str(&format!(
        "- Files scanned: `{}`\n- Directories scanned: `{}`\n- Skipped directories: `{}`\n",
        input.repo_map.files_scanned, input.repo_map.dirs_scanned, input.repo_map.skipped_dirs
    ));
    out.push_str("- Languages:\n");
    if input.repo_map.languages.is_empty() {
        out.push_str("  - none detected\n");
    } else {
        for language in input.repo_map.languages.iter().take(10) {
            out.push_str(&format!(
                "  - {}: {} file(s), {} bytes\n",
                language.label, language.files, language.bytes
            ));
        }
    }
    out.push_str("- Important files:\n");
    if input.repo_map.important_files.is_empty() {
        out.push_str("  - none detected\n");
    } else {
        for file in input.repo_map.important_files.iter().take(25) {
            out.push_str(&format!("  - `{file}`\n"));
        }
    }
    out.push_str("- Hotspots:\n");
    out.push_str(&format_hotspots(&input.repo_map));

    out.push_str("\n## Git State\n\n");
    out.push_str(&format!(
        "- Branch: `{}`\n",
        input.branch.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "- Last commit: `{}`\n",
        input.last_commit.as_deref().unwrap_or("unknown")
    ));
    if input.changed_files.is_empty() {
        out.push_str("- Working tree: clean\n");
    } else {
        out.push_str(&format!(
            "- Working tree: {} changed file(s)\n",
            input.changed_files.len()
        ));
        for file in input.changed_files.iter().take(60) {
            out.push_str(&format!(
                "  - `{}` {} ({})\n",
                file.path, file.status_code, file.status_label
            ));
        }
    }

    out.push_str("\n## RepoDesk Artifacts\n\n");
    for artifact in &input.artifacts {
        let state = if artifact.exists { "ready" } else { "missing" };
        out.push_str(&format!(
            "- {}: `{}` at `{}`\n",
            artifact.label, state, artifact.path
        ));
    }

    out.push_str("\n## Project Checks\n\n");
    if input.checks.is_empty() {
        out.push_str("- No checks configured in RepoDesk.\n");
    } else {
        for check in &input.checks {
            out.push_str(&format!("- `{check}`\n"));
        }
    }

    out.push_str("\n## Operating Rules For The Agent\n\n");
    out.push_str("- Stay within the task title and RepoDesk bounded context.\n");
    out.push_str("- Prefer small, reviewable changes over broad rewrites.\n");
    out.push_str(
        "- Do not read or print secrets, credentials, tokens, keys, or `.env` contents.\n",
    );
    out.push_str("- Before editing, state the files you expect to touch and why.\n");
    out.push_str(
        "- After editing, report changed files, checks run, failures, and residual risk.\n",
    );
    out.push_str(
        "- Do not commit, push, reset, or clean the repository unless the human explicitly asks.\n",
    );

    out
}

fn pack_artifacts(task: &TaskConfig) -> Vec<PackArtifact> {
    [
        ("Task", "task.md"),
        ("Context", "context.md"),
        ("Context pipeline", "context-pipeline.json"),
        ("Smart context", "smart-context.md"),
        ("Codex prompt", "prompt.codex.md"),
        ("ChatGPT prompt", "prompt.chatgpt.md"),
        ("Review prompt", "prompt.review.md"),
        ("Checks summary", "checks-summary.md"),
        ("Token estimate", "token-estimate.txt"),
    ]
    .into_iter()
    .map(|(label, file)| {
        let path = task.run_dir.join(file);
        PackArtifact {
            label: label.to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;

    use super::*;
    use crate::repo_map::LanguageStat;
    use crate::tasks::TaskStatus;

    fn sample_input() -> AgentContextPackInput {
        AgentContextPackInput {
            project_name: "demo".to_string(),
            project_path: "/tmp/demo".to_string(),
            project_type: "rust".to_string(),
            main_language: Some("rust".to_string()),
            checks: vec!["cargo test --workspace".to_string()],
            task: TaskConfig {
                id: "task-1".to_string(),
                project_name: "demo".to_string(),
                title: "Fix auth redirect".to_string(),
                status: TaskStatus::Open,
                verify_command: Some("cargo test --workspace".to_string()),
                run_dir: PathBuf::from("/tmp/repodesk/task-1"),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            repo_map: RepoMap {
                project_name: "demo".to_string(),
                project_path: PathBuf::from("/tmp/demo"),
                files_scanned: 2,
                dirs_scanned: 1,
                skipped_dirs: 0,
                total_bytes: 123,
                languages: vec![LanguageStat {
                    label: "rust".to_string(),
                    files: 1,
                    bytes: 100,
                }],
                hotspots: Vec::new(),
                important_files: vec!["Cargo.toml".to_string()],
            },
            branch: Some("main".to_string()),
            last_commit: Some("abc123 message".to_string()),
            changed_files: Vec::new(),
            artifacts: vec![PackArtifact {
                label: "Context".to_string(),
                path: "/tmp/repodesk/task-1/context.md".to_string(),
                exists: true,
            }],
        }
    }

    #[test]
    fn canonical_handoff_contains_exact_selected_context_without_rediscovery() {
        let canonical = "# RepoDesk Context Pack\n\nONLY SELECTED MATERIAL";
        let text = format_canonical_agent_context_pack(
            canonical,
            "/tmp/context.md",
            "/tmp/context-pipeline.json",
            "abc123",
            321,
        );

        assert!(text.contains(canonical));
        assert!(text.contains("Source: canonical RepoDesk Context Pipeline"));
        assert!(text.contains("Context fingerprint: `abc123`"));
        assert!(text.contains("Estimated canonical tokens: `321`"));
        assert!(!text.contains("## Repository Map"));
    }

    #[test]
    fn legacy_context_pack_is_structural_and_task_scoped() {
        let text = format_agent_context_pack(&sample_input());
        assert!(text.contains("Fix auth redirect"));
        assert!(text.contains("cargo test --workspace"));
        assert!(text.contains("Boundary: this pack is structural"));
        assert!(text.contains("Cargo.toml"));
    }

    #[test]
    fn canonical_fingerprint_is_stable_for_same_context() {
        assert_eq!(sha256_hex("same"), sha256_hex("same"));
        assert_ne!(sha256_hex("same"), sha256_hex("different"));
    }

    #[test]
    fn artifact_inventory_names_context_pipeline() {
        let task = sample_input().task;
        let artifacts = pack_artifacts(&task);
        assert!(artifacts.iter().any(|artifact| artifact.label == "Context pipeline"));
    }
}
