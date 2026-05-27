use crate::checks::last_checks;
use crate::errors::RepoDeskResult;
use crate::guard::{preflight, GuardLevel};
use crate::projects::get_active_project;
use crate::security::{audit_security_policy, SecurityLevel};
use crate::tasks::show_active_task;
use crate::tokens::estimate_file;
use crate::usage::budget::{evaluate_context, load_budget_config};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorLevel {
    Ok,
    Warning,
    Block,
}

#[derive(Debug, Clone)]
pub struct WorkflowDoctorReport {
    pub level: DoctorLevel,
    pub findings: Vec<String>,
    pub next_actions: Vec<String>,
    pub safe_for_codex: bool,
    pub safe_for_chatgpt: bool,
}

impl DoctorLevel {
    pub fn as_label(&self) -> &'static str {
        match self {
            DoctorLevel::Ok => "OK",
            DoctorLevel::Warning => "WARNING",
            DoctorLevel::Block => "BLOCK",
        }
    }
}

pub fn diagnose_workflow() -> RepoDeskResult<WorkflowDoctorReport> {
    let project = get_active_project()?;
    let task = show_active_task()?;

    let mut level = DoctorLevel::Ok;
    let mut findings = Vec::new();
    let mut next_actions = Vec::new();

    findings.push(format!("Active project: {}", project.name));
    findings.push(format!("Active task: {}", task.config.title));

    let context_file = task.config.run_dir.join("context.md");
    let codex_prompt = task.config.run_dir.join("prompt.codex.md");
    let chatgpt_prompt = task.config.run_dir.join("prompt.chatgpt.md");
    let review_prompt = task.config.run_dir.join("prompt.review.md");
    let checks_summary = task.config.run_dir.join("checks-summary.md");

    if !context_file.exists() {
        level = max_level(level, DoctorLevel::Block);
        findings.push("Missing context.md.".to_string());
        next_actions.push("Run `repodesk context build`.".to_string());
    } else {
        let estimate = estimate_file(&context_file)?;
        let budget = load_budget_config()?;
        let verdict = evaluate_context(&estimate, &budget);

        findings.push(format!(
            "Context exists: {} estimated tokens, budget {}.",
            estimate.estimated_tokens,
            verdict.level.as_label()
        ));

        match verdict.level.as_label() {
            "BLOCK" => {
                level = max_level(level, DoctorLevel::Block);
                next_actions.push("Reduce/compress context before using paid agents.".to_string());
            }
            "WARNING" => {
                level = max_level(level, DoctorLevel::Warning);
                next_actions
                    .push("Consider compressing context or narrowing task scope.".to_string());
            }
            _ => {}
        }
    }

    if !codex_prompt.exists() || !chatgpt_prompt.exists() || !review_prompt.exists() {
        level = max_level(level, DoctorLevel::Warning);
        findings.push("One or more prompt files are missing.".to_string());
        next_actions.push("Run `repodesk prompt all`.".to_string());
    } else {
        findings.push("Prompt files exist.".to_string());
    }

    if !checks_summary.exists() {
        level = max_level(level, DoctorLevel::Warning);
        findings.push("checks-summary.md is missing.".to_string());
        next_actions.push("Run `repodesk checks run` before patch-agent work.".to_string());
    } else {
        let last = last_checks();
        match last {
            Ok(result) => findings.push(format!(
                "Checks summary exists: {}",
                result.summary_file.display()
            )),
            Err(_) => findings.push("Checks summary exists but could not be loaded.".to_string()),
        }
    }

    let codex_guard = preflight("codex")?;
    let chatgpt_guard = preflight("chatgpt")?;

    findings.push(format!("Codex preflight: {}", codex_guard.level.as_label()));
    findings.push(format!(
        "ChatGPT preflight: {}",
        chatgpt_guard.level.as_label()
    ));

    if codex_guard.level == GuardLevel::Block || chatgpt_guard.level == GuardLevel::Block {
        level = max_level(level, DoctorLevel::Block);
        next_actions.push("Resolve guard BLOCK items before using agents.".to_string());
    } else if codex_guard.level == GuardLevel::Warning || chatgpt_guard.level == GuardLevel::Warning
    {
        level = max_level(level, DoctorLevel::Warning);
    }

    let security = audit_security_policy()?;
    findings.push(format!("Security audit: {}", security.level.as_label()));

    if security.level == SecurityLevel::Block {
        level = max_level(level, DoctorLevel::Block);
        next_actions.push("Fix security policy BLOCK findings before agent work.".to_string());
    } else if security.level == SecurityLevel::Warning {
        level = max_level(level, DoctorLevel::Warning);
    }

    if next_actions.is_empty() {
        next_actions
            .push("Workflow is ready. Use a bounded prompt for the next agent step.".to_string());
    }

    let safe_for_codex = codex_guard.level != GuardLevel::Block && level != DoctorLevel::Block;
    let safe_for_chatgpt = chatgpt_guard.level != GuardLevel::Block && level != DoctorLevel::Block;

    Ok(WorkflowDoctorReport {
        level,
        findings,
        next_actions,
        safe_for_codex,
        safe_for_chatgpt,
    })
}

pub fn format_workflow_doctor_report(report: &WorkflowDoctorReport) -> String {
    format!(
        r#"Workflow doctor: {}

Safe routing:
  Codex: {}
  ChatGPT: {}

Findings:
{}

Next actions:
{}
"#,
        report.level.as_label(),
        yes_no(report.safe_for_codex),
        yes_no(report.safe_for_chatgpt),
        format_list(&report.findings),
        format_list(&report.next_actions),
    )
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
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

fn max_level(left: DoctorLevel, right: DoctorLevel) -> DoctorLevel {
    if weight(&right) > weight(&left) {
        right
    } else {
        left
    }
}

fn weight(level: &DoctorLevel) -> usize {
    match level {
        DoctorLevel::Ok => 0,
        DoctorLevel::Warning => 1,
        DoctorLevel::Block => 2,
    }
}
