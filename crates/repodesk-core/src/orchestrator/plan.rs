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

/// Whether any step in the plan routes to a paid completion/manual provider.
pub fn plan_has_paid_provider_step(plan: &OrchestrationPlan) -> bool {
    plan.steps.iter().any(|step| {
        let provider_id = step
            .resolved_provider_id()
            .unwrap_or_default()
            .to_ascii_lowercase();
        matches!(
            step.provider.to_ascii_lowercase().as_str(),
            "chatgpt" | "gemini"
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

/// Whether any step in the plan routes to a coding-agent CLI executor.
pub fn plan_has_coding_agent_step(plan: &OrchestrationPlan) -> bool {
    plan.steps.iter().any(|step| {
        matches!(
            step.resolved_executor_id().to_ascii_lowercase().as_str(),
            "codex_cli" | "claude_code_cli"
        )
    })
}

/// Back-compat approval predicate: callers that only have one approval switch
/// should treat paid provider and coding-agent executor steps as gated.
pub fn plan_has_paid_step(plan: &OrchestrationPlan) -> bool {
    plan_has_paid_provider_step(plan) || plan_has_coding_agent_step(plan)
}

/// Provider capacities filtered to providers we can actually call.
pub fn available_capacities(
    settings: &ProviderSettings,
    budget: &BudgetConfig,
) -> Vec<ProviderCapacity> {
    available_capacities_with(settings, budget, |executor| {
        crate::executors::coding_agent_availability(executor)
            .map(|availability| availability.available)
            .unwrap_or(false)
    })
}

fn available_capacities_with(
    settings: &ProviderSettings,
    budget: &BudgetConfig,
    coding_agent_available: impl Fn(&str) -> bool,
) -> Vec<ProviderCapacity> {
    let avail = settings.available_providers();
    let has_openai = avail.contains(&"openai_api");
    let has_gemini = avail.contains(&"gemini_api");
    let has_anthropic = avail.contains(&"anthropic_api");

    default_capacities(budget)
        .into_iter()
        .filter_map(|mut capacity| match capacity.provider.as_str() {
            "ollama" | "local" | "lm_studio" | "llamafile" | "localai" | "local_checks" => {
                Some(capacity)
            }
            "openai_api" if has_openai => Some(capacity),
            "gemini_api" if has_gemini => Some(capacity),
            "anthropic_api" if has_anthropic => Some(capacity),
            "codex_cli" | "claude_code_cli" if coding_agent_available(&capacity.provider) => {
                capacity.auth_status = "cli_on_path".to_string();
                capacity.reachability = "unknown".to_string();
                Some(capacity)
            }
            "manual" => Some(capacity),
            _ => None,
        })
        .collect()
}

/// Find the capacity a manual override names, matching its canonical id against
/// each capacity's `provider` / `executor_id` / `provider_id` (case-insensitive),
/// plus coding-agent aliases (`codex` → `codex_cli`). Returns `None` for a blank
/// or unrecognized override.
fn find_override_capacity<'a>(
    caps: &'a [ProviderCapacity],
    raw: &str,
) -> Option<&'a ProviderCapacity> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let canonical_agent = crate::executors::canonical_coding_agent_id(raw);
    caps.iter().find(|c| {
        let matches = |candidate: &str| candidate.eq_ignore_ascii_case(raw);
        matches(&c.provider)
            || matches(&c.executor_id)
            || c.provider_id.as_deref().is_some_and(matches)
            || canonical_agent.is_some_and(|id| {
                c.executor_id.eq_ignore_ascii_case(id) || c.provider.eq_ignore_ascii_case(id)
            })
    })
}

/// Reject a manual provider/model override that can't be honored, instead of
/// silently fabricating a route. A blank override is a no-op. A non-blank one
/// must name a capacity that exists in the registry, is enabled, and (when a
/// model is given) advertises that model. Per-step write compatibility is
/// handled in [`route_steps`] (incompatible write steps route normally).
pub fn validate_override(
    caps: &[ProviderCapacity],
    override_provider: &Option<String>,
    override_model: &Option<String>,
) -> RepoDeskResult<()> {
    let Some(raw) = override_provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let Some(capacity) = find_override_capacity(caps, raw) else {
        return Err(crate::errors::RepoDeskError::RoutingFailed {
            detail: format!(
                "unknown provider/executor override '{raw}'; it is not in the available capacity registry"
            ),
        });
    };
    if !capacity.enabled {
        return Err(crate::errors::RepoDeskError::RoutingFailed {
            detail: format!(
                "override '{}' is not enabled; enable it before routing to it",
                capacity.label
            ),
        });
    }
    if let Some(model) = override_model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && !capacity.models.is_empty()
        && !capacity
            .models
            .iter()
            .any(|m| m.eq_ignore_ascii_case(model))
    {
        return Err(crate::errors::RepoDeskError::RoutingFailed {
            detail: format!(
                "override model '{model}' is not offered by '{}' (known: {})",
                capacity.label,
                capacity.models.join(", ")
            ),
        });
    }
    Ok(())
}

/// Route the template steps against `caps`, assigning each a provider/model.
/// `bias` is the learned routing nudge from the outcome ledger (empty for a
/// deterministic plan). Pure (no I/O) so it can be unit-tested with injected
/// capacities and bias.
pub fn route_steps(
    caps: &[ProviderCapacity],
    budget: &BudgetConfig,
    bias: &RouteBias,
    override_provider: Option<String>,
    override_model: Option<String>,
) -> Vec<SubAgentTask> {
    // Resolve a manual override to a concrete, real capacity once. Unknown ids
    // never reach here — `build_plan` rejects them via `validate_override` — but
    // `route_steps` is also called directly (tests), so it tolerates a miss by
    // falling back to normal routing rather than fabricating a route.
    let override_capacity = override_provider
        .as_deref()
        .and_then(|raw| find_override_capacity(caps, raw));

    TEMPLATE
        .iter()
        .map(|template| {
            // Apply the override only when it can satisfy this step: a write step
            // requires a coding-agent executor, so a completion/local override is
            // skipped for it (that step routes normally) instead of forcing a
            // patch through an LLM API that cannot write files.
            if let Some(capacity) = override_capacity {
                let write_ok = !template.allow_write
                    || capacity.resolved_executor_kind()
                        == crate::routing::types::ExecutorKind::CodingAgent;
                if write_ok {
                    let model = override_model
                        .clone()
                        .filter(|m| !m.trim().is_empty())
                        .or_else(|| capacity.preferred_model.clone());
                    return SubAgentTask {
                        id: template.id.to_string(),
                        title: template.title.to_string(),
                        kind: template.kind,
                        agent: capacity.resolved_executor_id().to_string(),
                        provider: capacity
                            .provider_id
                            .clone()
                            .unwrap_or_else(|| capacity.provider.clone()),
                        executor_kind: capacity.resolved_executor_kind(),
                        executor_id: capacity.resolved_executor_id().to_string(),
                        provider_id: capacity.provider_id.clone(),
                        model,
                        thinking: template.thinking,
                        instruction: template.instruction.to_string(),
                        depends_on: template.depends_on.iter().map(|d| d.to_string()).collect(),
                        budget_tokens: DEFAULT_STEP_BUDGET,
                        allow_write: template.allow_write,
                    };
                }
            }

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
    override_provider: Option<String>,
    override_model: Option<String>,
) -> RepoDeskResult<OrchestrationPlan> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let goal = goal
        .filter(|g| !g.trim().is_empty())
        .unwrap_or_else(|| task.config.title.clone());
    let budget = load_budget_config()?;
    let caps = available_capacities(settings, &budget);
    // Reject an override that names something not in the registry (or a model the
    // capacity doesn't offer) before building any steps.
    validate_override(&caps, &override_provider, &override_model)?;
    // Learned routing bias from this project's outcome ledger (N8-B). Empty —
    // hence a no-op — until the ledger has enough confirmed/auto signal.
    let bias = crate::outcomes::routing_bias(&project.name).unwrap_or_default();

    Ok(OrchestrationPlan {
        project: project.name,
        task_id: task.config.id,
        goal,
        steps: route_steps(&caps, &budget, &bias, override_provider, override_model),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_all_template_steps_with_a_provider() {
        let budget = BudgetConfig::default();
        let caps = default_capacities(&budget);
        let steps = route_steps(&caps, &budget, &RouteBias::default(), None, None);

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
        let caps = available_capacities_with(&settings, &budget, |_| false);
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
        let caps = available_capacities_with(&settings, &budget, |_| false);
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
        let caps = available_capacities_with(&settings, &budget, |_| false);
        assert!(
            caps.iter().any(|c| c.provider == "openai_api"),
            "openai_api should be present when an OpenAI API key is configured"
        );
        assert!(
            caps.iter().all(|c| c.provider != "codex_cli"),
            "codex_cli is not auto-routable while its CLI executor is missing"
        );
    }

    #[test]
    fn available_capacities_keeps_path_available_coding_agents() {
        let budget = BudgetConfig::default();
        let settings = ProviderSettings::default();
        let caps = available_capacities_with(&settings, &budget, |executor| {
            matches!(executor, "codex_cli")
        });
        let codex = caps
            .iter()
            .find(|capacity| capacity.provider == "codex_cli")
            .expect("codex capacity");
        assert_eq!(
            codex.executor_kind,
            crate::routing::types::ExecutorKind::CodingAgent
        );
        assert_eq!(codex.auth_status, "cli_on_path");
        assert!(caps.iter().all(|c| c.provider != "claude_code_cli"));
    }

    #[test]
    fn plan_predicates_split_paid_provider_and_coding_agent_steps() {
        let budget = BudgetConfig::default();
        let settings = ProviderSettings::default();
        let caps =
            available_capacities_with(&settings, &budget, |executor| executor == "codex_cli");
        let plan = OrchestrationPlan {
            project: "demo".to_string(),
            task_id: "task".to_string(),
            goal: "ship it".to_string(),
            steps: route_steps(&caps, &budget, &RouteBias::default(), None, None),
        };

        assert!(plan_has_coding_agent_step(&plan));
        assert!(!plan_has_paid_provider_step(&plan));
        assert!(plan_has_paid_step(&plan));
    }
}
