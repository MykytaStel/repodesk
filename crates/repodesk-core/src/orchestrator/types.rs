//! Data model for orchestration: the plan of sub-agent tasks, the per-task
//! results, and the aggregated run. Plus a deterministic topological ordering
//! used by the runner so dependencies execute before their dependents.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::api_clients::ThinkingLevel;
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::routing::types::TaskKind;

/// One unit of delegated work: an agent + resolved provider/model + the context
/// it needs, plus the dependencies that must finish before it runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentTask {
    /// Stable id, unique within a plan (used for `depends_on` wiring).
    pub id: String,
    pub title: String,
    pub kind: TaskKind,
    /// Agent name from the registry (ollama, chatgpt, codex, gemini, …).
    pub agent: String,
    /// Provider name the runner builds a client for (resolved by planning).
    pub provider: String,
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
            model: None,
            thinking: ThinkingLevel::None,
            instruction: String::new(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
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
}
