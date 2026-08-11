use anyhow::Result;
use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::orchestrator::{
    AgentWorkspacePolicy, ExecutionAuthorization, LoopOptions, LoopRun, OrchestrationPlan,
    OrchestrationRun, ReviewAction, RunOptions, build_plan, execution_preview, list_runs,
    load_latest_run, load_run, review_run, run_loop, run_plan,
};

use crate::cli::{OrchestrateAction, OrchestrateArgs};

pub async fn run(args: OrchestrateArgs) -> Result<String> {
    match args.action {
        OrchestrateAction::Plan {
            goal,
            provider,
            model,
        } => {
            let settings = ProviderSettings::from_env();
            let plan = build_plan(goal, &settings, provider, model)?;
            Ok(format_plan(&plan))
        }
        OrchestrateAction::Preview {
            goal,
            provider,
            model,
        } => {
            let settings = ProviderSettings::from_env();
            let preview = execution_preview(goal, &settings, provider, model)?;
            Ok(serde_json::to_string_pretty(&preview)?)
        }
        OrchestrateAction::Run {
            goal,
            dry_run,
            max_cost,
            yes,
            provider,
            model,
        } => {
            let settings = ProviderSettings::from_env();
            let plan = build_plan(goal, &settings, provider, model)?;
            let run = run_plan(
                &plan,
                &RunOptions {
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
                },
            )
            .await?;
            Ok(format_run(&run))
        }
        OrchestrateAction::Loop {
            goal,
            max_iterations,
            max_cost,
            dry_run,
            yes,
            provider,
            model,
        } => {
            let loop_run = run_loop(
                goal,
                &LoopOptions {
                    max_iterations,
                    max_total_cost: max_cost,
                    dry_run,
                    approve_paid: yes,
                    approve_coding_agents: yes,
                    coding_agent_timeout_secs: 600,
                    override_provider: provider,
                    override_model: model,
                    settings: ProviderSettings::from_env(),
                    agent_workspace_policy: AgentWorkspacePolicy::IsolatedRequired,
                },
            )
            .await?;
            Ok(format_loop(&loop_run))
        }
        OrchestrateAction::Status => Ok(load_latest_run()?
            .map(|run| format_run(&run))
            .unwrap_or_else(|| "No orchestration runs for the active task.\n".to_string())),
        OrchestrateAction::Show { run_id } => Ok(load_run(&run_id)?
            .map(|run| format_run(&run))
            .unwrap_or_else(|| format!("No orchestration run '{run_id}' for the active task.\n"))),
        OrchestrateAction::List => {
            let runs = list_runs()?;
            if runs.is_empty() {
                return Ok("No orchestration runs for the active task.\n".to_string());
            }
            let mut out = String::from("Orchestration runs\n\n");
            for run in runs {
                out.push_str(&format!(
                    "  {}  {:?}  steps={}  cost={:.3}  {}\n",
                    run.run_id, run.status, run.step_count, run.total_cost_units, run.goal
                ));
            }
            Ok(out)
        }
        OrchestrateAction::Review { run_id, action } => {
            let action = ReviewAction::from_label(&action)?;
            let review = review_run(&run_id, action)?;
            Ok(serde_json::to_string_pretty(&review)?)
        }
    }
}

fn format_plan(plan: &OrchestrationPlan) -> String {
    let mut out = format!(
        "Orchestration plan\nProject: {}\nTask: {}\nGoal: {}\n\n",
        plan.project, plan.task_id, plan.goal
    );
    for (idx, step) in plan.steps.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} [{:?}]\n   executor={} provider={} model={} budget={} write={}\n",
            idx + 1,
            step.title,
            step.kind,
            step.resolved_executor_id(),
            step.resolved_provider_id().unwrap_or("—"),
            step.model.as_deref().unwrap_or("default"),
            step.budget_tokens,
            step.allow_write,
        ));
        if !step.depends_on.is_empty() {
            out.push_str(&format!("   depends_on={}\n", step.depends_on.join(", ")));
        }
        out.push_str(&format!("   {}\n", step.instruction));
    }
    out
}

fn format_run(run: &OrchestrationRun) -> String {
    let mut out = format!(
        "Orchestration run {} — {:?}\nProject: {}  Task: {}\nGoal: {}\n\n",
        run.run_id, run.status, run.project, run.task_id, run.goal,
    );
    for result in &run.results {
        out.push_str(&format!(
            "  {} [{:?}] executor={} provider={} model={} tokens={input}/{output} cost={cost:.3} captured={captured}\n",
            result.task_id,
            result.status,
            result.agent,
            result.provider,
            result.model,
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
        repodesk_core::orchestrator::LoopStatus::EvidenceRecoveryRequired => out.push_str(
            "Stopped: execution finished, but its workflow evidence needs repair. Repair the existing run evidence before Review; do not re-run the agent.\n",
        ),
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
