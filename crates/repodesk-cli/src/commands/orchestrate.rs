use anyhow::{Result, anyhow};

use crate::cli::OrchestrateCommand;
use repodesk_core::api_clients::ProviderSettings;
use repodesk_core::orchestrator::{
    self, OrchestrationPlan, OrchestrationRun, RunOptions, SubAgentTask,
};

pub fn handle_orchestrate_command(command: OrchestrateCommand) -> Result<()> {
    match command {
        OrchestrateCommand::Plan { goal } => {
            let settings = ProviderSettings::from_env();
            let plan = orchestrator::build_plan(goal, &settings)?;
            print!("{}", format_plan(&plan));
        }
        OrchestrateCommand::Run {
            goal,
            dry_run,
            max_cost,
            yes,
        } => {
            let settings = ProviderSettings::from_env();
            let plan = orchestrator::build_plan(goal, &settings)?;

            // The human stays the operator: a real paid run needs confirmation.
            if !dry_run && !yes && plan_has_paid_step(&plan) {
                print!("{}", format_plan(&plan));
                return Err(anyhow!(
                    "This plan includes paid provider steps. Re-run with --yes to execute, \
                     or use --dry-run to preview cost and routing without calling any provider."
                ));
            }

            let opts = RunOptions {
                dry_run,
                max_cost,
                settings,
            };
            let rt = tokio::runtime::Runtime::new()?;
            let run = rt.block_on(orchestrator::run_plan(&plan, &opts))?;
            print!("{}", format_run(&run));
        }
        OrchestrateCommand::Status => match orchestrator::load_latest_run()? {
            Some(run) => print!("{}", format_run(&run)),
            None => println!("No orchestration runs yet for the active task."),
        },
        OrchestrateCommand::Show { run_id } => match orchestrator::load_run(&run_id)? {
            Some(run) => print!("{}", format_run(&run)),
            None => println!("No run '{run_id}' found for the active task."),
        },
    }
    Ok(())
}

const PAID_PROVIDERS: &[&str] = &[
    "chatgpt",
    "codex",
    "openai",
    "gpt",
    "gemini",
    "anthropic",
    "claude",
];

fn plan_has_paid_step(plan: &OrchestrationPlan) -> bool {
    plan.steps
        .iter()
        .any(|step| PAID_PROVIDERS.contains(&step.provider.to_ascii_lowercase().as_str()))
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
        "  • {id}: {title}\n      agent/provider: {agent} → model: {model}\n      thinking: {thinking:?}  write: {write}  depends on: {deps}\n",
        id = step.id,
        title = step.title,
        agent = step.agent,
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
