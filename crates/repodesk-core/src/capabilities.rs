use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesConfig {
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub local: bool,
    pub risk: String,
    pub boundary: String,
    pub preferred_for: Vec<String>,
    pub allowed_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CapabilityAudit {
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

impl Default for CapabilitiesConfig {
    fn default() -> Self {
        default_capabilities_config()
    }
}

impl crate::utils::ConfigStore for CapabilitiesConfig {
    const FILE_NAME: &'static str = "capabilities.toml";
}

pub fn ensure_capabilities_config() -> RepoDeskResult<CapabilitiesConfig> {
    use crate::utils::ConfigStore;
    CapabilitiesConfig::ensure_config()
}

pub fn load_capabilities_config() -> RepoDeskResult<CapabilitiesConfig> {
    use crate::utils::ConfigStore;
    CapabilitiesConfig::load_config()
}

pub fn recommend_capabilities(need: &str) -> RepoDeskResult<Vec<Capability>> {
    let config = ensure_capabilities_config()?;
    let normalized = need.to_ascii_lowercase();

    let mut matches = config
        .capabilities
        .into_iter()
        .filter(|capability| {
            capability.enabled
                && (capability
                    .preferred_for
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(&normalized))
                    || capability.kind.to_ascii_lowercase().contains(&normalized)
                    || capability.name.to_ascii_lowercase().contains(&normalized))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|a| risk_weight(&a.risk));

    Ok(matches)
}

pub fn audit_capabilities() -> RepoDeskResult<CapabilityAudit> {
    let config = ensure_capabilities_config()?;
    let mut warnings = Vec::new();
    let mut recommendations = Vec::new();

    for capability in config.capabilities.iter().filter(|item| item.enabled) {
        if capability.risk == "high" {
            warnings.push(format!(
                "Capability '{}' is enabled and marked high risk.",
                capability.name
            ));
        }

        if capability
            .allowed_actions
            .iter()
            .any(|item| item.to_ascii_lowercase().contains("unrestricted shell"))
        {
            warnings.push(format!(
                "Capability '{}' allows unrestricted shell behavior.",
                capability.name
            ));
        }

        if capability.local && capability.kind == "paid_agent" {
            warnings.push(format!(
                "Capability '{}' is marked local but kind is paid_agent.",
                capability.name
            ));
        }
    }

    if warnings.is_empty() {
        recommendations.push("Capabilities look safe for local-first workflow.".to_string());
    } else {
        recommendations.push(
            "Keep high-risk capabilities disabled until a security policy allows them.".to_string(),
        );
        recommendations
            .push("Prefer read-only adapters first, then controlled write adapters.".to_string());
    }

    Ok(CapabilityAudit {
        warnings,
        recommendations,
    })
}

pub fn format_capabilities(config: &CapabilitiesConfig) -> String {
    let mut output = String::new();
    output.push_str("Capabilities:\n\n");

    for capability in &config.capabilities {
        output.push_str(&format!("- {} ({})\n", capability.name, capability.kind));
        output.push_str(&format!("  enabled: {}\n", capability.enabled));
        output.push_str(&format!("  local: {}\n", capability.local));
        output.push_str(&format!("  risk: {}\n", capability.risk));
        output.push_str(&format!("  boundary: {}\n", capability.boundary));
        output.push_str(&format!(
            "  preferred for: {}\n",
            capability.preferred_for.join(", ")
        ));
        output.push_str(&format!(
            "  allowed: {}\n",
            capability.allowed_actions.join(", ")
        ));
        output.push_str(&format!(
            "  forbidden: {}\n\n",
            capability.forbidden_actions.join(", ")
        ));
    }

    output
}

pub fn format_capability_recommendations(need: &str, capabilities: &[Capability]) -> String {
    if capabilities.is_empty() {
        return format!("No enabled capabilities found for need '{}'.\n", need);
    }

    let mut output = format!("Recommended capabilities for '{}':\n\n", need);

    for capability in capabilities {
        output.push_str(&format!("- {} ({})\n", capability.name, capability.kind));
        output.push_str(&format!("  risk: {}\n", capability.risk));
        output.push_str(&format!("  boundary: {}\n\n", capability.boundary));
    }

    output
}

pub fn format_capability_audit(audit: &CapabilityAudit) -> String {
    let warnings = if audit.warnings.is_empty() {
        "  - none".to_string()
    } else {
        audit
            .warnings
            .iter()
            .map(|item| format!("  - {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let recommendations = audit
        .recommendations
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Capability audit:\n\nWarnings:\n{}\n\nRecommendations:\n{}\n",
        warnings, recommendations
    )
}

fn default_capabilities_config() -> CapabilitiesConfig {
    CapabilitiesConfig {
        capabilities: vec![
            Capability {
                name: "repodesk-core".to_string(),
                kind: "core_engine".to_string(),
                enabled: true,
                local: true,
                risk: "low".to_string(),
                boundary: "Owns workflow state, decisions, budgets, context packs.".to_string(),
                preferred_for: vec!["brain".to_string(), "workflow".to_string()],
                allowed_actions: vec!["read/write ~/.repodesk".to_string()],
                forbidden_actions: vec!["direct external AI calls".to_string()],
            },
            Capability {
                name: "local-files".to_string(),
                kind: "storage".to_string(),
                enabled: true,
                local: true,
                risk: "low".to_string(),
                boundary: "Local project registry, task runs, prompts, logs.".to_string(),
                preferred_for: vec!["storage".to_string(), "audit".to_string()],
                allowed_actions: vec!["read/write RepoDesk files".to_string()],
                forbidden_actions: vec!["secret scanning by default".to_string()],
            },
            Capability {
                name: "local-shell-checks".to_string(),
                kind: "controlled_tool".to_string(),
                enabled: true,
                local: true,
                risk: "medium".to_string(),
                boundary: "Runs only configured project checks.".to_string(),
                preferred_for: vec!["checks".to_string(), "validation".to_string()],
                allowed_actions: vec!["configured checks".to_string()],
                forbidden_actions: vec![
                    "unrestricted shell".to_string(),
                    "secret read".to_string(),
                ],
            },
            Capability {
                name: "ollama".to_string(),
                kind: "local_ai".to_string(),
                enabled: true,
                local: true,
                risk: "medium".to_string(),
                boundary: "Local summarization/compression; no final architecture authority."
                    .to_string(),
                preferred_for: vec![
                    "compression".to_string(),
                    "summary".to_string(),
                    "local-ai".to_string(),
                ],
                allowed_actions: vec!["summarize".to_string(), "compress".to_string()],
                forbidden_actions: vec![
                    "autonomous patching".to_string(),
                    "secret ingestion".to_string(),
                ],
            },
            Capability {
                name: "chatgpt".to_string(),
                kind: "paid_agent".to_string(),
                enabled: true,
                local: false,
                risk: "medium".to_string(),
                boundary: "Architecture/review only through bounded prompts.".to_string(),
                preferred_for: vec![
                    "architecture".to_string(),
                    "review".to_string(),
                    "planning".to_string(),
                ],
                allowed_actions: vec!["review context pack".to_string(), "plan slices".to_string()],
                forbidden_actions: vec![
                    "full repo ingestion".to_string(),
                    "unbounded prompt".to_string(),
                ],
            },
            Capability {
                name: "codex".to_string(),
                kind: "paid_patch_agent".to_string(),
                enabled: true,
                local: false,
                risk: "high".to_string(),
                boundary: "Bounded patch executor only after guard preflight.".to_string(),
                preferred_for: vec!["patch".to_string(), "implementation".to_string()],
                allowed_actions: vec!["small scoped patch".to_string()],
                forbidden_actions: vec!["broad rewrite".to_string(), "secret access".to_string()],
            },
            Capability {
                name: "desktop-ui".to_string(),
                kind: "planned_ui".to_string(),
                enabled: false,
                local: true,
                risk: "medium".to_string(),
                boundary: "Tauri UI over core commands/events.".to_string(),
                preferred_for: vec!["ui".to_string(), "dashboard".to_string()],
                allowed_actions: vec!["visualize core state".to_string()],
                forbidden_actions: vec!["bypass core guardrails".to_string()],
            },
            Capability {
                name: "mcp-readonly".to_string(),
                kind: "planned_adapter".to_string(),
                enabled: false,
                local: true,
                risk: "medium".to_string(),
                boundary: "Read-only context/project tools later.".to_string(),
                preferred_for: vec!["mcp".to_string(), "read-only".to_string()],
                allowed_actions: vec!["read status".to_string(), "read context packs".to_string()],
                forbidden_actions: vec![
                    "unrestricted write tools".to_string(),
                    "shell by default".to_string(),
                ],
            },
        ],
    }
}

fn risk_weight(risk: &str) -> usize {
    match risk {
        "low" => 0,
        "medium" => 1,
        "high" => 2,
        _ => 3,
    }
}
