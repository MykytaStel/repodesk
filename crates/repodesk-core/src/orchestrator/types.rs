//! Data model for orchestration: the plan of sub-agent tasks, the per-task
//! results, and the aggregated run. Plus a deterministic topological ordering
//! used by the runner so dependencies execute before their dependents.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::api_clients::ThinkingLevel;
use crate::change_evidence::ChangeEvidenceStatus;
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::routing::types::{ExecutorKind, TaskKind};
use crate::worktree::RunWorktree;

/// One unit of delegated work: an agent + resolved provider/model + the context
/// it needs, plus the dependencies that must finish before it runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentTask {
    /// Stable id, unique within a plan (used for `depends_on` wiring).
    pub id: String,
    pub title: String,
    pub kind: TaskKind,
    /// Legacy agent/accounting id. New code should prefer `executor_id`.
    pub agent: String,
    /// Legacy provider/display id. New code should prefer `provider_id` for
    /// completion calls and `executor_id` for coding-agent routes.
    pub provider: String,
    #[serde(default)]
    pub executor_kind: ExecutorKind,
    #[serde(default)]
    pub executor_id: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    /// Concrete model id; `None` lets the provider client pick its default.
    pub model: Option<String>,
    #[serde(default)]
    pub thinking: ThinkingLevel,
    /// What this sub-agent should do, appended to its context pack.
    pub instruction: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Output token cap for this sub-agent's call.
    pub budget_tokens: usize,
    /// Whether the sub-agent is allowed to propose file writes/patches.
    #[serde(default)]
    pub allow_write: bool,
    /// Optional command to run inside the worktree after an agent completes writes to verify them.
    #[serde(default)]
    pub verify_command: Option<String>,
}

impl SubAgentTask {
    pub fn resolved_executor_kind(&self) -> ExecutorKind {
        let id = self.resolved_executor_id().to_ascii_lowercase();
        match id.as_str() {
            "manual" => ExecutorKind::Manual,
            "local_checks" => ExecutorKind::CheckRunner,
            "codex" | "codex_cli" | "claude" | "claude_code" | "claude_code_cli" => {
                ExecutorKind::CodingAgent
            }
            "ollama" | "lm_studio" | "llamafile" | "localai" => ExecutorKind::LocalRuntime,
            _ => self.executor_kind,
        }
    }

    pub fn resolved_executor_id(&self) -> &str {
        let executor_id = self.executor_id.trim();
        if executor_id.is_empty() {
            self.agent.as_str()
        } else {
            self.executor_id.as_str()
        }
    }

    pub fn resolved_provider_id(&self) -> Option<&str> {
        self.provider_id
            .as_deref()
            .filter(|provider| !provider.trim().is_empty())
            .or_else(|| {
                if self.provider.trim().is_empty() {
                    None
                } else {
                    Some(self.provider.as_str())
                }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    /// Ran (or, in a dry run, would run) successfully.
    Ok,
    /// Not executed — a dependency didn't complete, or this was a dry run.
    Skipped,
    /// A guard, safety scan, budget, or cost ceiling stopped it.
    Blocked,
    /// The provider call errored.
    Failed,
}

impl SubAgentStatus {
    /// True when downstream dependents may proceed.
    pub fn is_success(&self) -> bool {
        matches!(self, SubAgentStatus::Ok)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    pub task_id: String,
    pub agent: String,
    pub provider: String,
    pub model: String,
    pub status: SubAgentStatus,
    pub output: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_units: f64,
    /// How many Memory Brain capture proposals this output produced.
    pub captured_proposals: usize,
    /// Repo-relative paths a coding-agent step changed (empty for every other
    /// executor kind). The reviewable changeset for an accept/reject decision.
    #[serde(default)]
    pub changed_files: Vec<String>,
    /// Whether `changed_files` is complete changeset evidence. Historical run
    /// JSON without this field remains conservative (`legacy_unknown`).
    #[serde(default)]
    pub change_evidence_status: ChangeEvidenceStatus,
    /// Bounded, secret-redacted execution diagnostics that must survive the
    /// executor boundary without being flattened into misleading notes.
    #[serde(default)]
    pub execution_issues: Vec<String>,
    /// Receipt file holding the full captured diff for a coding-agent step.
    #[serde(default)]
    pub diff_path: Option<String>,
    /// Isolated workspace used for this coding-agent step, when one was
    /// created. Review uses this metadata to apply changes back deliberately,
    /// never by assuming same-named active-checkout paths hold the agent output.
    #[serde(default)]
    pub workspace: Option<RunWorktree>,
    pub notes: Vec<String>,
}

/// A plan of sub-agent tasks for the active project + task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    pub project: String,
    pub task_id: String,
    pub goal: String,
    pub steps: Vec<SubAgentTask>,
}

impl OrchestrationPlan {
    /// Steps in dependency order (a dependency always precedes its dependents).
    pub fn ordered(&self) -> RepoDeskResult<Vec<&SubAgentTask>> {
        topological_order(&self.steps).map(|order| order.iter().map(|&i| &self.steps[i]).collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    Partial,
    Failed,
    DryRun,
}

/// The aggregated outcome of executing a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationRun {
    pub run_id: String,
    pub project: String,
    pub task_id: String,
    pub goal: String,
    pub status: RunStatus,
    pub dry_run: bool,
    pub started_at: String,
    pub finished_at: String,
    pub results: Vec<SubAgentResult>,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_cost_units: f64,
}

impl OrchestrationRun {
    /// A lightweight summary for history lists — identity, status, and totals
    /// without the per-step outputs (which can be large).
    pub fn summary(&self) -> RunSummary {
        RunSummary {
            run_id: self.run_id.clone(),
            goal: self.goal.clone(),
            status: self.status,
            dry_run: self.dry_run,
            started_at: self.started_at.clone(),
            finished_at: self.finished_at.clone(),
            step_count: self.results.len(),
            total_cost_units: self.total_cost_units,
        }
    }
}

/// A run boiled down to what a history list needs — no per-step outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub goal: String,
    pub status: RunStatus,
    pub dry_run: bool,
    pub started_at: String,
    pub finished_at: String,
    pub step_count: usize,
    pub total_cost_units: f64,
}

/// Kahn's algorithm: return step indices in dependency order. Errors on an
/// unknown `depends_on` id or a dependency cycle.
pub fn topological_order(steps: &[SubAgentTask]) -> RepoDeskResult<Vec<usize>> {
    let index_of: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    if index_of.len() != steps.len() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "orchestration plan has duplicate step ids".to_string(),
        });
    }

    let mut indegree = vec![0usize; steps.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); steps.len()];

    for (i, step) in steps.iter().enumerate() {
        for dep in &step.depends_on {
            let &dep_idx =
                index_of
                    .get(dep.as_str())
                    .ok_or_else(|| RepoDeskError::RoutingFailed {
                        detail: format!("step '{}' depends on unknown step '{}'", step.id, dep),
                    })?;
            indegree[i] += 1;
            dependents[dep_idx].push(i);
        }
    }

    // Seed with zero-indegree steps, preserving declaration order for stability.
    let mut queue: VecDeque<usize> = (0..steps.len()).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(steps.len());
    let mut visited: HashSet<usize> = HashSet::new();

    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) {
            continue;
        }
        order.push(node);
        for &next in &dependents[node] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    if order.len() != steps.len() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "orchestration plan has a dependency cycle".to_string(),
        });
    }

    Ok(order)
}

/// Group steps into dependency *waves* (layered Kahn): wave 0 is every step
/// with no dependencies; each later wave is the steps whose dependencies all
/// landed in earlier waves. Steps within a wave are independent of one another,
/// so the runner can execute them concurrently. Indices within a wave are
/// ascending (declaration order) for deterministic gating. Errors on the same
/// conditions as [`topological_order`] (unknown dep, duplicate id, cycle).
pub fn dependency_waves(steps: &[SubAgentTask]) -> RepoDeskResult<Vec<Vec<usize>>> {
    let index_of: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    if index_of.len() != steps.len() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "orchestration plan has duplicate step ids".to_string(),
        });
    }

    let mut indegree = vec![0usize; steps.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); steps.len()];
    for (i, step) in steps.iter().enumerate() {
        for dep in &step.depends_on {
            let &dep_idx =
                index_of
                    .get(dep.as_str())
                    .ok_or_else(|| RepoDeskError::RoutingFailed {
                        detail: format!("step '{}' depends on unknown step '{}'", step.id, dep),
                    })?;
            indegree[i] += 1;
            dependents[dep_idx].push(i);
        }
    }

    let mut remaining = steps.len();
    let mut current: Vec<usize> = (0..steps.len()).filter(|&i| indegree[i] == 0).collect();
    let mut waves = Vec::new();

    while !current.is_empty() {
        current.sort_unstable();
        remaining -= current.len();
        let mut next = Vec::new();
        for &node in &current {
            for &dependent in &dependents[node] {
                indegree[dependent] -= 1;
                if indegree[dependent] == 0 {
                    next.push(dependent);
                }
            }
        }
        waves.push(std::mem::take(&mut current));
        current = next;
    }

    if remaining != 0 {
        return Err(RepoDeskError::RoutingFailed {
            detail: "orchestration plan has a dependency cycle".to_string(),
        });
    }

    Ok(waves)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, deps: &[&str]) -> SubAgentTask {
        SubAgentTask {
            id: id.to_string(),
            title: id.to_string(),
            kind: TaskKind::Plan,
            agent: "ollama".to_string(),
            provider: "ollama".to_string(),
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: "ollama".to_string(),
            provider_id: Some("ollama".to_string()),
            model: None,
            thinking: ThinkingLevel::None,
            instruction: String::new(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            verify_command: None,
            budget_tokens: 1_000,
            allow_write: false,
        }
    }

    #[test]
    fn orders_dependencies_before_dependents() {
        let steps = vec![
            step("review", &["implement"]),
            step("implement", &["analyze"]),
            step("analyze", &[]),
        ];
        let order = topological_order(&steps).unwrap();
        let names: Vec<&str> = order.iter().map(|&i| steps[i].id.as_str()).collect();
        let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
        assert!(pos("analyze") < pos("implement"));
        assert!(pos("implement") < pos("review"));
    }

    #[test]
    fn detects_cycles() {
        let steps = vec![step("a", &["b"]), step("b", &["a"])];
        assert!(matches!(
            topological_order(&steps),
            Err(RepoDeskError::RoutingFailed { .. })
        ));
    }

    #[test]
    fn detects_unknown_dependency() {
        let steps = vec![step("a", &["ghost"])];
        assert!(matches!(
            topological_order(&steps),
            Err(RepoDeskError::RoutingFailed { .. })
        ));
    }

    #[test]
    fn independent_steps_keep_declaration_order() {
        let steps = vec![step("first", &[]), step("second", &[]), step("third", &[])];
        let order = topological_order(&steps).unwrap();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn dependency_waves_group_independent_steps_together() {
        // Diamond: a -> {b, c} -> d. Waves: [a], [b, c], [d].
        let steps = vec![
            step("a", &[]),
            step("b", &["a"]),
            step("c", &["a"]),
            step("d", &["b", "c"]),
        ];
        let waves = dependency_waves(&steps).unwrap();
        assert_eq!(waves, vec![vec![0], vec![1, 2], vec![3]]);
    }

    #[test]
    fn dependency_waves_detect_cycles() {
        let steps = vec![step("a", &["b"]), step("b", &["a"])];
        assert!(matches!(
            dependency_waves(&steps),
            Err(RepoDeskError::RoutingFailed { .. })
        ));
    }
}
