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
        policy
            .allowed_models
            .as_ref()
            .map(|l| l.join(", "))
            .unwrap_or_else(|| "Any".to_string()),
        policy
            .budget_limit_tokens
            .map(|l| l.to_string())
            .unwrap_or_else(|| "Unlimited".to_string()),
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

/// Classify a repository-relative path at the central file-access boundary.
///
/// The policy intentionally reasons about path segments, basenames, extensions
/// and delimiter-separated words. It does **not** block arbitrary substrings:
/// ordinary source files such as `tokens.ts`, `privateRoute.tsx`, or
/// `credentials_form.tsx` must remain editable just because their domain uses a
/// security-related word.
pub fn is_blocked_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);

    // Dotenv files follow a concrete naming convention.
    if file_name == ".env" || file_name.starts_with(".env.") || file_name.ends_with(".env") {
        return Some("secret-like path blocked".into());
    }

    // Known credential stores/directories are sensitive regardless of the file
    // nested inside them.
    if normalized
        .split('/')
        .any(|segment| matches!(segment, ".ssh" | ".aws" | ".gnupg"))
    {
        return Some("credential store path blocked".into());
    }

    let blocked_exact_names = [
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        ".netrc",
        ".npmrc",
        ".pypirc",
        "credentials",
        "token",
        "secret",
    ];
    if blocked_exact_names.contains(&file_name) {
        return Some("secret-like path blocked".into());
    }

    let blocked_suffixes = [
        ".pem", ".key", ".p12", ".pfx", ".sqlite", ".db", ".png", ".jpg", ".jpeg",
        ".gif", ".webp", ".pdf", ".zip",
    ];
    if blocked_suffixes
        .iter()
        .any(|item| file_name.ends_with(item))
    {
        return Some("binary or sensitive file type blocked".into());
    }

    // Source-code filenames are allowed to use domain words like `token` and
    // `credentials`. For non-source artifacts/config files, delimiter-separated
    // secret nouns are treated conservatively.
    let source_extensions = [
        "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "kt", "kts", "swift",
        "c", "h", "cc", "cpp", "cxx", "hpp", "cs", "rb", "php", "ex", "exs", "scala", "sh", "zsh",
        "fish",
    ];
    let extension = file_name.rsplit_once('.').map(|(_, ext)| ext);
    let source_like = extension.is_some_and(|ext| source_extensions.contains(&ext));
    if !source_like {
        let sensitive_words = [
            "secret",
            "secrets",
            "credential",
            "credentials",
            "token",
            "tokens",
        ];
        let has_sensitive_word = file_name
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .any(|word| sensitive_words.contains(&word));
        if has_sensitive_word {
            return Some("secret-like path blocked".into());
        }
    }

    None
}

/// The secret detectors shared by [`scan_text_for_secrets`] and
/// [`redact_secrets`]: `(kind, regex)`. Compiled once.
fn secret_detectors() -> &'static [(&'static str, Regex)] {
    use std::sync::OnceLock;
    static DETECTORS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    DETECTORS.get_or_init(|| {
        vec![
            ("AWS Access Key ID", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
            (
                "Stripe Live Key",
                Regex::new(r"(sk_live|rk_live)_[0-9a-zA-Z]{24}").unwrap(),
            ),
            (
                "OpenAI/Anthropic API Key",
                Regex::new(r"sk-(ant-)?[A-Za-z0-9_-]{20,}").unwrap(),
            ),
            (
                "GitHub Token",
                Regex::new(r"gh[pousr]_[A-Za-z0-9]{20,}").unwrap(),
            ),
            (
                "Generic API Key or Token",
                Regex::new(r#"(?i)(api[_-]?key|token|secret|password)["']?\s*[=:]\s*['"]?[a-zA-Z0-9_\-./+]{16,}['"]?"#)
                    .unwrap(),
            ),
            (
                "Private Key block",
                Regex::new(
                    r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----|-----BEGIN [A-Z ]*PRIVATE KEY-----",
                )
                .unwrap(),
            ),
        ]
    })
}

pub fn scan_text_for_secrets(text: &str) -> Vec<String> {
    secret_detectors()
        .iter()
        .filter(|(_, re)| re.is_match(text))
        .map(|(kind, _)| (*kind).to_string())
        .collect()
}

/// Replace secret-looking substrings with `[REDACTED:<kind>]` markers so the
/// text is safe to persist and surface. Returns the redacted text and the
/// distinct kinds that were redacted. Used to sanitize coding-agent output
/// before it is written to a run record or handed to the Memory Brain.
pub fn redact_secrets(text: &str) -> (String, Vec<String>) {
    let mut redacted = text.to_string();
    let mut kinds = Vec::new();
    for (kind, re) in secret_detectors() {
        if re.is_match(&redacted) {
            redacted = re
                .replace_all(&redacted, format!("[REDACTED:{kind}]").as_str())
                .into_owned();
            kinds.push((*kind).to_string());
        }
    }
    (redacted, kinds)
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
    fn blocked_path_flags_secret_artifacts_not_source_vocabulary() {
        assert!(is_blocked_path(".env").is_some());
        assert!(is_blocked_path("config/.env.local").is_some());
        assert!(is_blocked_path("home/user/id_rsa").is_some());
        assert!(is_blocked_path("my_secret_notes.txt").is_some());
        assert!(is_blocked_path("path/to/credentials.json").is_some());

        assert!(is_blocked_path("src/tokens.ts").is_none());
        assert!(is_blocked_path("src/privateRoute.tsx").is_none());
        assert!(is_blocked_path("src/credentials_form.tsx").is_none());
        assert!(is_blocked_path("src/my_secret_sauce.rs").is_none());
    }

    #[test]
    fn blocked_path_flags_sensitive_suffixes_and_stores() {
        assert!(is_blocked_path("server.pem").is_some());
        assert!(is_blocked_path("key.p12").is_some());
        assert!(is_blocked_path("data/app.sqlite").is_some());
        assert!(is_blocked_path("diagram.png").is_some());
        assert!(is_blocked_path("~/.ssh/config").is_some());
        assert!(is_blocked_path("C:\\Users\\me\\.aws\\credentials").is_some());
    }

    #[test]
    fn blocked_path_is_case_insensitive() {
        assert!(is_blocked_path("Config/.ENV").is_some());
        assert!(is_blocked_path("Server.PEM").is_some());
    }

    #[test]
    fn env_fragment_does_not_false_positive_on_unrelated_filenames() {
        assert!(is_blocked_path("src/message.envelope.ts").is_none());
        assert!(is_blocked_path("config/release.environment.yml").is_none());
        assert!(is_blocked_path("docs/environment.md").is_none());
        assert!(is_blocked_path(".env").is_some());
        assert!(is_blocked_path("config/.env.local").is_some());
        assert!(is_blocked_path("config/.env.production").is_some());
        assert!(is_blocked_path("legacy/config.env").is_some());
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
