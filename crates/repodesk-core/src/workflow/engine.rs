use super::actions::find_action;
use super::types::*;

pub fn has_block_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("safety scan: block")
        || text.contains("security audit: block")
        || text.contains("private key")
        || text.contains("aws_secret_access_key")
}

pub fn has_warn_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("safety scan: warning") || text.contains("security audit: warning")
}

pub fn build_product_workflow_state(
    params: ProductWorkflowStateParams,
) -> ProductWorkflowState {
    let ProductWorkflowStateParams {
        generated_at_ms,
        project_info,
        task_status,
        workflow_hint,
        security_verdict,
        context,
        smart_context,
        prompt_codex,
        prompt_chatgpt,
        prompt_review,
        checks_summary,
        token_estimate,
        checks_summary_preview,
    } = params;
    let project_ok = project_info.ok;
    let task_ok = task_status.ok;
    let context_ok = context.exists;
    let smart_context_ok = smart_context.exists;
    let prompts_ok = prompt_codex.exists && prompt_chatgpt.exists && prompt_review.exists;
    let checks_ok = checks_summary.exists;
    let safety_ok = security_verdict.ok && !has_block_signal(&security_verdict);

    let mut recommended_action_id = None;
    let mut recommended_action_title = None;
    let mut primary_cta = "Review workflow".to_string();
    let mut overall_status = "ready".to_string();

    let mut choose = |action_id: &str, cta: &str, status: &str| {
        if let Some(action) = find_action(action_id) {
            recommended_action_id = Some(action.id);
            recommended_action_title = Some(action.title);
            primary_cta = cta.to_string();
            overall_status = status.to_string();
        }
    };

    if !project_ok {
        primary_cta = "Add or select a project".into();
        overall_status = "setup_required".into();
    } else if !task_ok {
        primary_cta = "Create an active task".into();
        overall_status = "setup_required".into();
    } else if !context_ok {
        choose("context-build", "Build context", "needs_context");
    } else if !smart_context_ok {
        choose(
            "smart-context-build",
            "Build smart context",
            "needs_smart_context",
        );
    } else if !safety_ok {
        choose(
            "safety-scan-context",
            "Scan context safety",
            "needs_safety_review",
        );
    } else if !prompts_ok {
        choose("prompt-all", "Generate prompts", "needs_prompts");
    } else if !checks_ok {
        choose("checks-run", "Run checks", "needs_checks");
    } else if has_warn_signal(&security_verdict) {
        choose("judge-codex", "Review AI readiness", "warning");
    } else {
        choose("workflow-next", "Ask brain for next step", "ready");
    }

    let steps = vec![
        WorkflowStep {
            id: "project".into(),
            title: "Project".into(),
            description:
                "RepoDesk needs an active project before it can build context or run checks.".into(),
            status: if project_ok { "done" } else { "current" }.into(),
            action_id: None,
            artifact_kind: None,
            command_preview: Some("repodesk project info".into()),
            blocker: if project_ok {
                None
            } else {
                Some("No active project".into())
            },
        },
        WorkflowStep {
            id: "task".into(),
            title: "Task".into(),
            description: "Every AI workflow should be scoped to one concrete task.".into(),
            status: if task_ok {
                "done"
            } else if project_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: None,
            artifact_kind: None,
            command_preview: Some("repodesk task status".into()),
            blocker: if task_ok {
                None
            } else {
                Some("No active task".into())
            },
        },
        WorkflowStep {
            id: "context".into(),
            title: "Context".into(),
            description: "Build the base task context from project state and task metadata.".into(),
            status: if context_ok {
                "done"
            } else if task_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("context-build".into()),
            artifact_kind: Some("context".into()),
            command_preview: Some("repodesk context build".into()),
            blocker: if task_ok {
                None
            } else {
                Some("Requires active task".into())
            },
        },
        WorkflowStep {
            id: "smart_context".into(),
            title: "Smart Context".into(),
            description:
                "Compress the working context so paid agents do not read unnecessary files.".into(),
            status: if smart_context_ok {
                "done"
            } else if context_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("smart-context-build".into()),
            artifact_kind: Some("smart_context".into()),
            command_preview: Some("repodesk smart-context build".into()),
            blocker: if context_ok {
                None
            } else {
                Some("Requires context.md".into())
            },
        },
        WorkflowStep {
            id: "safety".into(),
            title: "Safety".into(),
            description: "Judge the active context before handing it to AI.".into(),
            status: if safety_ok {
                "done"
            } else if smart_context_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("safety-scan-context".into()),
            artifact_kind: None,
            command_preview: Some("repodesk safety scan-context".into()),
            blocker: if smart_context_ok {
                None
            } else {
                Some("Requires smart context".into())
            },
        },
        WorkflowStep {
            id: "prompts".into(),
            title: "Prompts".into(),
            description: "Generate bounded prompts for Codex, ChatGPT, and review.".into(),
            status: if prompts_ok {
                "done"
            } else if safety_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("prompt-all".into()),
            artifact_kind: Some("prompt_codex".into()),
            command_preview: Some("repodesk prompt all".into()),
            blocker: if safety_ok {
                None
            } else {
                Some("Requires safety pass".into())
            },
        },
        WorkflowStep {
            id: "checks".into(),
            title: "Checks".into(),
            description: "Run configured project checks and keep only the useful summary for AI."
                .into(),
            status: if checks_ok {
                "done"
            } else if prompts_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("checks-run".into()),
            artifact_kind: Some("checks_summary".into()),
            command_preview: Some("repodesk checks run".into()),
            blocker: if prompts_ok {
                None
            } else {
                Some("Generate prompts first".into())
            },
        },
        WorkflowStep {
            id: "review".into(),
            title: "Review".into(),
            description:
                "Review prompts, checks, and action history before using an external agent.".into(),
            status: if project_ok && task_ok && smart_context_ok && prompts_ok {
                "current"
            } else {
                "blocked"
            }
            .into(),
            action_id: Some("judge-codex".into()),
            artifact_kind: Some("prompt_codex".into()),
            command_preview: Some("repodesk judge agent --agent codex".into()),
            blocker: None,
        },
    ];

    ProductWorkflowState {
        generated_at_ms,
        overall_status,
        primary_cta,
        recommended_action_id,
        recommended_action_title,
        steps,
        artifacts: vec![
            context,
            smart_context,
            prompt_codex,
            prompt_chatgpt,
            prompt_review,
            checks_summary,
            token_estimate,
        ],
        project_ok,
        task_ok,
        context_ok,
        smart_context_ok,
        prompts_ok,
        checks_ok,
        safety_ok,
        project_info,
        task_status,
        workflow_hint,
        security_verdict,
        checks_summary_preview,
    }
}
