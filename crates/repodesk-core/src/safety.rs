use std::path::Path;

use crate::errors::RepoDeskResult;
use crate::security::scan_text_for_secrets;
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

    // The literal markers above only catch keyword-adjacent secrets (e.g. text
    // containing the literal string "aws_secret_access_key"). A bare key value
    // with no nearby keyword — an AKIA... access key ID, a Stripe/OpenAI/GitHub
    // token — would sail through undetected, even though this scan is the gate
    // `judge::judge_agent` relies on before allowing an external AI hand-off.
    // `security::scan_text_for_secrets` has real regexes for those formats;
    // fold its findings in here so the judge gate actually catches them.
    for kind in scan_text_for_secrets(text) {
        findings.push(SafetyFinding {
            level: SafetyLevel::Block,
            label: kind.clone(),
            reason: format!("{kind} pattern detected in content."),
            recommendation: "Remove secrets before sending this content to any external agent."
                .to_string(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_labels_are_stable() {
        assert_eq!(SafetyLevel::Ok.as_label(), "OK");
        assert_eq!(SafetyLevel::Warning.as_label(), "WARNING");
        assert_eq!(SafetyLevel::Block.as_label(), "BLOCK");
    }

    #[test]
    fn clean_text_produces_no_findings() {
        let report = scan_text("doc", "This is an ordinary line of documentation.");
        assert_eq!(report.level, SafetyLevel::Ok);
        assert!(report.findings.is_empty());
        assert_eq!(report.target, "doc");
    }

    #[test]
    fn private_key_marker_blocks() {
        let report = scan_text("ctx", "-----BEGIN PRIVATE KEY-----\nabc\n");
        assert_eq!(report.level, SafetyLevel::Block);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.level == SafetyLevel::Block)
        );
    }

    #[test]
    fn block_patterns_are_case_insensitive() {
        // The marker constant is upper-case; a lower-case occurrence must still match.
        let report = scan_text("ctx", "found aws_secret_access_key in env dump");
        assert_eq!(report.level, SafetyLevel::Block);
    }

    #[test]
    fn warning_patterns_escalate_to_warning_only() {
        let report = scan_text("ctx", "the api_key field needs a password");
        assert_eq!(report.level, SafetyLevel::Warning);
        assert!(!report.findings.is_empty());
        assert!(
            report
                .findings
                .iter()
                .all(|f| f.level == SafetyLevel::Warning)
        );
    }

    #[test]
    fn bare_secret_pattern_blocks_even_without_a_nearby_keyword() {
        // Regression: an AWS access key ID with no adjacent keyword like
        // "aws_secret_access_key" used to sail through this scan — the literal
        // markers only matched keyword text, not the actual key format. This
        // scan backs `judge::judge_agent`'s pre-AI-handoff gate, so a miss here
        // meant a real secret could reach a paid/cloud provider undetected.
        let report = scan_text("ctx", "config value: AKIAIOSFODNN7EXAMPLE");
        assert_eq!(report.level, SafetyLevel::Block);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.label.contains("AWS Access Key"))
        );
    }

    #[test]
    fn block_takes_precedence_over_warning() {
        // Contains both a warning marker ("password") and a block marker (private key).
        let report = scan_text("ctx", "password: x\n-----BEGIN OPENSSH PRIVATE KEY-----");
        assert_eq!(report.level, SafetyLevel::Block);
    }

    #[test]
    fn format_report_renders_empty_and_populated() {
        let clean = scan_text("ctx", "nothing here");
        let rendered = format_safety_report(&clean);
        assert!(rendered.contains("Safety scan: OK"));
        assert!(rendered.contains("No obvious secret or safety markers detected."));

        let dirty = scan_text("ctx", "-----BEGIN PRIVATE KEY-----");
        let rendered = format_safety_report(&dirty);
        assert!(rendered.contains("Safety scan: BLOCK"));
        assert!(rendered.contains("Findings:"));
        assert!(rendered.contains("reason:"));
        assert!(rendered.contains("recommendation:"));
    }

    #[test]
    fn rules_text_mentions_block_and_warning_handling() {
        let text = safety_rules_text();
        assert!(text.contains("BLOCK"));
        assert!(text.contains("WARNING"));
        assert!(text.contains("Never send secrets"));
    }
}
