use serde::{Deserialize, Serialize};

use crate::budget::BudgetConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Compress,
    Summarize,
    Plan,
    Review,
    Patch,
    Debug,
    Checks,
    Manual,
}

impl Default for TaskKind {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    Paid,
    PatchAgent,
    CheckRunner,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    Unknown,
    Available,
    Limited,
    Empty,
}

impl Default for QuotaStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl QuotaStatus {
    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "available" => Self::Available,
            "limited" => Self::Limited,
            "empty" => Self::Empty,
            _ => Self::Unknown,
        }
    }

    pub fn as_label(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Available => "available",
            Self::Limited => "limited",
            Self::Empty => "empty",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionLevel {
    Allow,
    Warn,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub task_kind: TaskKind,
    pub estimated_input_tokens: usize,
    pub estimated_output_tokens: usize,
    pub risk_level: String,
    pub changed_file_count: usize,
    pub requires_write: bool,
    #[serde(default)]
    pub context_safe: Option<bool>,
    #[serde(default)]
    pub checks_ok: Option<bool>,
    #[serde(default)]
    pub guard_allowed: Option<bool>,
    #[serde(default)]
    pub git_dirty: Option<bool>,
    #[serde(default)]
    pub max_cost_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapacity {
    pub provider: String,
    pub label: String,
    pub kind: ProviderKind,
    pub enabled: bool,
    pub auth_status: String,
    pub reachability: String,
    pub models: Vec<String>,
    pub preferred_model: Option<String>,
    pub daily_remaining_tokens: usize,
    pub estimated_cost_units: f64,
    pub quota_status: QuotaStatus,
    pub paid_agents_allowed: bool,
    pub max_patch_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub provider: String,
    pub label: String,
    pub kind: ProviderKind,
    pub model: Option<String>,
    pub score: i32,
    pub blocked: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub required_guardrails: Vec<String>,
    pub estimated_cost_units: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub task_kind: TaskKind,
    pub recommended_provider: String,
    pub recommended_model: Option<String>,
    pub fallback_provider: Option<String>,
    pub fallback_model: Option<String>,
    pub score: i32,
    pub decision_level: DecisionLevel,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub required_guardrails: Vec<String>,
    pub candidates: Vec<RouteCandidate>,
    pub estimated_total_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSnapshot {
    pub generated_at_ms: u128,
    pub request: RouteRequest,
    pub decision: RouteDecision,
    pub capacities: Vec<ProviderCapacity>,
}

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
                .filter(|candidate| {
                    !candidate.blocked && candidate.provider != recommended.provider
                })
                .next()
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
        task_kind: request.task_kind.clone(),
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

fn score_capacity(
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
                blockers.push("Patch tasks require Codex or a manual patch route.".to_string());
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
                blockers.push("Context is not safe for Codex.".to_string());
            }
            if capacity.quota_status == QuotaStatus::Empty {
                blockers.push("Codex quota status is empty.".to_string());
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
                    "Too many changed files for Codex: {} > {}.",
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
                warnings.push("Codex quota status is limited.".to_string());
            } else if capacity.quota_status == QuotaStatus::Unknown {
                warnings.push(
                    "Codex quota status is unknown; route is allowed with manual confirmation."
                        .to_string(),
                );
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

    if let Some(max_cost_units) = request.max_cost_units {
        if capacity.estimated_cost_units > max_cost_units {
            score -= 20;
            warnings.push(format!(
                "Estimated cost {:.4} exceeds preference {:.4}.",
                capacity.estimated_cost_units, max_cost_units
            ));
        }
    }

    score = score.clamp(0, 150);

    RouteCandidate {
        provider: capacity.provider.clone(),
        label: capacity.label.clone(),
        kind: capacity.kind.clone(),
        model: preferred_model(capacity),
        score,
        blocked: !blockers.is_empty(),
        blockers: unique_strings(blockers),
        warnings: unique_strings(warnings),
        required_guardrails: unique_strings(required_guardrails),
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

fn manual_capacity(budget: &BudgetConfig) -> ProviderCapacity {
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

fn risk_contains_block(value: &str) -> bool {
    value.contains("block")
        || value.contains("unsafe")
        || value.contains("secret")
        || value.contains("credential")
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

fn unique_strings(items: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for item in items {
        if !output.contains(&item) {
            output.push(item);
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
        let mut route_request = request(TaskKind::Patch);
        route_request.requires_write = true;
        let decision = route_request_fn(
            &route_request,
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
        let mut route_request = request(TaskKind::Patch);
        route_request.requires_write = true;
        let mut codex = capacity("codex", ProviderKind::PatchAgent);
        codex.quota_status = QuotaStatus::Empty;
        let decision = route_request_fn(&route_request, &[codex, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert_eq!(decision.decision_level, DecisionLevel::Block);
        assert!(decision
            .blockers
            .iter()
            .any(|blocker| blocker.contains("Codex quota status is empty")));
    }

    #[test]
    fn paid_disabled_blocks_reasoning_provider() {
        let budget = BudgetConfig::default();
        let mut paid = capacity("openai", ProviderKind::Paid);
        paid.paid_agents_allowed = false;
        let decision = route_request_fn(&request(TaskKind::Plan), &[paid, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert_eq!(decision.decision_level, DecisionLevel::Block);
        assert!(decision
            .blockers
            .iter()
            .any(|blocker| blocker.contains("Paid agents are disabled")));
    }

    #[test]
    fn context_above_paid_hard_limit_blocks_paid_provider() {
        let budget = BudgetConfig::default();
        let mut route_request = request(TaskKind::Debug);
        route_request.estimated_input_tokens = budget.paid_agent_hard_limit + 1;
        let decision = route_request_fn(
            &route_request,
            &[capacity("openai", ProviderKind::Paid), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "manual");
        assert!(decision
            .blockers
            .iter()
            .any(|blocker| blocker.contains("paid hard limit")));
    }

    #[test]
    fn no_reachable_models_routes_to_manual() {
        let budget = BudgetConfig::default();
        let mut local = capacity("ollama", ProviderKind::Local);
        local.reachability = "unreachable".to_string();
        let decision = route_request_fn(&request(TaskKind::Summarize), &[local, manual()], &budget);

        assert_eq!(decision.recommended_provider, "manual");
        assert!(decision
            .blockers
            .iter()
            .any(|blocker| blocker.contains("unreachable")));
    }

    #[test]
    fn dirty_workspace_adds_codex_warning() {
        let budget = BudgetConfig::default();
        let mut route_request = request(TaskKind::Patch);
        route_request.requires_write = true;
        route_request.git_dirty = Some(true);
        let decision = route_request_fn(
            &route_request,
            &[capacity("codex", ProviderKind::PatchAgent), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "codex");
        assert_eq!(decision.decision_level, DecisionLevel::Warn);
        assert!(decision
            .warnings
            .iter()
            .any(|warning| warning.contains("Workspace is dirty")));
    }

    #[test]
    fn checks_route_to_local_runner() {
        let budget = BudgetConfig::default();
        let decision = route_request_fn(
            &request(TaskKind::Checks),
            &[capacity("openai", ProviderKind::Paid), checks(), manual()],
            &budget,
        );

        assert_eq!(decision.recommended_provider, "local_checks");
        assert_eq!(decision.decision_level, DecisionLevel::Allow);
    }

    fn route_request_fn(
        input: &RouteRequest,
        capacities: &[ProviderCapacity],
        budget: &BudgetConfig,
    ) -> RouteDecision {
        route_request(input, capacities, budget)
    }
}
