use super::*;
use crate::store;

fn artifact_token_estimate(snapshot: &TokenUsageSnapshot, kind: &str) -> Option<usize> {
    snapshot
        .active_artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .and_then(|artifact| artifact.estimated_tokens)
}

fn infer_route_task_kind(
    workflow: &ProductWorkflowState,
    git: &repodesk_core::git_workspace::GitWorkspaceSnapshot,
) -> repodesk_core::routing::TaskKind {
    let action = workflow
        .recommended_action_id
        .as_deref()
        .unwrap_or_default();

    if !workflow.project_ok || !workflow.task_ok {
        return repodesk_core::routing::TaskKind::Manual;
    }
    if action.contains("check") {
        return repodesk_core::routing::TaskKind::Checks;
    }
    if action.contains("smart-context") {
        return repodesk_core::routing::TaskKind::Compress;
    }
    if action.contains("safety") {
        return repodesk_core::routing::TaskKind::Review;
    }
    if workflow.smart_context_ok
        && workflow.prompts_ok
        && workflow.checks_ok
        && !git.changed_files.is_empty()
    {
        return repodesk_core::routing::TaskKind::Patch;
    }
    if workflow.smart_context_ok {
        return repodesk_core::routing::TaskKind::Review;
    }

    repodesk_core::routing::TaskKind::Plan
}

fn default_output_tokens(kind: &repodesk_core::routing::TaskKind) -> usize {
    match kind {
        repodesk_core::routing::TaskKind::Compress
        | repodesk_core::routing::TaskKind::Summarize => 1_200,
        repodesk_core::routing::TaskKind::Plan
        | repodesk_core::routing::TaskKind::Review
        | repodesk_core::routing::TaskKind::Debug => 1_800,
        repodesk_core::routing::TaskKind::Patch => 3_500,
        repodesk_core::routing::TaskKind::Checks | repodesk_core::routing::TaskKind::Manual => 0,
    }
}

pub(crate) fn build_default_route_request(
    workflow: &ProductWorkflowState,
    tokens: &TokenUsageSnapshot,
    git: &repodesk_core::git_workspace::GitWorkspaceSnapshot,
    economy_mode: Option<String>,
) -> repodesk_core::routing::RouteRequest {
    let task_kind = infer_route_task_kind(workflow, git);
    let estimated_input_tokens = artifact_token_estimate(tokens, "smart_context")
        .or_else(|| artifact_token_estimate(tokens, "context"))
        .unwrap_or(0);
    let risk_level = if has_block_signal(&workflow.security_verdict) {
        "block"
    } else if has_warn_signal(&workflow.security_verdict) {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let requires_write = task_kind == repodesk_core::routing::TaskKind::Patch;

    repodesk_core::routing::RouteRequest {
        estimated_output_tokens: default_output_tokens(&task_kind),
        task_kind,
        estimated_input_tokens,
        risk_level,
        changed_file_count: git.changed_files.len(),
        requires_write,
        context_safe: Some(workflow.safety_ok),
        checks_ok: Some(workflow.checks_ok),
        guard_allowed: Some(workflow.safety_ok),
        git_dirty: Some(git.is_dirty),
        max_cost_units: None,
        economy_mode,
    }
}

fn cost_agent_for_provider(provider: &str) -> String {
    match provider {
        "openai_api" | "openai" | "chatgpt" => "openai_api".to_string(),
        "anthropic_api" | "anthropic" => "anthropic_api".to_string(),
        "codex_cli" | "codex" => "codex_cli".to_string(),
        "gemini_api" | "gemini" => "gemini_api".to_string(),
        "ollama" | "lm_studio" | "llamafile" | "localai" => "ollama".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

fn estimated_route_cost_units(
    cost_config: &repodesk_core::usage::cost::CostConfig,
    provider: &str,
    kind: &repodesk_core::routing::ProviderKind,
    request: &repodesk_core::routing::RouteRequest,
) -> f64 {
    if matches!(
        kind,
        repodesk_core::routing::ProviderKind::Local
            | repodesk_core::routing::ProviderKind::CheckRunner
            | repodesk_core::routing::ProviderKind::Manual
    ) {
        return 0.0;
    }

    let cost_agent = cost_agent_for_provider(provider);
    repodesk_core::usage::cost::estimate_agent_cost(
        cost_config,
        &cost_agent,
        request.estimated_input_tokens,
        request.estimated_output_tokens,
    )
    .estimated_cost_units
}

#[allow(clippy::too_many_arguments)]
fn route_capacity_from_health(
    provider: &ProviderHealth,
    kind: repodesk_core::routing::ProviderKind,
    preferred_model: Option<String>,
    daily_remaining_tokens: usize,
    cost_config: &repodesk_core::usage::cost::CostConfig,
    budget_config: &repodesk_core::usage::budget::BudgetConfig,
    request: &repodesk_core::routing::RouteRequest,
    paid_agents_allowed: bool,
) -> repodesk_core::routing::ProviderCapacity {
    let (route_id, executor_kind, executor_id, provider_id) = match provider.id.as_str() {
        "openai_api" | "openai" => (
            "openai_api".to_string(),
            repodesk_core::routing::ExecutorKind::CompletionProvider,
            "openai_api".to_string(),
            Some("openai_api".to_string()),
        ),
        "anthropic_api" | "anthropic" => (
            "anthropic_api".to_string(),
            repodesk_core::routing::ExecutorKind::CompletionProvider,
            "anthropic_api".to_string(),
            Some("anthropic_api".to_string()),
        ),
        "gemini_api" | "gemini" => (
            "gemini_api".to_string(),
            repodesk_core::routing::ExecutorKind::CompletionProvider,
            "gemini_api".to_string(),
            Some("gemini_api".to_string()),
        ),
        _ => (
            provider.id.clone(),
            kind.default_executor_kind(),
            provider.id.clone(),
            Some(provider.id.clone()),
        ),
    };
    let models = provider
        .models
        .iter()
        .filter(|model| model.available)
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    let estimated_cost_units = estimated_route_cost_units(cost_config, &route_id, &kind, request);

    repodesk_core::routing::ProviderCapacity {
        provider: route_id,
        label: provider.label.clone(),
        kind,
        executor_kind,
        executor_id,
        provider_id,
        enabled: provider.enabled,
        auth_status: provider.auth_status.clone(),
        reachability: provider.reachability.clone(),
        models,
        preferred_model,
        daily_remaining_tokens,
        estimated_cost_units,
        quota_status: repodesk_core::routing::QuotaStatus::Available,
        paid_agents_allowed,
        max_patch_files: budget_config.max_files_for_patch_agent,
    }
}

#[allow(clippy::too_many_arguments)]
fn manual_route_capacity(
    provider: &str,
    label: &str,
    kind: repodesk_core::routing::ProviderKind,
    enabled: bool,
    reachability: &str,
    model: Option<&str>,
    quota_status: repodesk_core::routing::QuotaStatus,
    daily_remaining_tokens: usize,
    cost_config: &repodesk_core::usage::cost::CostConfig,
    budget_config: &repodesk_core::usage::budget::BudgetConfig,
    request: &repodesk_core::routing::RouteRequest,
    paid_agents_allowed: bool,
) -> repodesk_core::routing::ProviderCapacity {
    let models = model
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let estimated_cost_units = estimated_route_cost_units(cost_config, provider, &kind, request);

    repodesk_core::routing::ProviderCapacity {
        provider: provider.to_string(),
        label: label.to_string(),
        kind,
        executor_kind: match kind {
            repodesk_core::routing::ProviderKind::PatchAgent => {
                repodesk_core::routing::ExecutorKind::CodingAgent
            }
            repodesk_core::routing::ProviderKind::CheckRunner => {
                repodesk_core::routing::ExecutorKind::CheckRunner
            }
            repodesk_core::routing::ProviderKind::Manual => {
                repodesk_core::routing::ExecutorKind::Manual
            }
            repodesk_core::routing::ProviderKind::Paid => {
                repodesk_core::routing::ExecutorKind::Manual
            }
            repodesk_core::routing::ProviderKind::Local => {
                repodesk_core::routing::ExecutorKind::LocalRuntime
            }
        },
        executor_id: provider.to_string(),
        provider_id: if matches!(
            kind,
            repodesk_core::routing::ProviderKind::Local
                | repodesk_core::routing::ProviderKind::Paid
        ) && !matches!(provider, "chatgpt" | "gemini")
        {
            Some(provider.to_string())
        } else {
            None
        },
        enabled,
        auth_status: if enabled { "manual" } else { "disabled" }.into(),
        reachability: if enabled { reachability } else { "disabled" }.into(),
        models,
        preferred_model: model.map(str::to_string),
        daily_remaining_tokens,
        estimated_cost_units,
        quota_status,
        paid_agents_allowed,
        max_patch_files: budget_config.max_files_for_patch_agent,
    }
}

fn build_routing_capacities(
    settings: &store::ProviderSettings,
    model_health: &ModelHealthSnapshot,
    tokens: &TokenUsageSnapshot,
    budget_config: &repodesk_core::usage::budget::BudgetConfig,
    cost_config: &repodesk_core::usage::cost::CostConfig,
    request: &repodesk_core::routing::RouteRequest,
) -> Vec<repodesk_core::routing::ProviderCapacity> {
    let mut capacities = Vec::new();
    let daily_remaining_tokens = tokens.totals.remaining_daily_tokens;
    let custom_providers =
        repodesk_core::custom_providers::list_custom_providers().unwrap_or_default();

    for provider in &model_health.providers {
        let custom_provider = custom_providers
            .iter()
            .find(|custom| custom.id.eq_ignore_ascii_case(&provider.id));
        let capacity = match provider.id.as_str() {
            "ollama" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Local,
                Some(settings.ollama_model.clone()),
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "lm_studio" | "llamafile" | "localai" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Local,
                None,
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "openai_api" | "openai" => Some(route_capacity_from_health(
                provider,
                repodesk_core::routing::ProviderKind::Paid,
                None,
                daily_remaining_tokens,
                cost_config,
                budget_config,
                request,
                settings.allow_paid_agents,
            )),
            "anthropic_api" | "anthropic" if settings.anthropic_api_enabled => {
                Some(route_capacity_from_health(
                    provider,
                    repodesk_core::routing::ProviderKind::Paid,
                    None,
                    daily_remaining_tokens,
                    cost_config,
                    budget_config,
                    request,
                    settings.allow_paid_agents,
                ))
            }
            "gemini_api" | "gemini" if settings.gemini_api_enabled => {
                Some(route_capacity_from_health(
                    provider,
                    repodesk_core::routing::ProviderKind::Paid,
                    None,
                    daily_remaining_tokens,
                    cost_config,
                    budget_config,
                    request,
                    settings.allow_paid_agents,
                ))
            }
            _ => custom_provider.map(|custom| {
                route_capacity_from_health(
                    provider,
                    repodesk_core::routing::ProviderKind::Paid,
                    Some(custom.default_model.clone()).filter(|model| !model.trim().is_empty()),
                    daily_remaining_tokens,
                    cost_config,
                    budget_config,
                    request,
                    settings.allow_paid_agents,
                )
            }),
        };

        if let Some(capacity) = capacity {
            capacities.push(capacity);
        }
    }

    capacities.push(manual_route_capacity(
        "chatgpt",
        "ChatGPT manual",
        repodesk_core::routing::ProviderKind::Paid,
        settings.chatgpt_enabled,
        "unknown",
        Some("user-configured"),
        repodesk_core::routing::QuotaStatus::Unknown,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        settings.allow_paid_agents,
    ));

    if settings.gemini_enabled && !settings.gemini_api_enabled {
        capacities.push(manual_route_capacity(
            "gemini",
            "Gemini manual",
            repodesk_core::routing::ProviderKind::Paid,
            true,
            "unknown",
            Some("user-configured"),
            repodesk_core::routing::QuotaStatus::Unknown,
            daily_remaining_tokens,
            cost_config,
            budget_config,
            request,
            settings.allow_paid_agents,
        ));
    }

    capacities.push(manual_route_capacity(
        "codex_cli",
        "Codex CLI",
        repodesk_core::routing::ProviderKind::PatchAgent,
        settings.codex_enabled,
        "unknown",
        Some("codex-plan"),
        repodesk_core::routing::QuotaStatus::from_label(&settings.codex_quota_status),
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        settings.allow_paid_agents,
    ));

    capacities.push(manual_route_capacity(
        "local_checks",
        "Local checks",
        repodesk_core::routing::ProviderKind::CheckRunner,
        true,
        "working",
        Some("allowlisted-shell"),
        repodesk_core::routing::QuotaStatus::Available,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        true,
    ));

    capacities.push(manual_route_capacity(
        "manual",
        "Manual",
        repodesk_core::routing::ProviderKind::Manual,
        true,
        "manual",
        None,
        repodesk_core::routing::QuotaStatus::Available,
        daily_remaining_tokens,
        cost_config,
        budget_config,
        request,
        true,
    ));

    capacities
}

pub(crate) fn build_routing_decision_for_request(
    input: &repodesk_core::routing::RouteRequest,
) -> repodesk_core::routing::RouteDecision {
    let settings = store::read_provider_settings().unwrap_or_default();
    let tokens = build_token_usage_snapshot();
    let model_health = model_health_from_settings(&settings);
    let budget_config = repodesk_core::usage::budget::load_budget_config().unwrap_or_default();
    let cost_config = repodesk_core::usage::cost::load_cost_config().unwrap_or_default();
    let capacities = build_routing_capacities(
        &settings,
        &model_health,
        &tokens,
        &budget_config,
        &cost_config,
        input,
    );

    repodesk_core::routing::route_request(input, &capacities, &budget_config)
}

pub(crate) fn build_routing_snapshot(
    economy_mode: Option<String>,
) -> repodesk_core::routing::RoutingSnapshot {
    let settings = store::read_provider_settings().unwrap_or_default();
    let tokens = build_token_usage_snapshot();
    let model_health = model_health_from_settings(&settings);
    let workflow = build_product_workflow_state();
    let git = repodesk_core::git_workspace::build_git_workspace_snapshot();
    let budget_config = repodesk_core::usage::budget::load_budget_config().unwrap_or_default();
    let cost_config = repodesk_core::usage::cost::load_cost_config().unwrap_or_default();
    let request = build_default_route_request(&workflow, &tokens, &git, economy_mode);
    let capacities = build_routing_capacities(
        &settings,
        &model_health,
        &tokens,
        &budget_config,
        &cost_config,
        &request,
    );
    let decision = repodesk_core::routing::route_request(&request, &capacities, &budget_config);

    repodesk_core::routing::RoutingSnapshot {
        generated_at_ms: now_ms(),
        request,
        decision,
        capacities,
    }
}

#[tauri::command]
pub fn routing_decision(
    input: repodesk_core::routing::RouteRequest,
) -> repodesk_core::routing::RouteDecision {
    build_routing_decision_for_request(&input)
}

#[tauri::command]
pub fn routing_snapshot(economy_mode: Option<String>) -> repodesk_core::routing::RoutingSnapshot {
    build_routing_snapshot(economy_mode)
}
