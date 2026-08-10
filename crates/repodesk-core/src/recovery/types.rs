use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryState {
    Healthy,
    Degraded,
    Repairing,
    NeedsApproval,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverySeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryFailureCode {
    MissingExecutable,
    IncompatibleVersion,
    ProcessCrashed,
    InitializationFailed,
    RequestTimedOut,
    InvalidConfiguration,
    PermissionDenied,
    UnknownFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionKind {
    Automatic,
    Confirmable,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub id: String,
    pub label: String,
    pub kind: RecoveryActionKind,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEvidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryObservation {
    pub capability_id: String,
    pub module_id: String,
    pub generation: u64,
    pub observed_at: DateTime<Utc>,
    pub state: RecoveryState,
    pub severity: RecoverySeverity,
    pub code: Option<RecoveryFailureCode>,
    pub title: String,
    pub explanation: String,
    pub affected: Vec<String>,
    pub unaffected: Vec<String>,
    pub evidence: Vec<RecoveryEvidence>,
    pub actions: Vec<RecoveryAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub capability_id: String,
    pub module_id: String,
    pub generation: u64,
    pub diagnosis_revision: String,
    pub observed_at: DateTime<Utc>,
    pub state: RecoveryState,
    pub severity: RecoverySeverity,
    pub code: Option<RecoveryFailureCode>,
    pub title: String,
    pub explanation: String,
    pub affected: Vec<String>,
    pub unaffected: Vec<String>,
    pub evidence: Vec<RecoveryEvidence>,
    pub actions: Vec<RecoveryAction>,
    pub automatic_attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub project: String,
    pub records: Vec<RecoveryRecord>,
    pub actionable_count: usize,
    pub warnings: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAttemptResult {
    Verified,
    Failed,
    VerificationFailed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub id: String,
    pub capability_id: String,
    pub diagnosis_revision: String,
    pub action_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<RecoveryAttemptResult>,
    pub verification_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveOutcome {
    Applied(Box<RecoveryRecord>),
    IgnoredStale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairCompletion {
    Verified {
        finished_at: DateTime<Utc>,
        summary: String,
    },
    Failed {
        finished_at: DateTime<Utc>,
        summary: String,
    },
    VerificationFailed {
        finished_at: DateTime<Utc>,
        summary: String,
    },
    Cancelled {
        finished_at: DateTime<Utc>,
        summary: String,
    },
}

impl RepairCompletion {
    pub(crate) fn parts(&self) -> (DateTime<Utc>, &str, RecoveryAttemptResult) {
        match self {
            Self::Verified {
                finished_at,
                summary,
            } => (*finished_at, summary, RecoveryAttemptResult::Verified),
            Self::Failed {
                finished_at,
                summary,
            } => (*finished_at, summary, RecoveryAttemptResult::Failed),
            Self::VerificationFailed {
                finished_at,
                summary,
            } => (
                *finished_at,
                summary,
                RecoveryAttemptResult::VerificationFailed,
            ),
            Self::Cancelled {
                finished_at,
                summary,
            } => (*finished_at, summary, RecoveryAttemptResult::Cancelled),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryRisk {
    Low,
    Moderate,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRepairPreview {
    pub capability_id: String,
    pub diagnosis_revision: String,
    pub action_id: String,
    pub title: String,
    pub summary: String,
    pub risk: RecoveryRisk,
    pub recipe_id: String,
    pub recipe_revision: String,
    pub changes: Vec<String>,
    pub network_required: bool,
    pub verification: String,
    pub confirmation_token: String,
    pub expires_at: DateTime<Utc>,
}
