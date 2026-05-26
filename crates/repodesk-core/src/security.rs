use std::fs;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::guard::preflight;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub allow_unrestricted_shell: bool,
    pub allow_secret_access: bool,
    pub allow_external_network_by_default: bool,
    pub require_preflight_for_paid_agents: bool,
    pub require_context_before_agent: bool,
    pub require_checks_before_patch_agent: bool,
    pub require_prompt_files_before_agent: bool,
    pub blocked_path_patterns: Vec<String>,
    pub paid_agents: Vec<String>,
    pub patch_agents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SecurityAudit {
    pub level: SecurityLevel,
    pub findings: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityLevel {
    Ok,
    Warning,
    Block,
}

impl SecurityLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            SecurityLevel::Ok => "OK",
            SecurityLevel::Warning => "WARNING",
            SecurityLevel::Block => "BLOCK",
        }
    }
}

pub fn ensure_security_policy() -> RepoDeskResult<SecurityPolicy> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("security.toml");

    if file.exists() {
        return load_security_policy();
    }

    let policy = SecurityPolicy::default();
    fs::write(file, toml::to_string_pretty(&policy)?)?;

    Ok(policy)
}

pub fn load_security_policy() -> RepoDeskResult<SecurityPolicy> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("security.toml");

    if !file.exists() {
        return ensure_security_policy();
    }

    let content = fs::read_to_string(file)?;
    let policy = toml::from_str(&content)?;

    Ok(policy)
}

pub fn audit_security_policy() -> RepoDeskResult<SecurityAudit> {
    let policy = ensure_security_policy()?;
    let project = get_active_project()?;
    let task = show_active_task()?;

    let mut level = SecurityLevel::Ok;
    let mut findings = Vec::new();
    let mut recommendations = Vec::new();

    if policy.allow_unrestricted_shell {
        level = SecurityLevel::Block;
        findings.push("Policy allows unrestricted shell.".to_string());
        recommendations.push("Disable unrestricted shell. Use configured checks only.".to_string());
    }

    if policy.allow_secret_access {
        level = SecurityLevel::Block;
        findings.push("Policy allows secret access.".to_string());
        recommendations.push("Keep secrets out of context packs and agent prompts.".to_string());
    }

    if policy.allow_external_network_by_default {
        if level != SecurityLevel::Block {
            level = SecurityLevel::Warning;
        }
        findings.push("External network access is allowed by default.".to_string());
        recommendations
            .push("Require explicit per-capability approval for network access.".to_string());
    }

    if policy.require_context_before_agent && !task.config.run_dir.join("context.md").exists() {
        if level != SecurityLevel::Block {
            level = SecurityLevel::Warning;
        }
        findings.push("context.md is missing.".to_string());
        recommendations.push("Run `repodesk context build` before using agents.".to_string());
    }

    if policy.require_checks_before_patch_agent
        && !task.config.run_dir.join("checks-summary.md").exists()
    {
        if level != SecurityLevel::Block {
            level = SecurityLevel::Warning;
        }
        findings.push("checks-summary.md is missing.".to_string());
        recommendations.push("Run `repodesk checks run` before patch-agent work.".to_string());
    }

    if project.path.parent().is_none() {
        level = SecurityLevel::Block;
        findings.push("Project path appears to be a filesystem root.".to_string());
        recommendations.push("Never register filesystem root as a project.".to_string());
    }

    if findings.is_empty() {
        findings.push("Security policy is conservative.".to_string());
        recommendations.push("Safe to continue with preflight-guarded agent workflow.".to_string());
    }

    Ok(SecurityAudit {
        level,
        findings,
        recommendations,
    })
}

pub fn explain_agent_security(agent: &str) -> RepoDeskResult<String> {
    let policy = ensure_security_policy()?;
    let normalized = agent.to_ascii_lowercase();
    let preflight = preflight(&normalized).ok();

    let mut output = String::new();
    output.push_str(&format!(
        "Security explanation for agent '{}':\n\n",
        normalized
    ));

    if policy.paid_agents.iter().any(|item| item == &normalized) {
        output.push_str("- This is treated as a paid/external agent.\n");
        output.push_str("- Send bounded context packs only.\n");
        output.push_str("- Do not send secrets, full logs, or whole repository dumps.\n");
    }

    if policy.patch_agents.iter().any(|item| item == &normalized) {
        output.push_str("- This is treated as a patch agent.\n");
        output
            .push_str("- It requires context, prompt file, checks summary, and guard preflight.\n");
        output.push_str("- It must not perform broad rewrites or touch unrelated modules.\n");
    }

    if let Some(result) = preflight {
        output.push_str(&format!(
            "- Current preflight level: {}.\n",
            result.level.as_label()
        ));
    } else {
        output.push_str("- Current preflight level: unknown.\n");
    }

    output.push_str("\nBlocked path patterns:\n");
    for pattern in &policy.blocked_path_patterns {
        output.push_str(&format!("  - {pattern}\n"));
    }

    Ok(output)
}

pub fn format_security_policy(policy: &SecurityPolicy) -> String {
    format!(
        r#"Security policy:

Shell:
  allow unrestricted shell: {}

Secrets:
  allow secret access: {}

Network:
  allow external network by default: {}

Requirements:
  require preflight for paid agents: {}
  require context before agent: {}
  require checks before patch agent: {}
  require prompt files before agent: {}

Paid agents:
{}

Patch agents:
{}

Blocked path patterns:
{}
"#,
        policy.allow_unrestricted_shell,
        policy.allow_secret_access,
        policy.allow_external_network_by_default,
        policy.require_preflight_for_paid_agents,
        policy.require_context_before_agent,
        policy.require_checks_before_patch_agent,
        policy.require_prompt_files_before_agent,
        format_list(&policy.paid_agents),
        format_list(&policy.patch_agents),
        format_list(&policy.blocked_path_patterns),
    )
}

pub fn format_security_audit(audit: &SecurityAudit) -> String {
    format!(
        r#"Security audit: {}

Findings:
{}

Recommendations:
{}
"#,
        audit.level.as_label(),
        format_list(&audit.findings),
        format_list(&audit.recommendations),
    )
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allow_unrestricted_shell: false,
            allow_secret_access: false,
            allow_external_network_by_default: false,
            require_preflight_for_paid_agents: true,
            require_context_before_agent: true,
            require_checks_before_patch_agent: true,
            require_prompt_files_before_agent: true,
            blocked_path_patterns: vec![
                ".env".to_string(),
                ".env.*".to_string(),
                "id_rsa".to_string(),
                "*.pem".to_string(),
                "*.key".to_string(),
                "secrets.*".to_string(),
                "credentials.*".to_string(),
            ],
            paid_agents: vec![
                "chatgpt".to_string(),
                "codex".to_string(),
                "gemini".to_string(),
            ],
            patch_agents: vec!["codex".to_string()],
        }
    }
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "  - none".to_string();
    }

    items
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}
