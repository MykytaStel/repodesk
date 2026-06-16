use super::scoring::score_capacity;
use super::types::*;
use crate::usage::budget::BudgetConfig;

pub fn route_request(
    request: &RouteRequest,
    capacities: &[ProviderCapacity],
    budget: &BudgetConfig,
) -> RouteDecision {
    let mut candidates = capacities
        .iter()
        .map(|capacity| score_capacity(request, capacity, budget))
        .collect::<Vec<_>>();

    if !candidates
        .iter()
        .any(|candidate| candidate.kind == ProviderKind::Manual)
    {
        candidates.push(score_capacity(request, &manual_capacity(budget), budget));
    }

    candidates.sort_by(|left, right| {
        left.blocked
            .cmp(&right.blocked)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| provider_rank(&left.provider).cmp(&provider_rank(&right.provider)))
            .then_with(|| left.provider.cmp(&right.provider))
    });

    let recommended = candidates
        .iter()
        .find(|candidate| !candidate.blocked)
        .cloned()
        .unwrap_or_else(|| score_capacity(request, &manual_capacity(budget), budget));

    let fallback = candidates
        .iter()
        .filter(|candidate| !candidate.blocked && candidate.provider != recommended.provider)
        .find(|candidate| candidate.kind != ProviderKind::Manual)
        .or_else(|| {
            candidates
                .iter()
                .find(|candidate| !candidate.blocked && candidate.provider != recommended.provider)
        });

    let non_manual_available = candidates.iter().any(|candidate| {
        !candidate.blocked
            && candidate.kind != ProviderKind::Manual
            && candidate.kind != ProviderKind::CheckRunner
    });
    let check_runner_available = request.task_kind == TaskKind::Checks
        && candidates
            .iter()
            .any(|candidate| !candidate.blocked && candidate.kind == ProviderKind::CheckRunner);
    let mut blockers = recommended.blockers.clone();
    let mut warnings = recommended.warnings.clone();
    let mut required_guardrails = recommended.required_guardrails.clone();

    if recommended.kind == ProviderKind::Manual && request.task_kind != TaskKind::Manual {
        blockers.extend(
            candidates
                .iter()
                .filter(|candidate| candidate.blocked && candidate.kind != ProviderKind::Manual)
                .flat_map(|candidate| candidate.blockers.clone()),
        );
    }

    if request.task_kind == TaskKind::Checks && !check_runner_available {
        blockers.push("No local check runner route is available.".to_string());
    } else if request.task_kind != TaskKind::Checks
        && recommended.kind == ProviderKind::Manual
        && !non_manual_available
    {
        blockers.push("No model or patch route is currently usable.".to_string());
    }

    blockers = unique_strings(blockers);
    warnings = unique_strings(warnings);
    required_guardrails = unique_strings(required_guardrails);

    let decision_level = if !blockers.is_empty() {
        DecisionLevel::Block
    } else if !warnings.is_empty() || recommended.score < 80 {
        DecisionLevel::Warn
    } else {
        DecisionLevel::Allow
    };

    RouteDecision {
        task_kind: request.task_kind,
        recommended_provider: recommended.provider.clone(),
        recommended_model: recommended.model.clone(),
        fallback_provider: fallback.map(|candidate| candidate.provider.clone()),
        fallback_model: fallback.and_then(|candidate| candidate.model.clone()),
        score: recommended.score,
        decision_level,
        blockers,
        warnings,
        required_guardrails,
        candidates,
        estimated_total_tokens: request
            .estimated_input_tokens
            .saturating_add(request.estimated_output_tokens),
    }
}

pub fn manual_capacity(budget: &BudgetConfig) -> ProviderCapacity {
    ProviderCapacity {
        provider: "manual".to_string(),
        label: "Manual".to_string(),
        kind: ProviderKind::Manual,
        enabled: true,
        auth_status: "not_required".to_string(),
        reachability: "manual".to_string(),
        models: Vec::new(),
        preferred_model: None,
        daily_remaining_tokens: budget.daily_hard_limit,
        estimated_cost_units: 0.0,
        quota_status: QuotaStatus::Available,
        paid_agents_allowed: true,
        max_patch_files: budget.max_files_for_patch_agent,
    }
}

fn provider_rank(provider: &str) -> usize {
    match provider {
        "local_checks" => 0,
        "ollama" => 10,
        "lm_studio" => 11,
        "llamafile" => 12,
        "localai" => 13,
        "codex" => 20,
        "chatgpt" => 30,
        "openai" => 31,
        "gemini" => 40,
        "manual" => 90,
        _ => 80,
    }
}

use indexmap::IndexSet;

pub fn unique_strings(items: Vec<String>) -> Vec<String> {
    let set: IndexSet<String> = items.into_iter().collect();
    set.into_iter().collect()
}

pub fn default_capacities(budget: &BudgetConfig) -> Vec<ProviderCapacity> {
    vec![
        ProviderCapacity {
            provider: "ollama".to_string(),
            label: "Ollama".to_string(),
            kind: ProviderKind::Local,
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["llama3.1".to_string()],
            preferred_model: Some("llama3.1".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 0.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "lm_studio".to_string(),
            label: "LM Studio".to_string(),
            kind: ProviderKind::Local,
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["local-model".to_string()],
            preferred_model: Some("local-model".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 0.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "chatgpt".to_string(),
            label: "ChatGPT".to_string(),
            kind: ProviderKind::Paid,
            enabled: true,
            auth_status: "manual".to_string(),
            reachability: "unknown".to_string(),
            models: vec!["gpt-5.5".to_string()],
            preferred_model: Some("gpt-5.5".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "codex".to_string(),
            label: "Codex".to_string(),
            kind: ProviderKind::PatchAgent,
            enabled: true,
            auth_status: "manual".to_string(),
            reachability: "unknown".to_string(),
            models: vec!["deepseek-v4".to_string()],
            preferred_model: Some("deepseek-v4".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "gemini".to_string(),
            label: "Gemini".to_string(),
            kind: ProviderKind::Paid,
            enabled: true,
            auth_status: "manual".to_string(),
            reachability: "unknown".to_string(),
            models: vec!["gemini-3.1-pro".to_string()],
            preferred_model: Some("gemini-3.1-pro".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "local_checks".to_string(),
            label: "Local checks".to_string(),
            kind: ProviderKind::CheckRunner,
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: Vec::new(),
            preferred_model: None,
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 0.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        manual_capacity(budget),
    ]
}

pub fn route_request_for_need(need: &str) -> RouteRequest {
    let normalized = need.to_lowercase();
    let task_kind = if normalized.contains("compress") {
        TaskKind::Compress
    } else if normalized.contains("summary") || normalized.contains("summarize") {
        TaskKind::Summarize
    } else if normalized.contains("patch")
        || normalized.contains("refactor")
        || normalized.contains("implementation")
    {
        TaskKind::Patch
    } else if normalized.contains("review") || normalized.contains("opinion") {
        TaskKind::Review
    } else if normalized.contains("debug") {
        TaskKind::Debug
    } else if normalized.contains("check") {
        TaskKind::Checks
    } else {
        TaskKind::Plan
    };

    let estimated_output_tokens = match task_kind {
        TaskKind::Compress | TaskKind::Summarize => 1_200,
        TaskKind::Plan | TaskKind::Review | TaskKind::Debug => 1_800,
        TaskKind::Patch => 3_500,
        TaskKind::Checks | TaskKind::Manual => 0,
    };

    RouteRequest {
        task_kind,
        estimated_input_tokens: 4_000,
        estimated_output_tokens,
        risk_level: "ok".to_string(),
        changed_file_count: 0,
        requires_write: task_kind == TaskKind::Patch,
        context_safe: Some(true),
        checks_ok: Some(true),
        guard_allowed: Some(true),
        git_dirty: Some(false),
        max_cost_units: None,
        economy_mode: None,
    }
}

pub fn format_route_decision(decision: &RouteDecision) -> String {
    let mut output = String::new();
    output.push_str("Routing Decision:\n\n");
    output.push_str(&format!(
        "need: {}\n",
        match decision.task_kind {
            TaskKind::Compress => "compression",
            TaskKind::Summarize => "summarization",
            TaskKind::Plan => "planning",
            TaskKind::Review => "review",
            TaskKind::Patch => "patch",
            TaskKind::Debug => "debugging",
            TaskKind::Checks => "checks",
            TaskKind::Manual => "manual",
        }
    ));
    output.push_str(&format!(
        "recommended provider: {}\n",
        decision.recommended_provider
    ));
    if let Some(ref model) = decision.recommended_model {
        output.push_str(&format!("recommended model: {}\n", model));
    }
    if let Some(ref fallback) = decision.fallback_provider {
        output.push_str(&format!("fallback provider: {}\n", fallback));
    }
    if let Some(ref fallback_model) = decision.fallback_model {
        output.push_str(&format!("fallback model: {}\n", fallback_model));
    }
    output.push_str(&format!("score: {}\n", decision.score));
    output.push_str(&format!("decision level: {:?}\n", decision.decision_level));
    output.push_str(&format!(
        "estimated total tokens: {}\n",
        decision.estimated_total_tokens
    ));

    if !decision.blockers.is_empty() {
        output.push_str("\nBlockers:\n");
        for blocker in &decision.blockers {
            output.push_str(&format!("  - {}\n", blocker));
        }
    }
    if !decision.warnings.is_empty() {
        output.push_str("\nWarnings:\n");
        for warning in &decision.warnings {
            output.push_str(&format!("  - {}\n", warning));
        }
    }
    if !decision.required_guardrails.is_empty() {
        output.push_str("\nRequired Guardrails:\n");
        for guardrail in &decision.required_guardrails {
            output.push_str(&format!("  - {}\n", guardrail));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(task_kind: TaskKind) -> RouteRequest {
        RouteRequest {
            task_kind,
            estimated_input_tokens: 4_000,
            estimated_output_tokens: 1_000,
            risk_level: "ok".to_string(),
            changed_file_count: 2,
            requires_write: false,
            context_safe: Some(true),
            checks_ok: Some(true),
            guard_allowed: Some(true),
            git_dirty: Some(false),
            max_cost_units: Some(10.0),
            economy_mode: None,
        }
    }

    fn capacity(provider: &str, kind: ProviderKind) -> ProviderCapacity {
        ProviderCapacity {
            provider: provider.to_string(),
            label: provider.to_string(),
            kind,
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["model-a".to_string()],
            preferred_model: None,
            daily_remaining_tokens: 100_000,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: 8,
        }
    }

    fn manual() -> ProviderCapacity {
        capacity("manual", ProviderKind::Manual)
    }

    fn checks() -> ProviderCapacity {
        capacity("local_checks", ProviderKind::CheckRunner)
    }

    #[test]
    fn local_compression_beats_paid_reasoning() {
        let budget = BudgetConfig::default();
        let decision = route_request(
            &request(TaskKind::Compress),
            &[
                capacity("ollama", ProviderKind::Local),
                capacity("openai", ProviderKind::Paid),
                manual(),
            ],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "ollama");
        assert_eq!(decision.decision_level, DecisionLevel::Allow);
    }

    #[test]
    fn codex_patch_allowed_after_guard_passes() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        let decision = route_request(
            &route_request_obj,
            &[
                capacity("ollama", ProviderKind::Local),
                capacity("codex", ProviderKind::PatchAgent),
                manual(),
            ],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "codex");
        assert_eq!(decision.decision_level, DecisionLevel::Allow);
    }

    #[test]
    fn codex_quota_empty_routes_to_manual_with_blocker() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        let mut codex = capacity("codex", ProviderKind::PatchAgent);
        codex.quota_status = QuotaStatus::Empty;
        let decision = route_request(&route_request_obj, &[codex, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert_eq!(decision.decision_level, DecisionLevel::Block);
        assert!(
            decision
                .blockers
                .iter()
                .any(|blocker| blocker.contains("Codex quota status is empty"))
        );
    }

    #[test]
    fn paid_disabled_blocks_reasoning_provider() {
        let budget = BudgetConfig::default();
        let mut paid = capacity("openai", ProviderKind::Paid);
        paid.paid_agents_allowed = false;
        let decision = route_request(&request(TaskKind::Plan), &[paid, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert_eq!(decision.decision_level, DecisionLevel::Block);
        assert!(
            decision
                .blockers
                .iter()
                .any(|blocker| blocker.contains("Paid agents are disabled"))
        );
    }

    #[test]
    fn context_above_paid_hard_limit_blocks_paid_provider() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Debug);
        route_request_obj.estimated_input_tokens = budget.paid_agent_hard_limit + 1;
        let decision = route_request(
            &route_request_obj,
            &[capacity("openai", ProviderKind::Paid), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "manual");
        assert!(
            decision
                .blockers
                .iter()
                .any(|blocker| blocker.contains("paid hard limit"))
        );
    }

    #[test]
    fn no_reachable_models_routes_to_manual() {
        let budget = BudgetConfig::default();
        let mut local = capacity("ollama", ProviderKind::Local);
        local.reachability = "unreachable".to_string();
        let decision = route_request(&request(TaskKind::Summarize), &[local, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert!(
            decision
                .blockers
                .iter()
                .any(|blocker| blocker.contains("unreachable"))
        );
    }

    #[test]
    fn dirty_workspace_adds_codex_warning() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        route_request_obj.git_dirty = Some(true);
        let decision = route_request(
            &route_request_obj,
            &[capacity("codex", ProviderKind::PatchAgent), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "codex");
        assert_eq!(decision.decision_level, DecisionLevel::Warn);
        assert!(
            decision
                .warnings
                .iter()
                .any(|warning| warning.contains("Workspace is dirty"))
        );
    }

    #[test]
    fn checks_route_to_local_runner() {
        let budget = BudgetConfig::default();
        let decision = route_request(
            &request(TaskKind::Checks),
            &[capacity("openai", ProviderKind::Paid), checks(), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "local_checks");
        assert_eq!(decision.decision_level, DecisionLevel::Allow);
    }
}
