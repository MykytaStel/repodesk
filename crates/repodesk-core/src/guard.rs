use std::fs;
use std::path::Path;
use std::process::Command;

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::estimate_file;
use crate::usage::budget::{evaluate_context, load_budget_config, BudgetLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardLevel {
    Ok,
    Warning,
    Block,
}

#[derive(Debug, Clone)]
pub struct GuardResult {
    pub level: GuardLevel,
    pub agent: String,
    pub reasons: Vec<String>,
    pub recommendations: Vec<String>,
}

impl GuardLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            GuardLevel::Ok => "OK",
            GuardLevel::Warning => "WARNING",
            GuardLevel::Block => "BLOCK",
        }
    }
}

pub fn preflight(agent: &str) -> RepoDeskResult<GuardResult> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let budget = load_budget_config()?;

    let normalized_agent = agent.to_ascii_lowercase();
    let mut level = GuardLevel::Ok;
    let mut reasons = Vec::new();
    let mut recommendations = Vec::new();

    let context_file = task.config.run_dir.join("context.md");

    if !context_file.exists() {
        level = GuardLevel::Block;
        reasons.push("context.md is missing for the active task.".to_string());
        recommendations
            .push("Run `repodesk context build` before sending anything to an agent.".to_string());
    } else {
        let estimate = estimate_file(&context_file)?;
        let verdict = evaluate_context(&estimate, &budget);

        match verdict.level {
            BudgetLevel::Block => {
                level = GuardLevel::Block;
                reasons.push("Context budget verdict is BLOCK.".to_string());
                recommendations
                    .push("Compress or split the task before paid-agent use.".to_string());
            }
            BudgetLevel::Warning => {
                if level != GuardLevel::Block {
                    level = GuardLevel::Warning;
                }
                reasons.push("Context budget verdict is WARNING.".to_string());
                recommendations.push("Prefer compression or a narrower context pack.".to_string());
            }
            BudgetLevel::Ok => {
                reasons.push("Context budget verdict is OK.".to_string());
            }
        }
    }

    if let Some(prompt_file) = expected_prompt_file(&task.config.run_dir, &normalized_agent) {
        if !prompt_file.exists() {
            if level != GuardLevel::Block {
                level = GuardLevel::Warning;
            }
            reasons.push(format!(
                "Expected prompt file is missing: {}",
                prompt_file.display()
            ));
            recommendations.push("Run `repodesk prompt all` before using the agent.".to_string());
        }
    }

    let summary_file = task.config.run_dir.join("checks-summary.md");

    if !summary_file.exists() {
        if level != GuardLevel::Block {
            level = GuardLevel::Warning;
        }
        reasons.push("checks-summary.md is missing.".to_string());
        recommendations
            .push("Run `repodesk checks run` before asking a patch agent to continue.".to_string());
    } else {
        let summary = fs::read_to_string(&summary_file).unwrap_or_default();
        let lowered = summary.to_ascii_lowercase();

        if lowered.contains("overall status: `failed`") || lowered.contains("status: failed") {
            if level != GuardLevel::Block {
                level = GuardLevel::Warning;
            }
            reasons.push("Last checks summary appears to contain failing checks.".to_string());
            recommendations.push("Fix the first failing check before expanding scope.".to_string());
        }
    }

    if normalized_agent == "codex" {
        let changed_files = count_changed_files(&project.path);

        if changed_files > budget.max_files_for_patch_agent * 2 {
            level = GuardLevel::Block;
            reasons.push(format!(
                "Too many changed files for a patch agent: {} > {}.",
                changed_files,
                budget.max_files_for_patch_agent * 2
            ));
            recommendations.push("Split the task before using Codex.".to_string());
        } else if changed_files > budget.max_files_for_patch_agent {
            if level != GuardLevel::Block {
                level = GuardLevel::Warning;
            }
            reasons.push(format!(
                "Many changed files for a patch agent: {} > {}.",
                changed_files, budget.max_files_for_patch_agent
            ));
            recommendations
                .push("Ask Codex for a very bounded patch or reduce the diff first.".to_string());
        }
    }

    if level == GuardLevel::Ok {
        recommendations.push(
            "Safe to proceed with the selected agent using the generated prompt.".to_string(),
        );
    }

    Ok(GuardResult {
        level,
        agent: normalized_agent,
        reasons,
        recommendations,
    })
}

pub fn format_guard_result(result: &GuardResult) -> String {
    let reasons = result
        .reasons
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    let recommendations = result
        .recommendations
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Preflight: {}
Agent: {}

Reasons:
{}

Recommendations:
{}
"#,
        result.level.as_label(),
        result.agent,
        reasons,
        recommendations
    )
}

fn expected_prompt_file(run_dir: &Path, agent: &str) -> Option<std::path::PathBuf> {
    match agent {
        "codex" => Some(run_dir.join("prompt.codex.md")),
        "chatgpt" => Some(run_dir.join("prompt.chatgpt.md")),
        "review" | "gemini" => Some(run_dir.join("prompt.review.md")),
        _ => None,
    }
}

fn count_changed_files(project_path: &Path) -> usize {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(project_path)
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        _ => 0,
    }
}
