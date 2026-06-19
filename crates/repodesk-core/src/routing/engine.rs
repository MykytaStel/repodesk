use super::scoring::score_capacity;
use super::types::*;
use crate::usage::budget::BudgetConfig;

/// Route a request using only the deterministic scoring rules (no learned
/// bias). Equivalent to [`route_request_with_bias`] with an empty bias.
pub fn route_request(
    request: &RouteRequest,
    capacities: &[ProviderCapacity],
    budget: &BudgetConfig,
) -> RouteDecision {
    route_request_with_bias(request, capacities, budget, &RouteBias::default())
}

/// Route a request, applying a learned [`RouteBias`] as a bounded nudge on top
/// of the deterministic scores. The bias only shifts non-blocked model routes:
/// it can break a tie or sway a close call toward what has worked on this
/// project, but it never unblocks a route, touches the Manual/CheckRunner
/// floors, or overrides a hard rule. Every applied nudge is recorded as a
/// candidate warning so the decision stays explainable.
pub fn route_request_with_bias(
    request: &RouteRequest,
    capacities: &[ProviderCapacity],
    budget: &BudgetConfig,
    bias: &RouteBias,
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

    if !bias.is_empty() {
        for candidate in &mut candidates {
            apply_bias(request, candidate, bias);
        }
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
        recommended_executor_kind: recommended.executor_kind,
        recommended_executor_id: recommended.executor_id.clone(),
        recommended_provider_id: recommended.provider_id.clone(),
        recommended_model: recommended.model.clone(),
        fallback_executor_id: fallback.map(|candidate| candidate.executor_id.clone()),
        fallback_provider_id: fallback.and_then(|candidate| candidate.provider_id.clone()),
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

/// Apply the learned bias to one candidate. Only non-blocked model routes are
/// nudged (Manual/CheckRunner floors and blocked routes are left untouched), the
/// adjusted score is re-clamped to the scoring range, and an explanation is
/// recorded so the nudge is visible in the decision.
fn apply_bias(request: &RouteRequest, candidate: &mut RouteCandidate, bias: &RouteBias) {
    if candidate.blocked
        || !matches!(
            candidate.kind,
            ProviderKind::Local | ProviderKind::Paid | ProviderKind::PatchAgent
        )
    {
        return;
    }
    let Some(entry) = bias.lookup(request.task_kind, &candidate.provider) else {
        return;
    };
    if entry.adjustment == 0 {
        return;
    }
    candidate.score = (candidate.score + entry.adjustment).clamp(0, 150);
    candidate.warnings.push(format!(
        "Learned routing: {} has {:.0}% success on {} tasks here ({}{} adjustment).",
        candidate.provider,
        entry.success_rate * 100.0,
        request.task_kind.as_label(),
        if entry.adjustment >= 0 { "+" } else { "" },
        entry.adjustment,
    ));
}

pub fn manual_capacity(budget: &BudgetConfig) -> ProviderCapacity {
    ProviderCapacity {
        provider: "manual".to_string(),
        label: "Manual".to_string(),
        kind: ProviderKind::Manual,
        executor_kind: ExecutorKind::Manual,
        executor_id: "manual".to_string(),
        provider_id: None,
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
        "codex_cli" => 20,
        "claude_code_cli" => 21,
        "openai_api" => 30,
        "anthropic_api" => 31,
        "gemini_api" => 40,
        "chatgpt" => 50,
        "openai" => 51,
        "gemini" => 60,
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
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: "ollama".to_string(),
            provider_id: Some("ollama".to_string()),
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
            executor_kind: ExecutorKind::LocalRuntime,
            executor_id: "lm_studio".to_string(),
            provider_id: Some("lm_studio".to_string()),
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
            provider: "openai_api".to_string(),
            label: "OpenAI API".to_string(),
            kind: ProviderKind::Paid,
            executor_kind: ExecutorKind::CompletionProvider,
            executor_id: "openai_api".to_string(),
            provider_id: Some("openai_api".to_string()),
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
            preferred_model: Some("gpt-4o-mini".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "anthropic_api".to_string(),
            label: "Anthropic API".to_string(),
            kind: ProviderKind::Paid,
            executor_kind: ExecutorKind::CompletionProvider,
            executor_id: "anthropic_api".to_string(),
            provider_id: Some("anthropic_api".to_string()),
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["claude-sonnet-4-6".to_string()],
            preferred_model: Some("claude-sonnet-4-6".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "codex_cli".to_string(),
            label: "Codex CLI".to_string(),
            kind: ProviderKind::PatchAgent,
            executor_kind: ExecutorKind::CodingAgent,
            executor_id: "codex_cli".to_string(),
            provider_id: None,
            enabled: true,
            auth_status: "cli_auth".to_string(),
            reachability: "unknown".to_string(),
            models: vec!["codex-cli".to_string()],
            preferred_model: Some("codex-cli".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Available,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "claude_code_cli".to_string(),
            label: "Claude Code CLI".to_string(),
            kind: ProviderKind::PatchAgent,
            executor_kind: ExecutorKind::CodingAgent,
            executor_id: "claude_code_cli".to_string(),
            provider_id: None,
            enabled: true,
            auth_status: "cli_auth".to_string(),
            reachability: "unknown".to_string(),
            models: vec!["claude-code-cli".to_string()],
            preferred_model: Some("claude-code-cli".to_string()),
            daily_remaining_tokens: budget.daily_hard_limit,
            estimated_cost_units: 1.0,
            quota_status: QuotaStatus::Unknown,
            paid_agents_allowed: true,
            max_patch_files: budget.max_files_for_patch_agent,
        },
        ProviderCapacity {
            provider: "gemini_api".to_string(),
            label: "Gemini API".to_string(),
            kind: ProviderKind::Paid,
            executor_kind: ExecutorKind::CompletionProvider,
            executor_id: "gemini_api".to_string(),
            provider_id: Some("gemini_api".to_string()),
            enabled: true,
            auth_status: "configured".to_string(),
            reachability: "working".to_string(),
            models: vec!["gemini-2.5-pro".to_string()],
            preferred_model: Some("gemini-2.5-pro".to_string()),
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
            executor_kind: ExecutorKind::CheckRunner,
            executor_id: "local_checks".to_string(),
            provider_id: None,
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
    output.push_str(&format!(
        "recommended executor: {} ({:?})\n",
        decision.recommended_executor_id, decision.recommended_executor_kind
    ));
    if let Some(ref provider_id) = decision.recommended_provider_id {
        output.push_str(&format!("completion provider: {}\n", provider_id));
    }
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
            executor_kind: kind.default_executor_kind(),
            executor_id: provider.to_string(),
            provider_id: match kind {
                ProviderKind::Local | ProviderKind::Paid => Some(provider.to_string()),
                ProviderKind::PatchAgent | ProviderKind::CheckRunner | ProviderKind::Manual => None,
            },
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
    fn coding_agent_patch_allowed_after_guard_passes() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        let decision = route_request(
            &route_request_obj,
            &[
                capacity("ollama", ProviderKind::Local),
                capacity("codex_cli", ProviderKind::PatchAgent),
                manual(),
            ],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "codex_cli");
        assert_eq!(decision.recommended_executor_id, "codex_cli");
        assert_eq!(
            decision.recommended_executor_kind,
            ExecutorKind::CodingAgent
        );
        assert_eq!(decision.decision_level, DecisionLevel::Allow);
    }

    #[test]
    fn coding_agent_quota_empty_routes_to_manual_with_blocker() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        let mut codex = capacity("codex_cli", ProviderKind::PatchAgent);
        codex.quota_status = QuotaStatus::Empty;
        let decision = route_request(&route_request_obj, &[codex, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert_eq!(decision.decision_level, DecisionLevel::Block);
        assert!(
            decision
                .blockers
                .iter()
                .any(|blocker| blocker.contains("codex_cli quota status is empty"))
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
    fn dirty_workspace_adds_coding_agent_warning() {
        let budget = BudgetConfig::default();
        let mut route_request_obj = request(TaskKind::Patch);
        route_request_obj.requires_write = true;
        route_request_obj.git_dirty = Some(true);
        let decision = route_request(
            &route_request_obj,
            &[capacity("codex_cli", ProviderKind::PatchAgent), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "codex_cli");
        assert_eq!(decision.decision_level, DecisionLevel::Warn);
        assert!(
            decision
                .warnings
                .iter()
                .any(|warning| warning.contains("Workspace is dirty"))
        );
    }

    fn bias_of(task_kind: TaskKind, provider: &str, adjustment: i32) -> RouteBias {
        let mut entries = std::collections::HashMap::new();
        entries.insert(
            (task_kind.as_label().to_string(), provider.to_string()),
            BiasEntry {
                adjustment,
                success_rate: if adjustment >= 0 { 1.0 } else { 0.0 },
                scored_weight: 8.0,
            },
        );
        RouteBias::new(entries)
    }

    #[test]
    fn learned_bias_flips_a_tie_between_equal_local_providers() {
        let budget = BudgetConfig::default();
        let caps = [
            capacity("ollama", ProviderKind::Local),
            capacity("lm_studio", ProviderKind::Local),
            manual(),
        ];

        // No bias: a Plan task ties both locals, so provider_rank picks ollama.
        let plain = route_request(&request(TaskKind::Plan), &caps, &budget);
        assert_eq!(plain.recommended_provider, "ollama");

        // A learned win for lm_studio on Plan tasks pulls it ahead.
        let bias = bias_of(TaskKind::Plan, "lm_studio", 20);
        let learned = route_request_with_bias(&request(TaskKind::Plan), &caps, &budget, &bias);
        assert_eq!(learned.recommended_provider, "lm_studio");
        // The nudge is explained on the chosen candidate.
        assert!(
            learned
                .warnings
                .iter()
                .any(|w| w.contains("Learned routing") && w.contains("lm_studio")),
            "the learned nudge should be surfaced: {:?}",
            learned.warnings
        );
    }

    #[test]
    fn learned_bias_never_unblocks_a_blocked_route() {
        let budget = BudgetConfig::default();
        let mut paid = capacity("openai", ProviderKind::Paid);
        paid.paid_agents_allowed = false; // hard block
        // An absurdly large positive bias must not resurrect a blocked route.
        let bias = bias_of(TaskKind::Plan, "openai", 150);
        let decision =
            route_request_with_bias(&request(TaskKind::Plan), &[paid, manual()], &budget, &bias);
        assert_eq!(decision.recommended_provider, "manual");
        assert_ne!(decision.recommended_provider, "openai");
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
