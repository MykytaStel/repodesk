#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessLevel {
    Allow,
    Warn,
    Block,
}

#[derive(Debug, Clone)]
pub struct AccessReport {
    pub agent: String,
    pub peripheral: String,
    pub level: AccessLevel,
    pub reason: String,
    pub recommendation: String,
}

impl AccessLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            AccessLevel::Allow => "ALLOW",
            AccessLevel::Warn => "WARN",
            AccessLevel::Block => "BLOCK",
        }
    }
}

pub fn evaluate_access(agent: &str, peripheral: &str) -> AccessReport {
    let agent = agent.to_ascii_lowercase();
    let peripheral = peripheral.to_ascii_lowercase();

    let (level, reason, recommendation) = match (agent.as_str(), peripheral.as_str()) {
        (_, "secrets") | (_, "credentials") => (
            AccessLevel::Block,
            "Secret and credential access is blocked for all agents.".to_string(),
            "Use redacted configuration or local-only manual inspection.".to_string(),
        ),
        ("chatgpt" | "gemini", "shell") => (
            AccessLevel::Block,
            "External planning/review agents must not receive shell control.".to_string(),
            "Use generated prompts and run shell checks locally through RepoDesk.".to_string(),
        ),
        ("chatgpt" | "gemini", "filesystem_write") => (
            AccessLevel::Block,
            "External review agents must not write local files.".to_string(),
            "Use them for planning/review only.".to_string(),
        ),
        ("codex", "shell") => (
            AccessLevel::Warn,
            "Patch agents may need checks, but unrestricted shell is risky.".to_string(),
            "Allow only a small command allowlist and prefer RepoDesk checks.".to_string(),
        ),
        ("codex", "filesystem_write") => (
            AccessLevel::Warn,
            "Patch agents can write files only for bounded tasks.".to_string(),
            "Run guard preflight and keep the patch scope small.".to_string(),
        ),
        ("ollama", "filesystem_read") => (
            AccessLevel::Warn,
            "Local AI may read broader context, but should still respect ignore rules.".to_string(),
            "Prefer context packs and summaries over raw repository dumps.".to_string(),
        ),
        (_, "filesystem_read") | (_, "context") | (_, "prompts") => (
            AccessLevel::Allow,
            "Bounded read access is allowed.".to_string(),
            "Keep access scoped to active project/task context.".to_string(),
        ),
        (_, "mcp_readonly") => (
            AccessLevel::Allow,
            "Read-only MCP-style access is acceptable when scoped.".to_string(),
            "Do not expose unrestricted filesystem or shell tools.".to_string(),
        ),
        _ => (
            AccessLevel::Warn,
            "Unknown agent/peripheral combination.".to_string(),
            "Review manually before granting access.".to_string(),
        ),
    };

    AccessReport {
        agent,
        peripheral,
        level,
        reason,
        recommendation,
    }
}

pub fn format_access_report(report: &AccessReport) -> String {
    format!(
        r#"Access: {}
Agent: {}
Peripheral: {}

Reason:
  - {}

Recommendation:
  - {}
"#,
        report.level.as_label(),
        report.agent,
        report.peripheral,
        report.reason,
        report.recommendation
    )
}

pub fn format_access_matrix() -> String {
    let checks = [
        ("chatgpt", "shell"),
        ("chatgpt", "context"),
        ("codex", "filesystem_write"),
        ("codex", "shell"),
        ("ollama", "filesystem_read"),
        ("gemini", "filesystem_write"),
        ("any", "secrets"),
        ("any", "mcp_readonly"),
    ];

    let mut output = String::new();
    output.push_str("Access matrix:\n\n");

    for (agent, peripheral) in checks {
        let report = evaluate_access(agent, peripheral);
        output.push_str(&format!(
            "- {} -> {}: {}\n",
            report.agent,
            report.peripheral,
            report.level.as_label()
        ));
    }

    output
}
