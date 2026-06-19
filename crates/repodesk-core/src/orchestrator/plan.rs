//! Build a sub-agent plan for the active task. Each step is routed through the
//! existing [`crate::routing`] engine, so the route's executor/provider identity
//! drives per-task model selection — the cost lever. Provider capacities are
//! filtered to those we can actually call (local runtimes always; paid
//! completion providers only when their key is configured), so routing never
//! mistakes a coding-agent executor for an LLM API client.

use crate::api_clients::{ProviderSettings, ThinkingLevel};
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::routing::engine::{default_capacities, route_request_with_bias};
use crate::routing::types::{ProviderCapacity, RouteBias, RouteRequest, TaskKind};
use crate::tasks::show_active_task;
use crate::usage::budget::{BudgetConfig, load_budget_config};

use super::types::{OrchestrationPlan, SubAgentTask};

/// Default per-sub-agent output token cap.
const DEFAULT_STEP_BUDGET: usize = 1_500;
/// Nominal input size used to route a step before its real context is built.
const NOMINAL_INPUT_TOKENS: usize = 4_000;

/// A step template, before routing assigns a concrete provider/model.
struct StepTemplate {
    id: &'static str,
    title: &'static str,
    kind: TaskKind,
    thinking: ThinkingLevel,
    allow_write: bool,
    depends_on: &'static [&'static str],
    instruction: &'static str,
}

/// The default analyze → implement → review pipeline.
const TEMPLATE: &[StepTemplate] = &[
    StepTemplate {
        id: "analyze",
        title: "Analyze the task and outline an approach",
        kind: TaskKind::Plan,
        thinking: ThinkingLevel::Low,
        allow_write: false,
        depends_on: &[],
        instruction: "Read the task and bounded context. Produce a short, concrete plan: the key decisions, the risks, and the smallest set of changes needed. Do not write code yet.",
    },
    StepTemplate {
        id: "implement",
        title: "Implement the change",
        kind: TaskKind::Patch,
        thinking: ThinkingLevel::None,
        allow_write: true,
        depends_on: &["analyze"],
        instruction: "Using the analysis above, produce a small, bounded patch or concrete implementation steps. Prefer minimal diffs. Do not touch secrets or unrelated files.",
    },
    StepTemplate {
        id: "review",
        title: "Independent review",
        kind: TaskKind::Review,
        thinking: ThinkingLevel::Medium,
        allow_write: false,
        depends_on: &["implement"],
        instruction: "Independently review the proposed change for correctness, risks, and missed cases. Be concise and specific; call out anything that should block merging.",
    },
];

/// Whether any step in the plan routes to a paid provider — the signal for the
/// confirm-before-paid gate (CLI `--yes`, desktop confirm, autonomous-loop pause).
pub fn plan_has_paid_step(plan: &OrchestrationPlan) -> bool {
    plan.steps.iter().any(|step| {
        let executor_id = step.resolved_executor_id().to_ascii_lowercase();
        let provider_id = step
            .resolved_provider_id()
            .unwrap_or_default()
            .to_ascii_lowercase();
        matches!(
            executor_id.as_str(),
            "codex_cli" | "claude_code_cli" | "chatgpt" | "gemini"
        ) || matches!(
            provider_id.as_str(),
            "openai_api"
                | "anthropic_api"
                | "gemini_api"
                | "openai"
                | "chatgpt"
                | "gpt"
                | "anthropic"
                | "gemini"
        )
    })
}

/// Provider capacities filtered to providers we can actually call.
pub fn available_capacities(
    settings: &ProviderSettings,
    budget: &BudgetConfig,
) -> Vec<ProviderCapacity> {
    let avail = settings.available_providers();
    let has_openai = avail.contains(&"openai_api");
    let has_gemini = avail.contains(&"gemini_api");
    let has_anthropic = avail.contains(&"anthropic_api");

    default_capacities(budget)
        .into_iter()
        .filter(|capacity| match capacity.provider.as_str() {
            "ollama" | "local" | "lm_studio" | "llamafile" | "localai" | "local_checks" => true,
            "openai_api" => has_openai,
            "gemini_api" => has_gemini,
            "anthropic_api" => has_anthropic,
            // First PR only separates identities. CLI coding-agent execution is
            // introduced later, so the orchestrator does not auto-route to it.
            "codex_cli" | "claude_code_cli" => false,
            _ => true,
        })
        .collect()
}

/// Route the template steps against `caps`, assigning each a provider/model.
/// `bias` is the learned routing nudge from the outcome ledger (empty for a
/// deterministic plan). Pure (no I/O) so it can be unit-tested with injected
/// capacities and bias.
pub fn route_steps(
    caps: &[ProviderCapacity],
    budget: &BudgetConfig,
    bias: &RouteBias,
) -> Vec<SubAgentTask> {
    TEMPLATE
        .iter()
        .map(|template| {
            let request = RouteRequest {
                task_kind: template.kind,
                estimated_input_tokens: NOMINAL_INPUT_TOKENS,
                estimated_output_tokens: DEFAULT_STEP_BUDGET,
                risk_level: if template.allow_write {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                changed_file_count: 0,
                requires_write: template.allow_write,
                context_safe: Some(true),
                checks_ok: None,
                guard_allowed: Some(true),
                git_dirty: None,
                max_cost_units: None,
                economy_mode: None,
            };
            let decision = route_request_with_bias(&request, caps, budget, bias);
            let executor_id = decision.recommended_executor_id.clone();
            let provider_id = decision.recommended_provider_id.clone();
            SubAgentTask {
                id: template.id.to_string(),
                title: template.title.to_string(),
                kind: template.kind,
                agent: executor_id.clone(),
                provider: provider_id
                    .clone()
                    .unwrap_or_else(|| decision.recommended_provider.clone()),
                executor_kind: decision.recommended_executor_kind,
                executor_id,
                provider_id,
                model: decision.recommended_model,
                thinking: template.thinking,
                instruction: template.instruction.to_string(),
                depends_on: template.depends_on.iter().map(|d| d.to_string()).collect(),
                budget_tokens: DEFAULT_STEP_BUDGET,
                allow_write: template.allow_write,
            }
        })
        .collect()
}

/// Build a plan for the active project + task. `goal` overrides the task title.
pub fn build_plan(
    goal: Option<String>,
    settings: &ProviderSettings,
) -> RepoDeskResult<OrchestrationPlan> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let goal = goal
        .filter(|g| !g.trim().is_empty())
        .unwrap_or_else(|| task.config.title.clone());
    let budget = load_budget_config()?;
    let caps = available_capacities(settings, &budget);
    // Learned routing bias from this project's outcome ledger (N8-B). Empty —
    // hence a no-op — until the ledger has enough confirmed/auto signal.
    let bias = crate::outcomes::routing_bias(&project.name).unwrap_or_default();

    Ok(OrchestrationPlan {
        project: project.name,
        task_id: task.config.id,
        goal,
        steps: route_steps(&caps, &budget, &bias),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_all_template_steps_with_a_provider() {
        let budget = BudgetConfig::default();
        let caps = default_capacities(&budget);
        let steps = route_steps(&caps, &budget, &RouteBias::default());

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].id, "analyze");
        assert_eq!(steps[1].id, "implement");
        assert_eq!(steps[2].id, "review");
        for step in &steps {
            assert!(
                !step.provider.is_empty(),
                "step {} has no provider",
                step.id
            );
        }
        assert!(steps[0].depends_on.is_empty());
        assert_eq!(steps[1].depends_on, vec!["analyze".to_string()]);
        assert_eq!(steps[2].depends_on, vec!["implement".to_string()]);
        assert!(steps[1].allow_write, "implement step should allow writes");
        assert!(!steps[2].allow_write, "review step should be read-only");
    }

    #[test]
    fn available_capacities_drops_unkeyed_paid_providers() {
        let budget = BudgetConfig::default();
        let settings = ProviderSettings::default();
        let caps = available_capacities(&settings, &budget);
        // With no keys, no paid provider survives (ollama / local_checks / manual stay).
        let paid = ["openai_api", "gemini_api", "anthropic_api"];
        assert!(caps.iter().all(|c| !paid.contains(&c.provider.as_str())));
        assert!(caps.iter().all(|c| c.provider != "codex_cli"));
        assert!(caps.iter().any(|c| c.provider == "ollama"));
    }

    #[test]
    fn lm_studio_is_offered_as_a_local_capacity_without_a_key() {
        let budget = BudgetConfig::default();
        let settings = ProviderSettings::default();
        let caps = available_capacities(&settings, &budget);
        // LM Studio is local: present even with no provider keys configured.
        assert!(
            caps.iter().any(|c| c.provider == "lm_studio"),
            "lm_studio should be a routable local capacity"
        );
    }

    #[test]
    fn available_capacities_keeps_keyed_paid_providers() {
        let budget = BudgetConfig::default();
        let mut settings = ProviderSettings::default();
        settings.openai.api_key = Some("sk-test".to_string());
        let caps = available_capacities(&settings, &budget);
        assert!(
            caps.iter().any(|c| c.provider == "openai_api"),
            "openai_api should be present when an OpenAI API key is configured"
        );
        assert!(
            caps.iter().all(|c| c.provider != "codex_cli"),
            "codex_cli is not auto-routable until a CLI executor exists"
        );
    }
}
