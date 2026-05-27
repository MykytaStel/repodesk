use std::fs;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::tokens::TokenEstimate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub context_ok_limit: usize,
    pub context_warning_limit: usize,
    pub context_block_limit: usize,
    pub paid_agent_soft_limit: usize,
    pub paid_agent_hard_limit: usize,
    pub daily_soft_limit: usize,
    pub daily_hard_limit: usize,
    pub max_log_lines_for_paid_agent: usize,
    pub max_files_for_patch_agent: usize,
}

#[derive(Debug, Clone)]
pub struct BudgetVerdict {
    pub level: BudgetLevel,
    pub estimated_tokens: usize,
    pub reasons: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetLevel {
    Ok,
    Warning,
    Block,
}

impl BudgetLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            BudgetLevel::Ok => "OK",
            BudgetLevel::Warning => "WARNING",
            BudgetLevel::Block => "BLOCK",
        }
    }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            context_ok_limit: 8_000,
            context_warning_limit: 12_000,
            context_block_limit: 30_000,
            paid_agent_soft_limit: 10_000,
            paid_agent_hard_limit: 18_000,
            daily_soft_limit: 60_000,
            daily_hard_limit: 120_000,
            max_log_lines_for_paid_agent: 120,
            max_files_for_patch_agent: 8,
        }
    }
}

pub fn ensure_budget_config() -> RepoDeskResult<BudgetConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("budgets.toml");

    if file.exists() {
        return load_budget_config();
    }

    let config = BudgetConfig::default();
    let content = toml::to_string_pretty(&config)?;
    fs::write(file, content)?;

    Ok(config)
}

pub fn load_budget_config() -> RepoDeskResult<BudgetConfig> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("budgets.toml");

    if !file.exists() {
        return ensure_budget_config();
    }

    let content = fs::read_to_string(file)?;
    let config = toml::from_str(&content)?;

    Ok(config)
}

pub fn evaluate_context(estimate: &TokenEstimate, config: &BudgetConfig) -> BudgetVerdict {
    let mut reasons = Vec::new();
    let mut recommendations = Vec::new();

    let tokens = estimate.estimated_tokens;

    let level = if tokens >= config.context_block_limit {
        reasons.push(format!(
            "Context is above block limit: {} >= {} tokens.",
            tokens, config.context_block_limit
        ));
        recommendations.push("Do not send this context to paid agents directly.".to_string());
        recommendations.push("Compress with Ollama or split the task.".to_string());
        recommendations.push("Remove full logs, generated files, and unrelated docs.".to_string());
        BudgetLevel::Block
    } else if tokens >= config.context_warning_limit {
        reasons.push(format!(
            "Context is large: {} >= {} tokens.",
            tokens, config.context_warning_limit
        ));
        recommendations.push("Compress before sending to paid agents.".to_string());
        recommendations.push("Keep only files directly related to the task.".to_string());
        BudgetLevel::Warning
    } else if tokens >= config.context_ok_limit {
        reasons.push(format!(
            "Context is medium-sized: {} >= {} tokens.",
            tokens, config.context_ok_limit
        ));
        recommendations.push("Safe only for focused tasks. Avoid broad refactors.".to_string());
        BudgetLevel::Warning
    } else {
        reasons.push(format!(
            "Context is within the safe range: {} tokens.",
            tokens
        ));
        recommendations.push("Safe to use as bounded context.".to_string());
        BudgetLevel::Ok
    };

    if estimate.breakdown.code_like_chars > estimate.breakdown.prose_chars {
        recommendations.push(
            "Code-like content dominates. Prefer giving agents specific files, not full dumps."
                .to_string(),
        );
    }

    if estimate.breakdown.markdown_chars > 8_000 {
        recommendations.push(
            "Markdown/docs content is large. Consider summarizing docs before paid-agent use."
                .to_string(),
        );
    }

    if estimate.breakdown.cyrillic_chars > 3_000 {
        recommendations.push(
            "Cyrillic/prose content is significant. Keep instructions short and structured."
                .to_string(),
        );
    }

    if estimate.breakdown.symbol_chars > 5_000 {
        recommendations.push(
            "Symbol-heavy content detected. This often means code/diff/logs are token-expensive."
                .to_string(),
        );
    }

    BudgetVerdict {
        level,
        estimated_tokens: tokens,
        reasons,
        recommendations,
    }
}

pub fn format_budget_config(config: &BudgetConfig) -> String {
    format!(
        r#"Budget config:

Context:
  OK limit:        {}
  Warning limit:   {}
  Block limit:     {}

Paid agents:
  Soft limit:      {}
  Hard limit:      {}

Daily:
  Soft limit:      {}
  Hard limit:      {}

Safety:
  Max log lines for paid agent: {}
  Max files for patch agent:    {}
"#,
        config.context_ok_limit,
        config.context_warning_limit,
        config.context_block_limit,
        config.paid_agent_soft_limit,
        config.paid_agent_hard_limit,
        config.daily_soft_limit,
        config.daily_hard_limit,
        config.max_log_lines_for_paid_agent,
        config.max_files_for_patch_agent
    )
}

pub fn format_verdict(verdict: &BudgetVerdict) -> String {
    let reasons = verdict
        .reasons
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    let recommendations = verdict
        .recommendations
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Budget verdict: {}
Estimated tokens: {}

Reasons:
{}

Recommendations:
{}
"#,
        verdict.level.as_label(),
        verdict.estimated_tokens,
        reasons,
        recommendations
    )
}
