use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Compress,
    Summarize,
    Plan,
    Review,
    Patch,
    Debug,
    Checks,
    #[default]
    Manual,
}

impl TaskKind {
    /// Canonical snake_case label (matches the serde rename). Used as a stable
    /// key for the outcome ledger and the learned routing bias.
    pub fn as_label(&self) -> &'static str {
        match self {
            TaskKind::Compress => "compress",
            TaskKind::Summarize => "summarize",
            TaskKind::Plan => "plan",
            TaskKind::Review => "review",
            TaskKind::Patch => "patch",
            TaskKind::Debug => "debug",
            TaskKind::Checks => "checks",
            TaskKind::Manual => "manual",
        }
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

/// What RepoDesk would execute for a route. This is intentionally separate
/// from completion-provider identity so `codex_cli` can never be mistaken for
/// an OpenAI API client.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorKind {
    /// Back-compat default for older serialized plans that only carried a
    /// provider name and expected the LLM client registry to resolve it.
    #[default]
    CompletionProvider,
    LocalRuntime,
    CodingAgent,
    CheckRunner,
    Manual,
}

impl ProviderKind {
    pub fn default_executor_kind(&self) -> ExecutorKind {
        match self {
            ProviderKind::Local => ExecutorKind::LocalRuntime,
            ProviderKind::Paid => ExecutorKind::CompletionProvider,
            ProviderKind::PatchAgent => ExecutorKind::CodingAgent,
            ProviderKind::CheckRunner => ExecutorKind::CheckRunner,
            ProviderKind::Manual => ExecutorKind::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    #[default]
    Unknown,
    Available,
    Limited,
    Empty,
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
    #[serde(default)]
    pub economy_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapacity {
    /// Legacy route id kept for existing UI/API consumers. New code should read
    /// `executor_id` and `provider_id`.
    pub provider: String,
    pub label: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub executor_kind: ExecutorKind,
    #[serde(default)]
    pub executor_id: String,
    /// Canonical completion-provider id, when the executor is an LLM API/local
    /// runtime. Coding-agent and manual routes leave this as `None`.
    #[serde(default)]
    pub provider_id: Option<String>,
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

impl ProviderCapacity {
    pub fn resolved_executor_kind(&self) -> ExecutorKind {
        if self.executor_id.trim().is_empty() {
            self.kind.default_executor_kind()
        } else {
            self.executor_kind
        }
    }

    pub fn resolved_executor_id(&self) -> &str {
        let executor_id = self.executor_id.trim();
        if executor_id.is_empty() {
            self.provider.as_str()
        } else {
            self.executor_id.as_str()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCandidate {
    /// Legacy route id kept for existing UI/API consumers. New code should read
    /// `executor_id` and `provider_id`.
    pub provider: String,
    pub label: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub executor_kind: ExecutorKind,
    #[serde(default)]
    pub executor_id: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub score: i32,
    pub blocked: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub required_guardrails: Vec<String>,
    pub estimated_cost_units: f64,
}

/// One learned routing adjustment for a (task_kind, provider) pair, derived from
/// the outcome ledger ([`crate::outcomes`]). The router applies `adjustment` as a
/// *bounded* nudge on top of the deterministic score — it can sway a tie or a
/// close call toward what has worked, but never unblock a route or override a
/// hard rule. `success_rate`/`scored_weight` are carried for explanation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BiasEntry {
    /// Signed score delta, already clamped to the bias ceiling.
    pub adjustment: i32,
    /// Weighted success rate (human-confirmed rows count double).
    pub success_rate: f64,
    /// Total signal weight behind this entry (the confidence proxy).
    pub scored_weight: f64,
}

/// A learned bias over (task_kind, provider) pairs. An empty bias (the
/// [`Default`]) is a no-op, so routing stays purely deterministic until the
/// ledger has enough signal. Keyed by `(task_kind label, provider)`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteBias {
    entries: std::collections::HashMap<(String, String), BiasEntry>,
}

impl RouteBias {
    pub fn new(entries: std::collections::HashMap<(String, String), BiasEntry>) -> Self {
        Self { entries }
    }

    /// The learned entry for a (task_kind, provider) pair, if one exists.
    pub fn lookup(&self, task_kind: TaskKind, provider: &str) -> Option<&BiasEntry> {
        self.entries
            .get(&(task_kind.as_label().to_string(), provider.to_string()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub task_kind: TaskKind,
    /// Legacy route id kept for existing UI/API consumers.
    pub recommended_provider: String,
    #[serde(default)]
    pub recommended_executor_kind: ExecutorKind,
    #[serde(default)]
    pub recommended_executor_id: String,
    #[serde(default)]
    pub recommended_provider_id: Option<String>,
    pub recommended_model: Option<String>,
    #[serde(default)]
    pub fallback_executor_id: Option<String>,
    #[serde(default)]
    pub fallback_provider_id: Option<String>,
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
