use serde::{Deserialize, Serialize};

use crate::usage::budget::{evaluate_context, load_budget_config, BudgetLevel};
use crate::context::estimate_active_context;
use crate::errors::RepoDeskResult;
use crate::event_journal::{log_event, LogEventInput};
use crate::guard::{preflight, GuardLevel};
use crate::safety::{scan_active_context, SafetyLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgementReport {
    pub decision: JudgementDecision,
    pub agent: String,
    pub reasons: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JudgementDecision {
    Allow,
    Warn,
    Block,
}

impl JudgementDecision {
    pub fn as_label(&self) -> &'static str {
        match self {
            JudgementDecision::Allow => "ALLOW",
            JudgementDecision::Warn => "WARN",
            JudgementDecision::Block => "BLOCK",
        }
    }
}

pub fn judge_agent(agent: &str) -> RepoDeskResult<JudgementReport> {
    let normalized_agent = agent.to_ascii_lowercase();
    let mut decision = JudgementDecision::Allow;
    let mut reasons = Vec::new();
    let mut recommendations = Vec::new();

    match preflight(&normalized_agent) {
        Ok(result) => match result.level {
            GuardLevel::Block => {
                decision = JudgementDecision::Block;
                reasons.push("Guard preflight blocks this agent.".to_string());
                recommendations.extend(result.recommendations);
            }
            GuardLevel::Warning => {
                if decision != JudgementDecision::Block {
                    decision = JudgementDecision::Warn;
                }
                reasons.push("Guard preflight warns before using this agent.".to_string());
                recommendations.extend(result.recommendations);
            }
            GuardLevel::Ok => {
                reasons.push("Guard preflight allows this agent.".to_string());
            }
        },
        Err(error) => {
            decision = JudgementDecision::Block;
            reasons.push(format!("Guard preflight failed: {error}"));
            recommendations.push("Fix workflow prerequisites before using an agent.".to_string());
        }
    }

    match scan_active_context() {
        Ok(report) => match report.level {
            SafetyLevel::Block => {
                decision = JudgementDecision::Block;
                reasons.push("Safety scanner found blocking content.".to_string());
                recommendations.push(
                    "Redact or split sensitive context before external AI usage.".to_string(),
                );
            }
            SafetyLevel::Warning => {
                if decision != JudgementDecision::Block {
                    decision = JudgementDecision::Warn;
                }
                reasons.push("Safety scanner found warning markers.".to_string());
                recommendations
                    .push("Manually review context before sending it externally.".to_string());
            }
            SafetyLevel::Ok => {
                reasons.push("Safety scanner found no obvious sensitive markers.".to_string());
            }
        },
        Err(error) => {
            if decision != JudgementDecision::Block {
                decision = JudgementDecision::Warn;
            }
            reasons.push(format!("Safety scan unavailable: {error}"));
            recommendations.push(
                "Run `repodesk context build` and `repodesk safety scan-context`.".to_string(),
            );
        }
    }

    match estimate_active_context() {
        Ok((_file, estimate)) => {
            let budget = load_budget_config()?;
            let verdict = evaluate_context(&estimate, &budget);

            match verdict.level {
                BudgetLevel::Block => {
                    decision = JudgementDecision::Block;
                    reasons.push("Budget verdict blocks this context.".to_string());
                    recommendations.extend(verdict.recommendations);
                }
                BudgetLevel::Warning => {
                    if decision != JudgementDecision::Block {
                        decision = JudgementDecision::Warn;
                    }
                    reasons.push("Budget verdict warns about this context.".to_string());
                    recommendations.extend(verdict.recommendations);
                }
                BudgetLevel::Ok => {
                    reasons.push("Budget verdict is OK.".to_string());
                }
            }
        }
        Err(error) => {
            if decision != JudgementDecision::Block {
                decision = JudgementDecision::Warn;
            }
            reasons.push(format!("Context estimate unavailable: {error}"));
            recommendations.push("Build context before asking a paid agent to work.".to_string());
        }
    }

    if recommendations.is_empty() {
        recommendations.push("Proceed with bounded generated prompt.".to_string());
    }

    let report = JudgementReport {
        decision,
        agent: normalized_agent,
        reasons,
        recommendations,
    };

    let _ = log_event(LogEventInput {
        module_name: "judge".to_string(),
        level: report.decision.as_label().to_ascii_lowercase(),
        message: format!("judged agent {}", report.agent),
        metadata: vec![(
            "decision".to_string(),
            report.decision.as_label().to_string(),
        )],
    });

    Ok(report)
}

pub fn format_judgement(report: &JudgementReport) -> String {
    let reasons = report
        .reasons
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    let recommendations = report
        .recommendations
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Judgement: {}
Agent: {}

Reasons:
{}

Recommendations:
{}
"#,
        report.decision.as_label(),
        report.agent,
        reasons,
        recommendations
    )
}
