use anyhow::{Result, anyhow};

use crate::cli::OrchestrateCommand;
use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::orchestrator::{
    self, AgentWorkspacePolicy, ExecutionAuthorization, LoopOptions, LoopRun, OrchestrationPlan,
    OrchestrationRun, RunOptions, SubAgentTask, plan_has_paid_step,
};
use repodesk_core::worktree::{RunWorktreeCleanup, RunWorktreeStatus};

pub fn handle_orchestrate_command(command: OrchestrateCommand) -> Result<()> {
    match command {
        OrchestrateCommand::Plan { goal } => {
            let settings = ProviderSettings::from_env();
            let plan = orchestrator::build_plan(goal, &settings, None, None)?;
            print!("{}", format_plan(&plan));
        }
        OrchestrateCommand::Run {
            goal,
            dry_run,
            max_cost,
            yes,
            worktree,
        } => {
            let _legacy_worktree_flag = worktree;
            let settings = ProviderSettings::from_env();
            let plan = orchestrator::build_plan(goal, &settings, None, None)?;

            // The human stays the operator: paid/API and coding-agent runs need confirmation.
            if !dry_run && !yes && plan_has_paid_step(&plan) {
                print!("{}", format_plan(&plan));
                return Err(anyhow!(
                    "This plan includes paid provider or coding-agent steps. Re-run with --yes to execute, \
                     or use --dry-run to preview cost and routing without calling any provider."
                ));
            }

            let opts = RunOptions {
                dry_run,
                max_cost,
                settings,
                authorization: ExecutionAuthorization {
                    allow_paid_providers: yes,
                    allow_coding_agents: yes,
                    allow_workspace_writes: yes,
                },
                coding_agent_timeout_secs: 600,
                agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
            };
            let rt = tokio::runtime::Runtime::new()?;
            let run = rt.block_on(orchestrator::run_plan(&plan, &opts))?;
            print!("{}", format_run(&run));
        }
        OrchestrateCommand::Loop {
            goal,
            max_iterations,
            max_cost,
            dry_run,
            yes,
        } => {
            let settings = ProviderSettings::from_env();
            let opts = LoopOptions {
                max_iterations,
                max_total_cost: max_cost,
                dry_run,
                approve_paid: yes,
                approve_coding_agents: yes,
                coding_agent_timeout_secs: 600,
                override_provider: None,
                override_model: None,
                settings,
                agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
            };
            let rt = tokio::runtime::Runtime::new()?;
            let loop_run = rt.block_on(orchestrator::run_loop(goal, &opts))?;
            print!("{}", format_loop(&loop_run));
        }
        OrchestrateCommand::Status => match orchestrator::load_latest_run()? {
            Some(run) => print!("{}", format_run(&run)),
            None => println!("No orchestration runs yet for the active task."),
        },
        OrchestrateCommand::Show { run_id } => match orchestrator::load_run(&run_id)? {
            Some(run) => print!("{}", format_run(&run)),
            None => println!("No run '{run_id}' found for the active task."),
        },
        OrchestrateCommand::Review { run_id, action } => {
            let action = orchestrator::ReviewAction::from_label(&action)?;
            let review = orchestrator::review_run(&run_id, action)?;
            print!("{}", format_review(&review));
        }
        OrchestrateCommand::Worktrees => {
            let (project_path, parent) = active_worktree_context()?;
            let worktrees = repodesk_core::worktree::list_run_worktree_statuses(
                project_path.as_path(),
                parent.as_path(),
            )?;
            print!("{}", format_worktrees(&worktrees));
        }
        OrchestrateCommand::CleanupWorktree { workspace_id } => {
            let (project_path, parent) = active_worktree_context()?;
            let cleanup = repodesk_core::worktree::cleanup_run_worktree(
                project_path.as_path(),
                parent.as_path(),
                &workspace_id,
            )?;
            print!("{}", format_worktree_cleanup(&cleanup));
        }
    }
    Ok(())
}

fn active_worktree_context() -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let project = repodesk_core::projects::get_active_project()?;
    let task = repodesk_core::tasks::show_active_task()?;
    let parent = repodesk_core::worktree::worktrees_parent(&task.config.run_dir);
    Ok((project.path, parent))
}

fn format_review(review: &repodesk_core::orchestrator::RunReview) -> String {
    let verb = match review.action {
        repodesk_core::orchestrator::ReviewAction::Accept => "Accepted",
        repodesk_core::orchestrator::ReviewAction::Reject => "Rejected",
    };
    let mut out = format!(
        "{verb} changeset for run {} (project {})\n",
        review.run_id, review.project
    );
    if review.processed.is_empty() {
        out.push_str("  (no files processed)\n");
    }
    for file in &review.processed {
        out.push_str(&format!("  • {} — {}\n", file.path, file.outcome));
    }
    for warning in &review.warnings {
        out.push_str(&format!("  ! {warning}\n"));
    }
    out
}

fn format_worktrees(worktrees: &[RunWorktreeStatus]) -> String {
    if worktrees.is_empty() {
        return "No RepoDesk-managed isolated worktrees for the active task.\n".to_string();
    }

    let mut out = String::from("RepoDesk-managed isolated worktrees:\n");
    for worktree in worktrees {
        let run = worktree.run_id.as_deref().unwrap_or("unknown run");
        let step = worktree.step_id.as_deref().unwrap_or("unknown step");
        let state = if worktree.dirty { "dirty" } else { "clean" };
        out.push_str(&format!(
            "  - {} ({run}/{step}, {state}, {} changed)\n",
            worktree.workspace_id,
            worktree.changed_files.len()
        ));
        out.push_str(&format!("    path: {}\n", worktree.path));
        if !worktree.changed_files.is_empty() {
            out.push_str(&format!(
                "    changed: {}\n",
                worktree.changed_files.join(", ")
            ));
        }
        for warning in &worktree.warnings {
            out.push_str(&format!("    ! {warning}\n"));
        }
    }
    out
}

fn format_worktree_cleanup(cleanup: &RunWorktreeCleanup) -> String {
    let mut out = format!(
        "Removed worktree {} at {}\n",
        cleanup.workspace_id, cleanup.path
    );
    if cleanup.metadata_removed {
        out.push_str("Removed recovery metadata.\n");
    }
    for warning in &cleanup.warnings {
        out.push_str(&format!("! {warning}\n"));
    }
    out
}

fn format_plan(plan: &OrchestrationPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Orchestration plan for '{}' (task {})\nGoal: {}\n\nSteps:\n",
        plan.project, plan.task_id, plan.goal
    ));
    match plan.ordered() {
        Ok(steps) => {
            for step in steps {
                out.push_str(&format_step(step));
            }
        }
        Err(error) => out.push_str(&format!("  (plan ordering error: {error})\n")),
    }
    out
}

fn format_step(step: &SubAgentTask) -> String {
    let model = step.model.as_deref().unwrap_or("(provider default)");
    let deps = if step.depends_on.is_empty() {
        "none".to_string()
    } else {
        step.depends_on.join(", ")
    };
    format!(
        "  • {id}: {title}\n      executor/provider: {executor} → {provider} → model: {model}\n      thinking: {thinking:?}  write: {write}  depends on: {deps}\n",
        id = step.id,
        title = step.title,
        executor = step.resolved_executor_id(),
        provider = step.resolved_provider_id().unwrap_or("(none)"),
        thinking = step.thinking,
        write = step.allow_write,
    )
}

fn format_run(run: &OrchestrationRun) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Orchestration run {} — {:?}{}\nProject: {}  Task: {}\nGoal: {}\n\n",
        run.run_id,
        run.status,
        if run.dry_run { " (dry run)" } else { "" },
        run.project,
        run.task_id,
        run.goal,
    ));
    for result in &run.results {
        out.push_str(&format!(
            "  [{status:?}] {task} — {provider}/{model}\n      tokens: {input} in / {output} out  cost: {cost:.3}  captured: {captured}\n",
            status = result.status,
            task = result.task_id,
            provider = result.provider,
            model = if result.model.is_empty() {
                "(default)"
            } else {
                &result.model
            },
            input = result.input_tokens,
            output = result.output_tokens,
            cost = result.cost_units,
            captured = result.captured_proposals,
        ));
        for note in &result.notes {
            out.push_str(&format!("      note: {note}\n"));
        }
    }
    out.push_str(&format!(
        "\nTotals: {} in / {} out tokens, cost {:.3} units\n",
        run.total_input_tokens, run.total_output_tokens, run.total_cost_units
    ));
    if !run.dry_run {
        out.push_str("Review captured memory with `repodesk memory review`.\n");
    }
    out
}

fn format_loop(loop_run: &LoopRun) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Autonomous loop — {:?}\nProject: {}  Task: {}\nGoal: {}\n\nAttempts:\n",
        loop_run.status, loop_run.project, loop_run.task_id, loop_run.goal,
    ));
    for iteration in &loop_run.iterations {
        let run_ref = if iteration.run_id.is_empty() {
            "(no run)".to_string()
        } else {
            iteration.run_id.clone()
        };
        out.push_str(&format!(
            "  #{idx} [{status:?}] {run} — cost {cost:.3}\n      {note}\n",
            idx = iteration.index + 1,
            status = iteration.run_status,
            run = run_ref,
            cost = iteration.cost_units,
            note = iteration.note,
        ));
    }
    out.push_str(&format!(
        "\nTotal cost: {:.3} units over {} attempt(s).\n",
        loop_run.total_cost_units,
        loop_run.iterations.len(),
    ));
    match loop_run.status {
        repodesk_core::orchestrator::LoopStatus::NeedsApproval => out.push_str(
            "Paused: the plan includes paid or coding-agent steps. Re-run with --yes to allow execution.\n",
        ),
        repodesk_core::orchestrator::LoopStatus::GuardrailBlocked => {
            out.push_str("Stopped at a safety/budget guardrail — resolve it, then re-run.\n")
        }
        repodesk_core::orchestrator::LoopStatus::Exhausted => out
            .push_str("Out of attempts or budget before succeeding. Raise the limits or re-run.\n"),
        repodesk_core::orchestrator::LoopStatus::Succeeded => {
            out.push_str("Review captured memory with `repodesk memory review`.\n")
        }
        repodesk_core::orchestrator::LoopStatus::DryRun => {
            out.push_str("Preview only — nothing was executed.\n")
        }
    }
    out
}
