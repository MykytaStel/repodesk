use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeripheralsConfig {
    pub peripherals: Vec<PeripheralConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeripheralConfig {
    pub name: String,
    pub kind: String,
    pub access: String,
    pub risk: String,
    pub allowed_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PeripheralAudit {
    pub findings: Vec<String>,
    pub recommendations: Vec<String>,
}

pub fn ensure_peripherals_config() -> RepoDeskResult<PeripheralsConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("peripherals.toml");

    if file.exists() {
        return load_peripherals_config();
    }

    let config = default_peripherals_config();
    std::fs::write(file, toml::to_string_pretty(&config)?)?;

    Ok(config)
}

pub fn load_peripherals_config() -> RepoDeskResult<PeripheralsConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("peripherals.toml");

    if !file.exists() {
        return ensure_peripherals_config();
    }

    let content = std::fs::read_to_string(file)?;
    Ok(toml::from_str(&content)?)
}

pub fn audit_peripherals() -> RepoDeskResult<PeripheralAudit> {
    let config = ensure_peripherals_config()?;
    let mut findings = Vec::new();
    let mut recommendations = Vec::new();

    for peripheral in &config.peripherals {
        if peripheral.access == "unrestricted" {
            findings.push(format!(
                "{} has unrestricted access. This is unsafe for AI-controlled workflows.",
                peripheral.name
            ));
            recommendations.push(format!(
                "Reduce {} to manual, restricted, or read-only access.",
                peripheral.name
            ));
        }

        if peripheral.kind == "shell" && peripheral.access != "manual" {
            findings.push("Shell access should stay manual by default.".to_string());
            recommendations
                .push("Use RepoDesk checks runner instead of giving agents raw shell.".to_string());
        }

        if peripheral.kind == "filesystem_write" && peripheral.access != "restricted" {
            findings.push("Filesystem write access should be restricted.".to_string());
            recommendations.push("Allow writes only through bounded patch workflows.".to_string());
        }
    }

    if findings.is_empty() {
        findings.push("No dangerous peripheral configuration detected.".to_string());
        recommendations
            .push("Keep shell and write access behind guard/preflight checks.".to_string());
    }

    Ok(PeripheralAudit {
        findings,
        recommendations,
    })
}

pub fn format_peripherals(config: &PeripheralsConfig) -> String {
    let mut output = String::new();
    output.push_str("Peripherals registry:\n\n");

    for item in &config.peripherals {
        output.push_str(&format!("- {} ({})\n", item.name, item.kind));
        output.push_str(&format!("  access: {}\n", item.access));
        output.push_str(&format!("  risk: {}\n", item.risk));
        output.push_str(&format!("  allowed: {}\n", item.allowed_actions.join(", ")));
        output.push_str(&format!(
            "  forbidden: {}\n\n",
            item.forbidden_actions.join(", ")
        ));
    }

    output
}

pub fn format_peripheral_audit(audit: &PeripheralAudit) -> String {
    let findings = audit
        .findings
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    let recommendations = audit
        .recommendations
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Peripheral audit:\n\nFindings:\n{}\n\nRecommendations:\n{}\n",
        findings, recommendations
    )
}

pub fn explain_peripheral(config: &PeripheralsConfig, name: &str) -> String {
    let Some(item) = config
        .peripherals
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
    else {
        return format!("Peripheral '{name}' was not found.\n");
    };

    format!(
        r#"Peripheral: {}
Kind: {}
Access: {}
Risk: {}

Allowed actions:
{}

Forbidden actions:
{}
"#,
        item.name,
        item.kind,
        item.access,
        item.risk,
        item.allowed_actions
            .iter()
            .map(|action| format!("  - {action}"))
            .collect::<Vec<_>>()
            .join("\n"),
        item.forbidden_actions
            .iter()
            .map(|action| format!("  - {action}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn default_peripherals_config() -> PeripheralsConfig {
    PeripheralsConfig {
        peripherals: vec![
            PeripheralConfig {
                name: "shell".to_string(),
                kind: "shell".to_string(),
                access: "manual".to_string(),
                risk: "high".to_string(),
                allowed_actions: vec!["run approved checks".to_string()],
                forbidden_actions: vec!["unrestricted command execution".to_string()],
            },
            PeripheralConfig {
                name: "git".to_string(),
                kind: "vcs".to_string(),
                access: "read-mostly".to_string(),
                risk: "medium".to_string(),
                allowed_actions: vec![
                    "status".to_string(),
                    "diff".to_string(),
                    "branch".to_string(),
                ],
                forbidden_actions: vec!["push without human approval".to_string()],
            },
            PeripheralConfig {
                name: "filesystem_read".to_string(),
                kind: "filesystem_read".to_string(),
                access: "bounded".to_string(),
                risk: "medium".to_string(),
                allowed_actions: vec!["read selected project files".to_string()],
                forbidden_actions: vec!["read secrets or full home directory".to_string()],
            },
            PeripheralConfig {
                name: "filesystem_write".to_string(),
                kind: "filesystem_write".to_string(),
                access: "restricted".to_string(),
                risk: "high".to_string(),
                allowed_actions: vec!["write generated RepoDesk artifacts".to_string()],
                forbidden_actions: vec!["unbounded patching".to_string()],
            },
            PeripheralConfig {
                name: "ollama".to_string(),
                kind: "local_ai".to_string(),
                access: "local".to_string(),
                risk: "medium".to_string(),
                allowed_actions: vec!["compress".to_string(), "summarize".to_string()],
                forbidden_actions: vec!["final unreviewed decisions".to_string()],
            },
            PeripheralConfig {
                name: "desktop_ui".to_string(),
                kind: "ui".to_string(),
                access: "planned".to_string(),
                risk: "low".to_string(),
                allowed_actions: vec![
                    "display status".to_string(),
                    "trigger approved commands".to_string(),
                ],
                forbidden_actions: vec!["hidden background mutations".to_string()],
            },
            PeripheralConfig {
                name: "mcp_readonly".to_string(),
                kind: "mcp".to_string(),
                access: "planned_readonly".to_string(),
                risk: "medium".to_string(),
                allowed_actions: vec!["serve bounded context".to_string()],
                forbidden_actions: vec!["unrestricted shell tools".to_string()],
            },
        ],
    }
}
