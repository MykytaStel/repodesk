//! Per-sub-agent context assembly. The canonical RepoDesk context is prepared
//! once and reused by every sub-agent; each step then receives only its own
//! assignment plus bounded in-run dependency results.
//!
//! This is intentionally a consumer of `crate::context`, not another context
//! builder. The Context Evidence UI, Execute preview and executed agents should
//! therefore refer to the same selected source set and context fingerprint.

use sha2::{Digest, Sha256};
use tokio::fs;

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

use super::types::{SubAgentResult, SubAgentTask};

/// Cap on how much of each upstream result is forwarded, to keep the pack bounded.
const MAX_UPSTREAM_CHARS: usize = 3_000;

/// Return the prepared canonical packet when both its pipeline evidence and
/// rendered `context.md` agree on the fingerprint. This prevents Execute from
/// silently approving one packet in the UI and launching another. Direct CLI
/// callers or recoverable missing/damaged context fall back to a fresh build.
pub async fn build_base_context() -> RepoDeskResult<String> {
    if let Some(prepared) = load_prepared_context().await {
        return Ok(prepared);
    }

    let result = crate::context::build_context()?;
    Ok(fs::read_to_string(&result.context_file).await?)
}

async fn load_prepared_context() -> Option<String> {
    let task = show_active_task().ok()?;
    let report = crate::engineering::load_context_inspector(&task.config.run_dir).ok()?;
    if report.pipeline_error.is_some() {
        return None;
    }
    let pipeline = report.pipeline?;
    let context_path = task.config.run_dir.join("context.md");
    let content = fs::read_to_string(context_path).await.ok()?;

    (sha256_hex(&content) == pipeline.context_fingerprint).then_some(content)
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// System prompt: the sub-agent's role plus the bounded-context house rules.
pub fn step_system_prompt(step: &SubAgentTask) -> String {
    let write_rule = if step.allow_write {
        "You MAY propose a small, bounded patch."
    } else {
        "Do NOT write code; provide analysis/review only."
    };
    format!(
        "You are the '{agent}' sub-agent in a RepoDesk orchestration. Role: {title}.\n\
         Rules: stay strictly within the bounded context provided; prefer the smallest correct \
         change; never request the full repository; never read, infer, or emit \
         secrets/credentials. {write_rule} Answer concisely and concretely.",
        agent = step.agent,
        title = step.title,
    )
}

/// Compose the full prompt for a sub-agent: the shared canonical base pack, this
/// sub-agent's assignment, and the bounded in-run results of its dependencies.
pub fn compose_step_prompt(
    base: &str,
    goal: &str,
    step: &SubAgentTask,
    upstream: &[&SubAgentResult],
) -> String {
    let mut handoff = String::new();
    for up in upstream {
        let trimmed: String = up.output.chars().take(MAX_UPSTREAM_CHARS).collect();
        let ellipsis = if up.output.chars().count() > MAX_UPSTREAM_CHARS {
            "\n[trimmed]"
        } else {
            ""
        };
        handoff.push_str(&format!(
            "### From '{}' ({})\n{}{}\n\n",
            up.task_id, up.agent, trimmed, ellipsis
        ));
    }
    if handoff.is_empty() {
        handoff.push_str("(none — this is a starting step)\n");
    }

    format!(
        "{base}\n\n---\n\n# Orchestration goal\n\n{goal}\n\n\
         # Your sub-task: {title}\n\n{instruction}\n\n\
         # Upstream sub-agent results\n\n{handoff}",
        title = step.title,
        instruction = step.instruction,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_clients::ThinkingLevel;
    use crate::orchestrator::types::SubAgentStatus;
    use crate::routing::types::{ExecutorKind, TaskKind};

    fn sample_step(allow_write: bool) -> SubAgentTask {
        SubAgentTask {
            id: "implement".to_string(),
            title: "Implement the change".to_string(),
            kind: TaskKind::Patch,
            agent: "ollama".to_string(),
            provider: "ollama".to_string(),
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: "ollama".to_string(),
            provider_id: Some("ollama".to_string()),
            model: None,
            thinking: ThinkingLevel::None,
            instruction: "Do the thing.".to_string(),
            depends_on: vec!["analyze".to_string()],
            verify_command: None,
            budget_tokens: 1_000,
            allow_write,
        }
    }

    fn upstream_result(output: &str) -> SubAgentResult {
        SubAgentResult {
            task_id: "analyze".to_string(),
            agent: "ollama".to_string(),
            provider: "ollama".to_string(),
            model: "llama3.1".to_string(),
            status: SubAgentStatus::Ok,
            output: output.to_string(),
            input_tokens: 10,
            output_tokens: 20,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: Vec::new(),
            diff_path: None,
            workspace: None,
            notes: Vec::new(),
        }
    }

    #[test]
    fn system_prompt_reflects_write_permission() {
        assert!(step_system_prompt(&sample_step(true)).contains("MAY propose"));
        assert!(step_system_prompt(&sample_step(false)).contains("Do NOT write code"));
    }

    #[test]
    fn prompt_includes_base_goal_instruction_and_handoff() {
        let up = upstream_result("the analysis");
        let prompt = compose_step_prompt("CANONICAL CONTEXT", "ship feature X", &sample_step(true), &[&up]);
        assert!(prompt.contains("CANONICAL CONTEXT"));
        assert!(prompt.contains("ship feature X"));
        assert!(prompt.contains("Do the thing."));
        assert!(prompt.contains("From 'analyze'"));
        assert!(prompt.contains("the analysis"));
    }

    #[test]
    fn trims_long_upstream_output() {
        let big = "x".repeat(MAX_UPSTREAM_CHARS + 500);
        let up = upstream_result(&big);
        let prompt = compose_step_prompt("BASE", "goal", &sample_step(false), &[&up]);
        assert!(prompt.contains("[trimmed]"));
    }

    #[test]
    fn no_upstream_marks_starting_step() {
        let prompt = compose_step_prompt("BASE", "goal", &sample_step(false), &[]);
        assert!(prompt.contains("this is a starting step"));
    }

    #[test]
    fn fingerprint_changes_with_packet_content() {
        assert_eq!(sha256_hex("same"), sha256_hex("same"));
        assert_ne!(sha256_hex("same"), sha256_hex("different"));
    }
}
