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

/// Whether the recorded checks run passed.
///
/// Returns `None` when checks have not been run yet (no summary), `Some(true)`
/// when the summary reports an overall pass, and `Some(false)` on failure.
/// The checks summary always contains a line: ``Overall status: `passed`|`failed` ``.
pub fn checks_passed(checks_summary_preview: Option<&str>) -> Option<bool> {
    let text = checks_summary_preview?.to_lowercase();
    if text.contains("overall status: `passed`") {
        Some(true)
    } else if text.contains("overall status: `failed`") {
        Some(false)
    } else {
        None
    }
}

/// Whether RepoPilot, run as a configured check, gated the commit on new
/// High/Critical findings. The checks summary lists each command as
/// ``- `repopilot review . --fail-on new-high` => `failed` ...``, so a failed
/// line mentioning `repopilot` means its severity gate tripped.
pub fn repopilot_blocked(checks_summary_preview: Option<&str>) -> bool {
    let Some(text) = checks_summary_preview else {
        return false;
    };
    text.lines()
        .map(str::to_lowercase)
        .any(|line| line.contains("repopilot") && line.contains("=> `failed`"))
}

#[derive(Debug, Clone)]
pub struct CommitReadinessParams<'a> {
    pub git: &'a GitCommitContext,
    pub checks_passed: Option<bool>,
    pub repopilot_blocked: bool,
    pub safety_block: bool,
    pub safety_warn: bool,
}

/// Pure evaluation of whether the working tree is safe to commit.
///
/// Commit readiness is a *safety gate*, not an AI-readiness gate: it cares about
/// failing checks, secrets in context, RepoPilot High/Critical findings, and
/// merge conflicts — not whether prompts or smart context were generated.
pub fn evaluate_commit_readiness(params: CommitReadinessParams) -> CommitReadiness {
    let CommitReadinessParams {
        git,
        checks_passed,
        repopilot_blocked,
        safety_block,
        safety_warn,
    } = params;

    let base = |status: &str, headline: String| CommitReadiness {
        status: status.to_string(),
        headline,
        blockers: Vec::new(),
        warnings: Vec::new(),
        is_dirty: git.is_dirty,
        changed_count: git.changed_count,
        branch: git.branch.clone(),
    };

    if !git.is_repo {
        return base(
            "not_a_repo",
            "Active project is not a Git repository.".to_string(),
        );
    }

    let mut blockers = Vec::new();
    let mut warnings = Vec::new();

    if git.has_conflicts {
        blockers.push("Unresolved merge conflicts in the working tree.".to_string());
    }
    if safety_block {
        blockers.push("Safety scan found secrets/keys in the active context.".to_string());
    }
    if repopilot_blocked {
        blockers.push("RepoPilot flagged new High/Critical findings.".to_string());
    }
    match checks_passed {
        Some(false) => blockers.push("Project checks are failing.".to_string()),
        None => warnings.push("Checks have not been run for this change yet.".to_string()),
        Some(true) => {}
    }
    if safety_warn {
        warnings.push("Safety scan returned a warning; review before committing.".to_string());
    }

    if !git.is_dirty {
        let mut readiness = base(
            "nothing_to_commit",
            "Working tree is clean — nothing to commit.".to_string(),
        );
        // Surface latent blockers/warnings so they are still visible once edits begin.
        readiness.blockers = blockers;
        readiness.warnings = warnings;
        return readiness;
    }

    let (status, headline) = if !blockers.is_empty() {
        (
            "blocked",
            format!("Not safe to commit: {} blocker(s).", blockers.len()),
        )
    } else if !warnings.is_empty() {
        (
            "warning",
            format!(
                "{} change(s) ready, but review {} warning(s) first.",
                git.changed_count,
                warnings.len()
            ),
        )
    } else {
        (
            "ready",
            format!("{} change(s) ready to commit safely.", git.changed_count),
        )
    };

    CommitReadiness {
        status: status.to_string(),
        headline,
        blockers,
        warnings,
        is_dirty: git.is_dirty,
        changed_count: git.changed_count,
        branch: git.branch.clone(),
    }
}

pub fn build_product_workflow_state(params: ProductWorkflowStateParams) -> ProductWorkflowState {
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
        git,
    } = params;
    let project_ok = project_info.ok;
    let task_ok = task_status.ok;
    let context_ok = context.exists;
    let smart_context_ok = smart_context.exists;
    let prompts_ok = prompt_codex.exists && prompt_chatgpt.exists && prompt_review.exists;
    let checks_passed_verdict = checks_passed(checks_summary_preview.as_deref());
    // Checks are "ok" only when a summary exists AND it did not report failure.
    let checks_ok = checks_summary.exists && checks_passed_verdict != Some(false);
    let safety_block = !security_verdict.ok || has_block_signal(&security_verdict);
    let safety_ok = !safety_block;

    let commit_readiness = evaluate_commit_readiness(CommitReadinessParams {
        git: &git,
        checks_passed: checks_passed_verdict,
        repopilot_blocked: repopilot_blocked(checks_summary_preview.as_deref()),
        safety_block,
        safety_warn: has_warn_signal(&security_verdict),
    });

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
        // All AI-readiness gates pass — surface the commit verdict as the CTA.
        match commit_readiness.status.as_str() {
            "ready" => {
                primary_cta = "Ready to commit".into();
                overall_status = "ready_to_commit".into();
            }
            "warning" => {
                primary_cta = "Review before committing".into();
                overall_status = "commit_warning".into();
            }
            "blocked" => {
                primary_cta = "Resolve commit blockers".into();
                overall_status = "commit_blocked".into();
            }
            _ => choose("workflow-next", "Ask brain for next step", "ready"),
        }
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
        commit_readiness,
        project_info,
        task_status,
        workflow_hint,
        security_verdict,
        checks_summary_preview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirty_repo() -> GitCommitContext {
        GitCommitContext {
            is_repo: true,
            is_dirty: true,
            changed_count: 3,
            has_conflicts: false,
            branch: Some("feat/x".into()),
        }
    }

    fn params(git: &GitCommitContext) -> CommitReadinessParams<'_> {
        CommitReadinessParams {
            git,
            checks_passed: Some(true),
            repopilot_blocked: false,
            safety_block: false,
            safety_warn: false,
        }
    }

    #[test]
    fn checks_passed_reads_overall_status() {
        assert_eq!(
            checks_passed(Some("Overall status: `passed`\n")),
            Some(true)
        );
        assert_eq!(
            checks_passed(Some("# Checks\nOverall status: `failed`\n")),
            Some(false)
        );
        assert_eq!(checks_passed(Some("no status line here")), None);
        assert_eq!(checks_passed(None), None);
    }

    #[test]
    fn repopilot_blocked_detects_failed_repopilot_line() {
        let summary = "## Commands\n\n- `cargo test` => `passed` in 10ms\n- `repopilot review . --fail-on new-high` => `failed` in 50ms\n";
        assert!(repopilot_blocked(Some(summary)));
    }

    #[test]
    fn repopilot_not_blocked_when_passed_or_absent() {
        let passed = "- `repopilot review .` => `passed` in 50ms\n";
        assert!(!repopilot_blocked(Some(passed)));
        let other_failed = "- `cargo test` => `failed` in 10ms\n";
        assert!(!repopilot_blocked(Some(other_failed)));
        assert!(!repopilot_blocked(None));
    }

    #[test]
    fn ready_when_dirty_and_all_green() {
        let git = dirty_repo();
        let r = evaluate_commit_readiness(params(&git));
        assert_eq!(r.status, "ready");
        assert!(r.blockers.is_empty());
        assert_eq!(r.changed_count, 3);
    }

    #[test]
    fn clean_tree_is_nothing_to_commit() {
        let git = GitCommitContext {
            is_repo: true,
            is_dirty: false,
            ..Default::default()
        };
        let r = evaluate_commit_readiness(params(&git));
        assert_eq!(r.status, "nothing_to_commit");
    }

    #[test]
    fn non_repo_short_circuits() {
        let git = GitCommitContext::default();
        let r = evaluate_commit_readiness(params(&git));
        assert_eq!(r.status, "not_a_repo");
    }

    #[test]
    fn failing_checks_block_commit() {
        let git = dirty_repo();
        let mut p = params(&git);
        p.checks_passed = Some(false);
        let r = evaluate_commit_readiness(p);
        assert_eq!(r.status, "blocked");
        assert!(r.blockers.iter().any(|b| b.contains("checks are failing")));
    }

    #[test]
    fn safety_block_and_repopilot_are_blockers() {
        let git = dirty_repo();
        let mut p = params(&git);
        p.safety_block = true;
        p.repopilot_blocked = true;
        let r = evaluate_commit_readiness(p);
        assert_eq!(r.status, "blocked");
        assert_eq!(r.blockers.len(), 2);
    }

    #[test]
    fn merge_conflicts_block_commit() {
        let mut git = dirty_repo();
        git.has_conflicts = true;
        let r = evaluate_commit_readiness(params(&git));
        assert_eq!(r.status, "blocked");
        assert!(r.blockers.iter().any(|b| b.contains("conflict")));
    }

    #[test]
    fn warnings_without_blockers_are_warning_status() {
        let git = dirty_repo();
        let mut p = params(&git);
        p.checks_passed = None; // not run -> warning
        p.safety_warn = true;
        let r = evaluate_commit_readiness(p);
        assert_eq!(r.status, "warning");
        assert!(r.blockers.is_empty());
        assert_eq!(r.warnings.len(), 2);
    }

    #[test]
    fn block_takes_precedence_over_warning() {
        let git = dirty_repo();
        let p = CommitReadinessParams {
            git: &git,
            checks_passed: Some(false), // blocker
            repopilot_blocked: false,
            safety_block: false,
            safety_warn: true, // warning
        };
        let r = evaluate_commit_readiness(p);
        assert_eq!(r.status, "blocked");
    }
}
