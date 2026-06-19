use regex::Regex;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::guard::preflight;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::utils::format_list;

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
    
    // Enterprise Policy Additions
    pub allowed_models: Option<Vec<String>>,
    pub budget_limit_tokens: Option<u64>,
    pub policy_source_url: Option<String>,
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

impl crate::utils::ConfigStore for SecurityPolicy {
    const FILE_NAME: &'static str = "security.toml";
}

pub fn ensure_security_policy() -> RepoDeskResult<SecurityPolicy> {
    use crate::utils::ConfigStore;
    SecurityPolicy::ensure_config()
}

pub fn load_security_policy() -> RepoDeskResult<SecurityPolicy> {
    use crate::utils::ConfigStore;
    SecurityPolicy::load_config()
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
    let base_format = format!(
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
    );
    
    let enterprise_section = format!(
        "\nEnterprise Policy:\n  allowed models: {}\n  budget limit tokens: {}\n  policy source url: {}\n",
        policy.allowed_models.as_ref().map(|l| l.join(", ")).unwrap_or_else(|| "Any".to_string()),
        policy.budget_limit_tokens.map(|l| l.to_string()).unwrap_or_else(|| "Unlimited".to_string()),
        policy.policy_source_url.as_deref().unwrap_or("Local")
    );
    
    base_format + &enterprise_section
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
            allowed_models: None,
            budget_limit_tokens: None,
            policy_source_url: None,
        }
    }
}

pub fn is_blocked_path(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    let blocked_fragments = [".env", "secret", "credential", "private", "token", "id_rsa"];
    let blocked_suffixes = [
        ".pem", ".key", ".p12", ".pfx", ".sqlite", ".db", ".png", ".jpg", ".jpeg", ".gif", ".webp",
        ".pdf", ".zip",
    ];

    if blocked_fragments.iter().any(|item| lower.contains(item)) {
        return Some("secret-like path blocked".into());
    }

    if blocked_suffixes.iter().any(|item| lower.ends_with(item)) {
        return Some("binary or sensitive file type blocked".into());
    }

    None
}

pub fn scan_text_for_secrets(text: &str) -> Vec<String> {
    use std::sync::OnceLock;

    let mut findings = Vec::new();

    // AWS Access Key ID
    static AWS_REGEX: OnceLock<Regex> = OnceLock::new();
    let aws_re = AWS_REGEX.get_or_init(|| Regex::new(r"AKIA[0-9A-Z]{16}").unwrap());
    if aws_re.is_match(text) {
        findings.push("AWS Access Key ID".to_string());
    }

    // Stripe keys
    static STRIPE_REGEX: OnceLock<Regex> = OnceLock::new();
    let stripe_re =
        STRIPE_REGEX.get_or_init(|| Regex::new(r"(sk_live|rk_live)_[0-9a-zA-Z]{24}").unwrap());
    if stripe_re.is_match(text) {
        findings.push("Stripe Live Key".to_string());
    }

    // Generic API keys/tokens (heuristic)
    static GENERIC_REGEX: OnceLock<Regex> = OnceLock::new();
    let generic_re = GENERIC_REGEX.get_or_init(|| {
        Regex::new(r#"(?i)(api_key|token|secret)[=:]\s*['"]?[a-zA-Z0-9_-]{20,}['"]?"#).unwrap()
    });
    if generic_re.is_match(text) {
        findings.push("Generic API Key or Token".to_string());
    }

    // Private keys
    if text.contains("-----BEGIN RSA PRIVATE KEY-----")
        || text.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        || text.contains("-----BEGIN PRIVATE KEY-----")
    {
        findings.push("Private Key block".to_string());
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_labels_are_stable() {
        assert_eq!(SecurityLevel::Ok.as_label(), "OK");
        assert_eq!(SecurityLevel::Warning.as_label(), "WARNING");
        assert_eq!(SecurityLevel::Block.as_label(), "BLOCK");
    }

    #[test]
    fn default_policy_is_conservative() {
        let policy = SecurityPolicy::default();
        assert!(!policy.allow_unrestricted_shell);
        assert!(!policy.allow_secret_access);
        assert!(!policy.allow_external_network_by_default);
        assert!(policy.require_preflight_for_paid_agents);
        assert!(policy.require_context_before_agent);
        assert!(policy.require_checks_before_patch_agent);
        assert!(policy.require_prompt_files_before_agent);
        assert!(policy.patch_agents.contains(&"codex".to_string()));
        assert!(policy.paid_agents.contains(&"codex".to_string()));
        assert!(!policy.blocked_path_patterns.is_empty());
    }

    #[test]
    fn blocked_path_flags_secret_fragments() {
        assert!(is_blocked_path(".env").is_some());
        assert!(is_blocked_path("config/.env.local").is_some());
        assert!(is_blocked_path("home/user/id_rsa").is_some());
        assert!(is_blocked_path("my_secret_notes.txt").is_some());
        assert!(is_blocked_path("path/to/credentials.json").is_some());
    }

    #[test]
    fn blocked_path_flags_sensitive_suffixes() {
        assert!(is_blocked_path("server.pem").is_some());
        assert!(is_blocked_path("key.p12").is_some());
        assert!(is_blocked_path("data/app.sqlite").is_some());
        assert!(is_blocked_path("diagram.png").is_some());
    }

    #[test]
    fn blocked_path_is_case_insensitive() {
        assert!(is_blocked_path("Config/.ENV").is_some());
        assert!(is_blocked_path("Server.PEM").is_some());
    }

    #[test]
    fn ordinary_source_paths_are_allowed() {
        assert!(is_blocked_path("src/main.rs").is_none());
        assert!(is_blocked_path("crates/repodesk-core/src/security.rs").is_none());
        assert!(is_blocked_path("README.md").is_none());
    }

    #[test]
    fn secret_scan_detects_known_markers() {
        assert_eq!(
            scan_text_for_secrets("key=AKIAIOSFODNN7EXAMPLE"),
            vec!["AWS Access Key ID".to_string()]
        );
        assert!(
            scan_text_for_secrets("sk_live_0123456789abcdefghij0123")
                .contains(&"Stripe Live Key".to_string())
        );
        assert!(
            scan_text_for_secrets("api_key=abcdefghijklmnopqrstuvwxyz")
                .contains(&"Generic API Key or Token".to_string())
        );
        assert!(
            scan_text_for_secrets("-----BEGIN RSA PRIVATE KEY-----")
                .contains(&"Private Key block".to_string())
        );
    }

    #[test]
    fn secret_scan_is_clean_on_ordinary_text() {
        assert!(scan_text_for_secrets("just some regular prose without secrets").is_empty());
        // A short token value should not trip the 20+ char generic heuristic.
        assert!(scan_text_for_secrets("token=short").is_empty());
    }

    #[test]
    fn secret_scan_reports_multiple_findings() {
        let text = "AKIAIOSFODNN7EXAMPLE and -----BEGIN PRIVATE KEY-----";
        let findings = scan_text_for_secrets(text);
        assert!(findings.contains(&"AWS Access Key ID".to_string()));
        assert!(findings.contains(&"Private Key block".to_string()));
    }

    #[test]
    fn format_security_policy_renders_key_fields() {
        let rendered = format_security_policy(&SecurityPolicy::default());
        assert!(rendered.contains("allow unrestricted shell: false"));
        assert!(rendered.contains("Blocked path patterns:"));
    }
}
