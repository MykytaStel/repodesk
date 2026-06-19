use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProvider {
    pub name: String,
    pub kind: String,
    pub access_model: String,
    pub cost_profile: String,
    pub trust_level: String,
    pub strengths: Vec<String>,
    pub limits: Vec<String>,
    pub recommended_for: Vec<String>,
    pub health_check: RuntimeHealthCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeHealthCheck {
    pub mode: String,
    pub command: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProviderStatus {
    pub provider: String,
    pub available: bool,
    pub status: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRoute {
    pub need: String,
    pub recommended_provider: String,
    pub reason: String,
    pub fallback_provider: String,
    pub required_guardrails: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub generated_at: String,
    pub providers: Vec<RuntimeProvider>,
    pub statuses: Vec<RuntimeProviderStatus>,
}

pub fn runtime_providers() -> Vec<RuntimeProvider> {
    vec![
        RuntimeProvider {
            name: "ollama".to_string(),
            kind: "local_ai".to_string(),
            access_model: "local process via CLI/API".to_string(),
            cost_profile: "free/local compute".to_string(),
            trust_level: "medium; local, but still needs prompt safety".to_string(),
            strengths: vec![
                "cheap compression".to_string(),
                "drafting".to_string(),
                "offline-friendly reasoning".to_string(),
            ],
            limits: vec![
                "model quality depends on installed model".to_string(),
                "large context may be slow".to_string(),
            ],
            recommended_for: vec![
                "compression".to_string(),
                "summary".to_string(),
                "local_review".to_string(),
            ],
            health_check: RuntimeHealthCheck {
                mode: "command".to_string(),
                command: Some("ollama --version".to_string()),
                notes: "Checks whether the Ollama CLI is installed and reachable.".to_string(),
            },
        },
        RuntimeProvider {
            name: "chatgpt".to_string(),
            kind: "manual_ai_handoff".to_string(),
            access_model: "manual/web handoff".to_string(),
            cost_profile: "paid/limited tokens".to_string(),
            trust_level: "high reasoning; bounded context only".to_string(),
            strengths: vec![
                "architecture review".to_string(),
                "product judgement".to_string(),
                "complex debugging explanation".to_string(),
            ],
            limits: vec![
                "should not receive full repo dumps".to_string(),
                "should not receive secrets".to_string(),
            ],
            recommended_for: vec![
                "architecture".to_string(),
                "debugging".to_string(),
                "planning".to_string(),
            ],
            health_check: RuntimeHealthCheck {
                mode: "manual".to_string(),
                command: None,
                notes: "External provider. RepoDesk should route only bounded context.".to_string(),
            },
        },
        RuntimeProvider {
            name: "codex_cli".to_string(),
            kind: "coding_agent_executor".to_string(),
            access_model: "bounded repository access via CLI executor (planned)".to_string(),
            cost_profile: "paid/limited tokens".to_string(),
            trust_level: "powerful but must be guarded".to_string(),
            strengths: vec![
                "file edits".to_string(),
                "focused patches".to_string(),
                "test-driven implementation".to_string(),
            ],
            limits: vec![
                "must not receive unrestricted shell".to_string(),
                "must not patch without preflight".to_string(),
            ],
            recommended_for: vec!["patch".to_string(), "refactor".to_string()],
            health_check: RuntimeHealthCheck {
                mode: "manual".to_string(),
                command: None,
                notes: "Coding-agent executor. CLI execution is planned behind guard/judge/access checks.".to_string(),
            },
        },
        RuntimeProvider {
            name: "gemini".to_string(),
            kind: "manual_ai_handoff".to_string(),
            access_model: "manual/web handoff or gemini_api completion provider".to_string(),
            cost_profile: "paid/limited tokens".to_string(),
            trust_level: "external; bounded context only".to_string(),
            strengths: vec![
                "alternative reasoning".to_string(),
                "large context review".to_string(),
            ],
            limits: vec![
                "provider behavior may differ from patch agents".to_string(),
                "do not expose secrets".to_string(),
            ],
            recommended_for: vec!["second_opinion".to_string(), "review".to_string()],
            health_check: RuntimeHealthCheck {
                mode: "manual".to_string(),
                command: None,
                notes: "External provider. Integration can be added later.".to_string(),
            },
        },
    ]
}

pub fn provider_status(provider: &str) -> RepoDeskResult<RuntimeProviderStatus> {
    let provider = provider.to_lowercase();

    match provider.as_str() {
        "ollama" => Ok(check_ollama()),
        "chatgpt" | "codex" | "codex_cli" | "gemini" => Ok(RuntimeProviderStatus {
            provider,
            available: true,
            status: "manual".to_string(),
            details: "External/manual provider. RepoDesk can prepare bounded context and guardrails, but does not call it directly yet.".to_string(),
        }),
        other => Ok(RuntimeProviderStatus {
            provider: other.to_string(),
            available: false,
            status: "unknown".to_string(),
            details: "Provider is not registered in RepoDesk runtime registry.".to_string(),
        }),
    }
}

pub fn runtime_snapshot_json() -> RepoDeskResult<String> {
    let providers = runtime_providers();
    let statuses = providers
        .iter()
        .map(|provider| provider_status(&provider.name))
        .collect::<RepoDeskResult<Vec<_>>>()?;

    let snapshot = RuntimeSnapshot {
        generated_at: Utc::now().to_rfc3339(),
        providers,
        statuses,
    };

    Ok(serde_json::to_string_pretty(&snapshot)?)
}

pub fn format_runtime_providers(providers: &[RuntimeProvider]) -> String {
    let mut output = String::new();
    output.push_str("Runtime providers:\n\n");

    for provider in providers {
        output.push_str(&format!("{}\n", provider.name));
        output.push_str(&format!("  kind: {}\n", provider.kind));
        output.push_str(&format!("  access: {}\n", provider.access_model));
        output.push_str(&format!("  cost: {}\n", provider.cost_profile));
        output.push_str(&format!("  trust: {}\n", provider.trust_level));
        output.push_str(&format!(
            "  recommended for: {}\n",
            provider.recommended_for.join(", ")
        ));
        output.push('\n');
    }

    output
}

pub fn format_provider_status(status: &RuntimeProviderStatus) -> String {
    format!(
        "Provider status:\n\nprovider: {}\navailable: {}\nstatus: {}\ndetails: {}\n",
        status.provider, status.available, status.status, status.details
    )
}

pub fn format_runtime_route(route: &RuntimeRoute) -> String {
    let mut output = String::new();
    output.push_str("Runtime route:\n\n");
    output.push_str(&format!("need: {}\n", route.need));
    output.push_str(&format!(
        "recommended provider: {}\n",
        route.recommended_provider
    ));
    output.push_str(&format!("fallback provider: {}\n", route.fallback_provider));
    output.push_str(&format!("reason: {}\n", route.reason));
    output.push_str("required guardrails:\n");
    for item in &route.required_guardrails {
        output.push_str(&format!("  - {item}\n"));
    }
    output
}

fn check_ollama() -> RuntimeProviderStatus {
    match Command::new("ollama").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let details = if !stdout.is_empty() { stdout } else { stderr };

            RuntimeProviderStatus {
                provider: "ollama".to_string(),
                available: true,
                status: "available".to_string(),
                details,
            }
        }
        Ok(output) => RuntimeProviderStatus {
            provider: "ollama".to_string(),
            available: false,
            status: "command_failed".to_string(),
            details: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        },
        Err(error) => RuntimeProviderStatus {
            provider: "ollama".to_string(),
            available: false,
            status: "not_found".to_string(),
            details: error.to_string(),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub provider: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub retries_used: usize,
}

pub async fn execute_with_fallback<F, Fut>(
    route: &RuntimeRoute,
    mut run_agent: F,
) -> ExecutionResult
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let providers = vec![
        route.recommended_provider.clone(),
        route.fallback_provider.clone(),
    ];

    for provider in providers {
        if provider.trim().is_empty() {
            continue;
        }

        let mut retries = 0;
        let max_retries = 3;
        let mut backoff_ms = 1000;

        while retries <= max_retries {
            match run_agent(provider.clone()).await {
                Ok(output) => {
                    return ExecutionResult {
                        provider,
                        success: true,
                        output: Some(output),
                        error: None,
                        retries_used: retries,
                    };
                }
                Err(error) => {
                    let is_rate_limit = error.contains("429")
                        || error.to_lowercase().contains("rate limit")
                        || error.to_lowercase().contains("too many requests");

                    if is_rate_limit {
                        retries += 1;
                        if retries <= max_retries {
                            sleep(Duration::from_millis(backoff_ms)).await;
                            backoff_ms *= 2; // Exponential backoff
                            continue;
                        }
                    }

                    // If it's not a rate limit error, or we ran out of retries,
                    // we break out of the while loop and try the next fallback provider.
                    break;
                }
            }
        }
    }

    ExecutionResult {
        provider: "none".to_string(),
        success: false,
        output: None,
        error: Some("All routing providers failed or exhausted rate limit retries.".to_string()),
        retries_used: 0,
    }
}
