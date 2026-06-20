//! Per-sub-agent context assembly. The shared base pack is built once (reusing
//! the smart-context builder, which already injects the Memory Brain slice), and
//! each sub-agent gets that base plus its own assignment and the in-run results
//! of its dependencies. This is what keeps the orchestrator's own context lean:
//! each sub-agent reasons in its own pack, not in the orchestrator's.

use tokio::fs;

use crate::errors::RepoDeskResult;
use crate::smart_context::build_smart_context;

use super::types::{SubAgentResult, SubAgentTask};

/// Cap on how much of each upstream result is forwarded, to keep the pack bounded.
const MAX_UPSTREAM_CHARS: usize = 3_000;

/// Build the shared base context pack once per run (task + Memory Brain slice +
/// repo map + changed files). Reuses [`build_smart_context`] so every sub-agent
/// reads the same bounded, brain-injected pack.
pub async fn build_base_context() -> RepoDeskResult<String> {
    let result = build_smart_context().await?;
    let content = fs::read_to_string(&result.context_file).await?;
    Ok(content)
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

/// Compose the full prompt for a sub-agent: the shared base pack, this
/// sub-agent's assignment, and the (in-run) results of its dependencies.
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
        let prompt = compose_step_prompt("BASE PACK", "ship feature X", &sample_step(true), &[&up]);
        assert!(prompt.contains("BASE PACK"));
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
}
