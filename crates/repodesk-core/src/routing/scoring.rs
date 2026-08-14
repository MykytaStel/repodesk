use super::types::*;
use crate::usage::budget::BudgetConfig;

pub fn score_capacity(
    request: &RouteRequest,
    capacity: &ProviderCapacity,
    budget: &BudgetConfig,
) -> RouteCandidate {
    let mut score = 100i32;
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut required_guardrails = Vec::new();
    let total_tokens = request
        .estimated_input_tokens
        .saturating_add(request.estimated_output_tokens);
    let reachability = capacity.reachability.to_ascii_lowercase();
    let auth_status = capacity.auth_status.to_ascii_lowercase();
    let risk = request.risk_level.to_ascii_lowercase();

    match capacity.kind {
        ProviderKind::Manual => {
            score = if request.task_kind == TaskKind::Manual {
                120
            } else {
                1
            };
            warnings
                .push("Manual route requires user judgement and explicit commands.".to_string());
            required_guardrails.push("Keep actions manual and allowlisted.".to_string());
        }
        ProviderKind::CheckRunner => {
            if request.task_kind == TaskKind::Checks {
                score += 80;
                required_guardrails
                    .push("Run checks locally through the allowlisted runner.".to_string());
            } else {
                blockers.push("Local check runner is only valid for checks tasks.".to_string());
            }
        }
        ProviderKind::Local => {
            if request.task_kind == TaskKind::Checks {
                blockers.push(
                    "Checks must run through the local check runner, not a model.".to_string(),
                );
            }
            if request.task_kind == TaskKind::Patch {
                blockers.push("Patch tasks require a patch agent or manual route.".to_string());
            }
            if matches!(request.task_kind, TaskKind::Compress | TaskKind::Summarize) {
                score += 25;
            }

            // Economy mode adjustments
            if let Some(mode) = &request.economy_mode {
                if mode == "economy" {
                    score += 100; // Heavily prefer local
                } else if mode == "balanced"
                    && matches!(
                        request.task_kind,
                        TaskKind::Compress | TaskKind::Summarize | TaskKind::Review
                    )
                {
                    score += 50; // Prefer local for drafts/checks
                } else if mode == "quality" {
                    score -= 50; // Heavily penalize local
                }
            }

            if request.task_kind == TaskKind::Review {
                score += 5;
                warnings.push("Local review is best for cheap first-pass feedback.".to_string());
            }
            if risk_contains_block(&risk) {
                warnings.push(
                    "Risk markers are blocking for paid routes; local route still needs review."
                        .to_string(),
                );
            }
            if request.requires_write {
                score -= 25;
                warnings.push(
                    "Task requires writes; local model should only draft guidance.".to_string(),
                );
            }
        }
        ProviderKind::Paid => {
            if request.task_kind == TaskKind::Checks {
                blockers.push("Checks must run locally and never through paid models.".to_string());
            }
            if request.task_kind == TaskKind::Patch {
                blockers.push(
                    "Patch tasks require a coding-agent executor or a manual patch route."
                        .to_string(),
                );
            }
            if !capacity.paid_agents_allowed {
                blockers.push("Paid agents are disabled in provider settings.".to_string());
            }
            if risk_contains_block(&risk) || request.context_safe == Some(false) {
                blockers.push("Context is not safe for a paid provider.".to_string());
            }
            if matches!(
                request.task_kind,
                TaskKind::Plan | TaskKind::Review | TaskKind::Debug
            ) {
                score += 15;
            }

            // Economy mode adjustments
            if let Some(mode) = &request.economy_mode {
                if mode == "economy" {
                    score -= 100; // Penalize paid heavily
                    warnings.push("Economy mode prefers local models over paid ones.".to_string());
                } else if mode == "balanced"
                    && matches!(
                        request.task_kind,
                        TaskKind::Plan | TaskKind::Debug | TaskKind::Patch
                    )
                {
                    score += 50; // Prefer paid for hard tasks
                } else if mode == "quality" {
                    score += 100; // Prefer paid always
                }
            }

            if total_tokens > budget.paid_agent_hard_limit {
                blockers.push(format!(
                    "Estimated context exceeds paid hard limit: {} > {} tokens.",
                    total_tokens, budget.paid_agent_hard_limit
                ));
            } else if total_tokens > budget.paid_agent_soft_limit {
                score -= 40;
                warnings.push(format!(
                    "Estimated context exceeds paid soft limit: {} > {} tokens.",
                    total_tokens, budget.paid_agent_soft_limit
                ));
            }
            if request.requires_write {
                score -= 25;
                warnings.push(
                    "Task requires writes; reasoning providers should not patch directly."
                        .to_string(),
                );
            }
            required_guardrails.push("Send only bounded smart context.".to_string());
            required_guardrails
                .push("Do not include raw secrets or private credentials.".to_string());
        }
        ProviderKind::PatchAgent => {
            if request.task_kind == TaskKind::Checks {
                blockers
                    .push("Checks must run locally and never through patch agents.".to_string());
            }
            if request.task_kind != TaskKind::Patch && !request.requires_write {
                blockers.push("Patch agent is reserved for patch or refactor tasks.".to_string());
            }
            if !capacity.paid_agents_allowed {
                blockers.push("Paid agents are disabled in provider settings.".to_string());
            }
            if risk_contains_block(&risk) || request.context_safe == Some(false) {
                blockers.push("Context is not safe for a coding-agent executor.".to_string());
            }
            if capacity.quota_status == QuotaStatus::Empty {
                blockers.push(format!("{} quota status is empty.", capacity.label));
            }
            if request.guard_allowed == Some(false) {
                blockers.push("Guard preflight does not allow the patch route.".to_string());
            }
            if request.checks_ok == Some(false) {
                blockers.push(
                    "Checks are not available or not passing for the patch route.".to_string(),
                );
            }
            if request.changed_file_count > capacity.max_patch_files.saturating_mul(2) {
                blockers.push(format!(
                    "Too many changed files for {}: {} > {}.",
                    capacity.label,
                    request.changed_file_count,
                    capacity.max_patch_files.saturating_mul(2)
                ));
            } else if request.changed_file_count > capacity.max_patch_files {
                warnings.push(format!(
                    "Many changed files for a patch agent: {} > {}.",
                    request.changed_file_count, capacity.max_patch_files
                ));
            }
            if total_tokens > budget.paid_agent_hard_limit {
                blockers.push(format!(
                    "Estimated context exceeds paid hard limit: {} > {} tokens.",
                    total_tokens, budget.paid_agent_hard_limit
                ));
            } else if total_tokens > budget.paid_agent_soft_limit {
                score -= 40;
                warnings.push(format!(
                    "Estimated context exceeds paid soft limit: {} > {} tokens.",
                    total_tokens, budget.paid_agent_soft_limit
                ));
            }
            if request.git_dirty == Some(true) {
                warnings.push(
                    "Workspace is dirty; review existing changes before using a patch agent."
                        .to_string(),
                );
            }
            if capacity.quota_status == QuotaStatus::Limited {
                score -= 50;
                warnings.push(format!("{} quota status is limited.", capacity.label));
            } else if capacity.quota_status == QuotaStatus::Unknown {
                warnings.push(format!(
                    "{} quota status is unknown; route is allowed with manual confirmation.",
                    capacity.label
                ));
            }
            if request.task_kind == TaskKind::Patch || request.requires_write {
                score += 25;
            }
            required_guardrails.push("Run guard preflight before handing off.".to_string());
            required_guardrails.push("Run local checks before and after patching.".to_string());
            required_guardrails.push("Review Git diff before accepting changes.".to_string());
        }
    }

    if !capacity.enabled || reachability == "disabled" {
        blockers.push(format!("{} is disabled.", capacity.label));
    }

    if matches!(
        capacity.kind,
        ProviderKind::Local | ProviderKind::Paid | ProviderKind::PatchAgent
    ) {
        if auth_status == "auth_missing" {
            blockers.push(format!("{} authentication is missing.", capacity.label));
        }

        if reachability == "unreachable" {
            blockers.push(format!("{} is unreachable.", capacity.label));
        } else if reachability == "rate_limited" {
            score -= 50;
            warnings.push(format!("{} is rate limited.", capacity.label));
        } else if reachability == "unknown" {
            score -= 30;
            warnings.push(format!("{} reachability is unknown.", capacity.label));
        }

        if capacity.kind != ProviderKind::PatchAgent
            && reachability == "working"
            && capacity.models.is_empty()
        {
            blockers.push(format!("{} has no available models.", capacity.label));
        }

        if capacity.daily_remaining_tokens == 0
            && matches!(capacity.kind, ProviderKind::Paid | ProviderKind::PatchAgent)
        {
            blockers.push("Daily token budget proxy is empty.".to_string());
        }

        if capacity.quota_status == QuotaStatus::Limited
            && capacity.kind != ProviderKind::PatchAgent
        {
            score -= 50;
            warnings.push(format!("{} quota status is limited.", capacity.label));
        } else if capacity.quota_status == QuotaStatus::Empty {
            blockers.push(format!("{} quota status is empty.", capacity.label));
        }
    }

    if let Some(max_cost_units) = request.max_cost_units
        && capacity.estimated_cost_units > max_cost_units
    {
        blockers.push(format!(
            "Estimated cost {:.4} exceeds maximum {:.4}.",
            capacity.estimated_cost_units, max_cost_units
        ));
    }

    score = score.clamp(0, 150);

    RouteCandidate {
        provider: capacity.provider.clone(),
        label: capacity.label.clone(),
        kind: capacity.kind,
        executor_kind: capacity.resolved_executor_kind(),
        executor_id: capacity.resolved_executor_id().to_string(),
        provider_id: capacity.provider_id.clone(),
        model: preferred_model(capacity),
        score,
        blocked: !blockers.is_empty(),
        blockers: crate::routing::engine::unique_strings(blockers),
        warnings: crate::routing::engine::unique_strings(warnings),
        required_guardrails: crate::routing::engine::unique_strings(required_guardrails),
        estimated_cost_units: capacity.estimated_cost_units,
    }
}

fn preferred_model(capacity: &ProviderCapacity) -> Option<String> {
    capacity
        .preferred_model
        .clone()
        .filter(|model| !model.trim().is_empty())
        .or_else(|| capacity.models.first().cloned())
}

fn risk_contains_block(value: &str) -> bool {
    value.contains("block")
        || value.contains("unsafe")
        || value.contains("secret")
        || value.contains("credential")
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
            max_cost_units: None,
            economy_mode: None,
        }
    }

    fn capacity(kind: ProviderKind) -> ProviderCapacity {
        ProviderCapacity {
            provider: "provider".to_string(),
            label: "Provider".to_string(),
            kind,
            executor_kind: kind.default_executor_kind(),
            executor_id: "provider".to_string(),
            provider_id: Some("provider".to_string()),
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

    fn score(
        task: TaskKind,
        mutate: impl FnOnce(&mut RouteRequest, &mut ProviderCapacity),
    ) -> RouteCandidate {
        let mut req = request(task);
        let mut cap = capacity(ProviderKind::Paid);
        mutate(&mut req, &mut cap);
        score_capacity(&req, &cap, &BudgetConfig::default())
    }

    #[test]
    fn unsafe_context_blocks_paid_provider() {
        let candidate = score(TaskKind::Plan, |req, _| req.context_safe = Some(false));
        assert!(candidate.blocked);
        assert!(candidate.blockers.iter().any(|b| b.contains("not safe")));
    }

    #[test]
    fn risk_marker_blocks_paid_provider() {
        let candidate = score(TaskKind::Plan, |req, _| {
            req.risk_level = "secret".to_string()
        });
        assert!(candidate.blocked);
    }

    #[test]
    fn disabled_paid_agent_is_blocked() {
        let candidate = score(TaskKind::Plan, |_, cap| cap.paid_agents_allowed = false);
        assert!(candidate.blocked);
        assert!(candidate.blockers.iter().any(|b| b.contains("disabled")));
    }

    #[test]
    fn checks_task_never_routes_to_paid_or_local() {
        let paid = score_capacity(
            &request(TaskKind::Checks),
            &capacity(ProviderKind::Paid),
            &BudgetConfig::default(),
        );
        assert!(paid.blocked);
        let local = score_capacity(
            &request(TaskKind::Checks),
            &capacity(ProviderKind::Local),
            &BudgetConfig::default(),
        );
        assert!(local.blocked);
    }

    #[test]
    fn paid_hard_token_limit_blocks() {
        let candidate = score(TaskKind::Plan, |req, _| {
            req.estimated_input_tokens = 50_000;
            req.estimated_output_tokens = 50_000;
        });
        assert!(candidate.blocked);
        assert!(candidate.blockers.iter().any(|b| b.contains("hard limit")));
    }

    #[test]
    fn max_cost_units_is_a_hard_constraint() {
        let candidate = score(TaskKind::Plan, |req, cap| {
            req.max_cost_units = Some(0.5);
            cap.estimated_cost_units = 0.75;
        });
        assert!(candidate.blocked);
        assert!(candidate.blockers.iter().any(|b| b.contains("maximum")));
    }

    #[test]
    fn cost_at_or_below_maximum_remains_eligible() {
        let candidate = score(TaskKind::Plan, |req, cap| {
            req.max_cost_units = Some(1.0);
            cap.estimated_cost_units = 1.0;
        });
        assert!(!candidate.blocked);
    }

    #[test]
    fn unsafe_context_blocks_patch_agent() {
        let mut req = request(TaskKind::Patch);
        req.requires_write = true;
        req.context_safe = Some(false);
        let candidate = score_capacity(
            &req,
            &capacity(ProviderKind::PatchAgent),
            &BudgetConfig::default(),
        );
        assert!(candidate.blocked);
    }

    #[test]
    fn patch_agent_blocked_when_guard_disallows() {
        let mut req = request(TaskKind::Patch);
        req.requires_write = true;
        req.guard_allowed = Some(false);
        let candidate = score_capacity(
            &req,
            &capacity(ProviderKind::PatchAgent),
            &BudgetConfig::default(),
        );
        assert!(candidate.blocked);
        assert!(
            candidate
                .blockers
                .iter()
                .any(|b| b.contains("Guard preflight"))
        );
    }

    #[test]
    fn economy_mode_prefers_local_over_paid() {
        let mut req = request(TaskKind::Compress);
        req.economy_mode = Some("economy".to_string());
        let local = score_capacity(
            &req,
            &capacity(ProviderKind::Local),
            &BudgetConfig::default(),
        );
        let paid = score_capacity(
            &req,
            &capacity(ProviderKind::Paid),
            &BudgetConfig::default(),
        );
        assert!(local.score > paid.score);
    }

    #[test]
    fn disabled_capacity_is_blocked_regardless_of_kind() {
        let mut cap = capacity(ProviderKind::Local);
        cap.enabled = false;
        let candidate =
            score_capacity(&request(TaskKind::Compress), &cap, &BudgetConfig::default());
        assert!(candidate.blocked);
    }
}
