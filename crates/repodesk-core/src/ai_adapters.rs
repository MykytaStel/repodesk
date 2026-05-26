use std::fs;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAdaptersConfig {
    pub adapters: Vec<AiAdapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAdapter {
    pub name: String,
    pub kind: String,
    pub local: bool,
    pub enabled: bool,
    pub access_mode: String,
    pub preferred_for: Vec<String>,
    pub guardrails: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AiAdapterStatus {
    pub name: String,
    pub available: bool,
    pub detail: String,
}

pub fn ensure_ai_adapters_config() -> RepoDeskResult<AiAdaptersConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("ai-adapters.toml");

    if file.exists() {
        return load_ai_adapters_config();
    }

    let config = default_ai_adapters_config();
    fs::write(file, toml::to_string_pretty(&config)?)?;

    Ok(config)
}

pub fn load_ai_adapters_config() -> RepoDeskResult<AiAdaptersConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("ai-adapters.toml");

    if !file.exists() {
        return ensure_ai_adapters_config();
    }

    let content = fs::read_to_string(file)?;
    Ok(toml::from_str(&content)?)
}

pub fn recommend_ai_adapters(need: &str) -> RepoDeskResult<Vec<AiAdapter>> {
    let config = ensure_ai_adapters_config()?;
    let need = need.to_ascii_lowercase();

    let mut adapters = config
        .adapters
        .into_iter()
        .filter(|adapter| {
            adapter.enabled
                && adapter
                    .preferred_for
                    .iter()
                    .any(|item| item.to_ascii_lowercase().contains(&need))
        })
        .collect::<Vec<_>>();

    adapters.sort_by_key(|adapter| if adapter.local { 0 } else { 1 });
    Ok(adapters)
}

pub fn check_ai_status(name: Option<&str>) -> RepoDeskResult<Vec<AiAdapterStatus>> {
    let config = ensure_ai_adapters_config()?;
    let filter = name.map(|value| value.to_ascii_lowercase());

    let statuses = config
        .adapters
        .into_iter()
        .filter(|adapter| {
            filter
                .as_ref()
                .map(|needle| adapter.name.to_ascii_lowercase() == *needle)
                .unwrap_or(true)
        })
        .map(|adapter| status_for_adapter(&adapter))
        .collect::<Vec<_>>();

    Ok(statuses)
}

pub fn format_ai_adapters(config: &AiAdaptersConfig) -> String {
    let mut output = String::new();
    output.push_str("AI adapters:\n\n");

    for adapter in &config.adapters {
        output.push_str(&format!("- {} ({})\n", adapter.name, adapter.kind));
        output.push_str(&format!("  enabled: {}\n", adapter.enabled));
        output.push_str(&format!("  local: {}\n", adapter.local));
        output.push_str(&format!("  access mode: {}\n", adapter.access_mode));
        output.push_str(&format!(
            "  preferred for: {}\n",
            adapter.preferred_for.join(", ")
        ));
        output.push_str(&format!(
            "  guardrails: {}\n\n",
            adapter.guardrails.join(", ")
        ));
    }

    output
}

pub fn format_ai_recommendations(need: &str, adapters: &[AiAdapter]) -> String {
    if adapters.is_empty() {
        return format!("No enabled AI adapters recommended for '{}'.\n", need);
    }

    let mut output = format!("Recommended AI adapters for '{}':\n\n", need);

    for adapter in adapters {
        output.push_str(&format!("- {} ({})\n", adapter.name, adapter.kind));
        output.push_str(&format!("  access mode: {}\n", adapter.access_mode));
        output.push_str(&format!(
            "  preferred for: {}\n\n",
            adapter.preferred_for.join(", ")
        ));
    }

    output
}

pub fn format_ai_status(statuses: &[AiAdapterStatus]) -> String {
    if statuses.is_empty() {
        return "No AI adapters matched.\n".to_string();
    }

    let mut output = String::new();
    output.push_str("AI adapter status:\n\n");

    for status in statuses {
        output.push_str(&format!("- {}\n", status.name));
        output.push_str(&format!("  available: {}\n", status.available));
        output.push_str(&format!("  detail: {}\n\n", status.detail));
    }

    output
}

fn status_for_adapter(adapter: &AiAdapter) -> AiAdapterStatus {
    if !adapter.enabled {
        return AiAdapterStatus {
            name: adapter.name.clone(),
            available: false,
            detail: "adapter disabled in config".to_string(),
        };
    }

    match adapter.name.as_str() {
        "ollama" => match Command::new("ollama").arg("--version").output() {
            Ok(output) if output.status.success() => AiAdapterStatus {
                name: adapter.name.clone(),
                available: true,
                detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            },
            Ok(output) => AiAdapterStatus {
                name: adapter.name.clone(),
                available: false,
                detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            },
            Err(error) => AiAdapterStatus {
                name: adapter.name.clone(),
                available: false,
                detail: error.to_string(),
            },
        },
        _ => AiAdapterStatus {
            name: adapter.name.clone(),
            available: false,
            detail: "external/manual adapter; availability is not checked locally".to_string(),
        },
    }
}

fn default_ai_adapters_config() -> AiAdaptersConfig {
    AiAdaptersConfig {
        adapters: vec![
            AiAdapter {
                name: "ollama".to_string(),
                kind: "local_llm".to_string(),
                local: true,
                enabled: true,
                access_mode: "local_cli_or_http".to_string(),
                preferred_for: vec![
                    "compression".to_string(),
                    "summarization".to_string(),
                    "cheap reasoning".to_string(),
                ],
                guardrails: vec![
                    "no secrets".to_string(),
                    "read-only by default".to_string(),
                    "summarize before paid agent".to_string(),
                ],
            },
            AiAdapter {
                name: "chatgpt".to_string(),
                kind: "external_reasoning".to_string(),
                local: false,
                enabled: true,
                access_mode: "manual_paste".to_string(),
                preferred_for: vec![
                    "architecture".to_string(),
                    "review".to_string(),
                    "planning".to_string(),
                ],
                guardrails: vec!["bounded context".to_string(), "no full logs".to_string()],
            },
            AiAdapter {
                name: "codex".to_string(),
                kind: "external_patch_agent".to_string(),
                local: false,
                enabled: true,
                access_mode: "manual_or_cli".to_string(),
                preferred_for: vec![
                    "patch".to_string(),
                    "tests".to_string(),
                    "refactor".to_string(),
                ],
                guardrails: vec![
                    "preflight required".to_string(),
                    "small diff only".to_string(),
                ],
            },
            AiAdapter {
                name: "gemini".to_string(),
                kind: "external_second_opinion".to_string(),
                local: false,
                enabled: true,
                access_mode: "manual_or_cli".to_string(),
                preferred_for: vec![
                    "second opinion".to_string(),
                    "large context review".to_string(),
                ],
                guardrails: vec!["review only by default".to_string()],
            },
        ],
    }
}
