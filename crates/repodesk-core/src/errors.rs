use thiserror::Error;

pub type RepoDeskResult<T> = Result<T, RepoDeskError>;

/// Category of error — used by the frontend to pick the right UI treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// User needs to fix their configuration (project, task, path).
    Configuration,
    /// A provider is temporarily unavailable; the user can retry later.
    ProviderTransient,
    /// The agent or sandbox explicitly blocked the operation.
    SecurityBlock,
    /// The operation is too expensive (token budget / context size).
    ResourceLimit,
    /// An unexpected internal problem occurred.
    Internal,
}

#[derive(Debug, Error)]
pub enum RepoDeskError {
    // ── Configuration / project state ──────────────────────────────────────
    #[error("could not determine a valid RepoDesk home directory")]
    HomeDirectoryNotFound,

    #[error("project '{0}' already exists")]
    ProjectAlreadyExists(String),

    #[error("project '{0}' was not found")]
    ProjectNotFound(String),

    #[error("active project is not set")]
    ActiveProjectNotSet,

    #[error("active task is not set for project '{0}'")]
    ActiveTaskNotSet(String),

    #[error("task '{0}' was not found")]
    TaskNotFound(String),

    #[error("invalid project name '{0}'. Use letters, numbers, '-', '_' only")]
    InvalidProjectName(String),

    #[error("project path does not exist: {0}")]
    ProjectPathDoesNotExist(String),

    #[error("invalid check command: {0}")]
    InvalidCheckCommand(String),

    // ── Security / sandbox ─────────────────────────────────────────────────
    /// Raised when the command sandbox blocks a shell command that is not on
    /// the allow-list or matches a known-dangerous pattern.
    #[error("sandbox blocked command: {command} — reason: {reason}")]
    SandboxBlocked { command: String, reason: String },

    /// Raised when the secret scanner finds a credential pattern in the text
    /// that is about to be sent to an external provider.
    #[error("secret detected in {location}: {detail}")]
    SecretDetected { location: String, detail: String },

    // ── Provider / routing ─────────────────────────────────────────────────
    /// The provider returned HTTP 429 or similar; use `retry_after_secs` if
    /// it was provided in the response headers.
    #[error("provider '{provider}' is rate-limited (retry after {retry_after_secs}s)")]
    ProviderRateLimit {
        provider: String,
        retry_after_secs: u64,
    },

    /// The provider is down, unreachable, or not configured.
    #[error("provider '{provider}' is unavailable: {detail}")]
    ProviderUnavailable { provider: String, detail: String },

    /// All providers in the fallback chain have been exhausted.
    #[error("routing failed: no available provider could handle the request — {detail}")]
    RoutingFailed { detail: String },

    // ── Workflow / execution ───────────────────────────────────────────────
    /// A long-running workflow step has been safely paused. The frontend
    /// should offer the user a "Resume" button.
    #[error("workflow paused at step '{step}': {reason}")]
    WorkflowPaused { step: String, reason: String },

    // ── Resource limits ────────────────────────────────────────────────────
    /// The context payload would exceed the model's context window.
    #[error(
        "context too large: {estimated_tokens} tokens exceeds the {limit_tokens} limit for '{model}'"
    )]
    ContextTooLarge {
        model: String,
        estimated_tokens: usize,
        limit_tokens: usize,
    },

    /// The operation would exceed the configured daily token budget.
    #[error(
        "budget exceeded: estimated {estimated_cost} {currency} but budget is {budget} {currency}"
    )]
    BudgetExceeded {
        estimated_cost: String,
        budget: String,
        currency: String,
    },

    // ── Low-level I/O and serialization ────────────────────────────────────
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("API error: {0}")]
    Api(String),
}

impl RepoDeskError {
    /// Which UI treatment should the frontend apply?
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::HomeDirectoryNotFound
            | Self::ProjectAlreadyExists(_)
            | Self::ProjectNotFound(_)
            | Self::ActiveProjectNotSet
            | Self::ActiveTaskNotSet(_)
            | Self::TaskNotFound(_)
            | Self::InvalidProjectName(_)
            | Self::ProjectPathDoesNotExist(_)
            | Self::InvalidCheckCommand(_) => ErrorCategory::Configuration,

            Self::SandboxBlocked { .. } | Self::SecretDetected { .. } => {
                ErrorCategory::SecurityBlock
            }

            Self::ProviderRateLimit { .. }
            | Self::ProviderUnavailable { .. }
            | Self::RoutingFailed { .. }
            | Self::WorkflowPaused { .. } => ErrorCategory::ProviderTransient,

            Self::ContextTooLarge { .. } | Self::BudgetExceeded { .. } => {
                ErrorCategory::ResourceLimit
            }

            Self::Io(_)
            | Self::TomlDe(_)
            | Self::TomlSer(_)
            | Self::Json(_)
            | Self::Database(_)
            | Self::Api(_) => ErrorCategory::Internal,
        }
    }

    /// Should the frontend offer a retry button for this error?
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderRateLimit { .. } | Self::ProviderUnavailable { .. } | Self::Io(_)
        )
    }
}
