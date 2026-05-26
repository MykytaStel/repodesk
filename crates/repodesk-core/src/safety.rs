use std::path::Path;

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyLevel {
    Ok,
    Warning,
    Block,
}

#[derive(Debug, Clone)]
pub struct SafetyFinding {
    pub level: SafetyLevel,
    pub label: String,
    pub reason: String,
    pub recommendation: String,
}

#[derive(Debug, Clone)]
pub struct SafetyReport {
    pub level: SafetyLevel,
    pub target: String,
    pub findings: Vec<SafetyFinding>,
}

impl SafetyLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            SafetyLevel::Ok => "OK",
            SafetyLevel::Warning => "WARNING",
            SafetyLevel::Block => "BLOCK",
        }
    }
}

pub fn scan_active_context() -> RepoDeskResult<SafetyReport> {
    let task = show_active_task()?;
    let context_file = task.config.run_dir.join("context.md");
    scan_file(&context_file)
}

pub fn scan_file(path: &Path) -> RepoDeskResult<SafetyReport> {
    let content = std::fs::read_to_string(path)?;
    Ok(scan_text(&path.display().to_string(), &content))
}

pub fn scan_text(target: &str, text: &str) -> SafetyReport {
    let lowered = text.to_ascii_lowercase();
    let mut findings = Vec::new();

    let block_patterns = [
        ("BEGIN PRIVATE KEY", "Private key material may be present."),
        (
            "aws_secret_access_key",
            "AWS secret access key marker detected.",
        ),
        (
            "github_pat_",
            "GitHub personal access token marker detected.",
        ),
        (
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "OpenSSH private key marker detected.",
        ),
    ];

    for (pattern, reason) in block_patterns {
        if text.contains(pattern) || lowered.contains(&pattern.to_ascii_lowercase()) {
            findings.push(SafetyFinding {
                level: SafetyLevel::Block,
                label: pattern.to_string(),
                reason: reason.to_string(),
                recommendation: "Remove secrets before sending this content to any external agent."
                    .to_string(),
            });
        }
    }

    let warning_patterns = [
        ("api_key", "API key-like text detected."),
        ("secret", "Secret-like text detected."),
        ("password", "Password-like text detected."),
        ("authorization:", "Authorization header-like text detected."),
        ("bearer ", "Bearer token-like text detected."),
        (".env", "Environment file reference detected."),
        ("credentials", "Credentials-like text detected."),
        ("token", "Token-like text detected."),
    ];

    for (pattern, reason) in warning_patterns {
        if lowered.contains(pattern) {
            findings.push(SafetyFinding {
                level: SafetyLevel::Warning,
                label: pattern.to_string(),
                reason: reason.to_string(),
                recommendation: "Review and redact sensitive values before external AI usage."
                    .to_string(),
            });
        }
    }

    let level = if findings.iter().any(|item| item.level == SafetyLevel::Block) {
        SafetyLevel::Block
    } else if findings
        .iter()
        .any(|item| item.level == SafetyLevel::Warning)
    {
        SafetyLevel::Warning
    } else {
        SafetyLevel::Ok
    };

    SafetyReport {
        level,
        target: target.to_string(),
        findings,
    }
}

pub fn format_safety_report(report: &SafetyReport) -> String {
    let mut output = String::new();

    output.push_str(&format!("Safety scan: {}\n", report.level.as_label()));
    output.push_str(&format!("Target: {}\n\n", report.target));

    if report.findings.is_empty() {
        output.push_str("No obvious secret or safety markers detected.\n");
        return output;
    }

    output.push_str("Findings:\n");
    for finding in &report.findings {
        output.push_str(&format!(
            "  - [{}] {}\n",
            finding.level.as_label(),
            finding.label
        ));
        output.push_str(&format!("    reason: {}\n", finding.reason));
        output.push_str(&format!("    recommendation: {}\n", finding.recommendation));
    }

    output
}

pub fn safety_rules_text() -> String {
    r#"Safety rules:

- Never send secrets, tokens, private keys, .env files, or credentials to external AI agents.
- Treat context.md as shareable only after safety scan passes.
- Local agents may inspect broader context, but still should not receive private keys.
- Patch agents should not get unrestricted shell access.
- Generated prompts should include only bounded task context.
- When safety is WARNING, manually review before sending.
- When safety is BLOCK, redact/split the context first.
"#
    .to_string()
}
