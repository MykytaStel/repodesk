use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProbeStatus {
    Available,
    Missing,
    Maybe,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProbeCategory {
    LocalRuntime,
    PaidAgent,
    CliAgent,
    Editor,
    DesktopApp,
    RuntimeDependency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolProbe {
    pub id: String,
    pub name: String,
    pub category: AiProbeCategory,
    pub status: AiProbeStatus,
    pub detection: String,
    pub executable_path: Option<String>,
    pub app_path: Option<String>,
    pub local_only: bool,
    pub requires_paid_account: bool,
    pub risk_level: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEndpointProbe {
    pub id: String,
    pub name: String,
    pub url: String,
    pub status: AiProbeStatus,
    pub local_only: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDiscoveryReport {
    pub generated_at: DateTime<Utc>,
    pub host_os: String,
    pub tools: Vec<AiToolProbe>,
    pub endpoints: Vec<AiEndpointProbe>,
    pub recommendations: Vec<String>,
    pub warnings: Vec<String>,
    pub report_path: Option<String>,
}
