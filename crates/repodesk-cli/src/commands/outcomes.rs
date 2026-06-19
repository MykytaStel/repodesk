use anyhow::{Result, anyhow};

use crate::cli::OutcomesCommand;
use repodesk_core::outcomes::{self, OutcomeRecord, ProviderStat, Verdict};
use repodesk_core::projects::read_active_project;

pub fn handle_outcomes_command(command: OutcomesCommand) -> Result<()> {
    match command {
        OutcomesCommand::List { limit } => {
            let rows = outcomes::list_outcomes(limit)?;
            if rows.is_empty() {
                println!("No recorded outcomes yet. Run the orchestrator first.");
            } else {
                print!("{}", format_rows(&rows));
            }
        }
        OutcomesCommand::Stats => {
            let project = read_active_project()?;
            let stats = outcomes::outcome_stats(&project)?;
            if stats.is_empty() {
                println!("No outcome stats yet for project '{project}'.");
            } else {
                print!("{}", format_stats(&project, &stats));
            }
        }
        OutcomesCommand::Confirm { id, verdict } => {
            let parsed = parse_verdict(&verdict)?;
            outcomes::confirm_outcome(id, parsed)?;
            println!("Outcome {id} confirmed as '{}'.", parsed.as_label());
        }
    }
    Ok(())
}

fn parse_verdict(value: &str) -> Result<Verdict> {
    match value.trim().to_ascii_lowercase().as_str() {
        "good" => Ok(Verdict::Good),
        "bad" => Ok(Verdict::Bad),
        "neutral" => Ok(Verdict::Neutral),
        other => Err(anyhow!(
            "unknown verdict '{other}' — use good, bad, or neutral"
        )),
    }
}

fn format_rows(rows: &[OutcomeRecord]) -> String {
    let mut out = String::from("Recent step outcomes (newest first):\n\n");
    for row in rows {
        let source = if row.confirmed {
            "human"
        } else {
            row.verdict_source.as_str()
        };
        out.push_str(&format!(
            "  #{id}  [{verdict}/{source}]  {kind} → {provider}  ({status})\n      run {run}  step {step}  cost {cost:.3}  tokens {tin} in / {tout} out\n",
            id = row.id,
            verdict = row.verdict.as_label(),
            kind = row.task_kind,
            provider = row.provider,
            status = row.status,
            run = row.run_id,
            step = row.step_id,
            cost = row.cost_units,
            tin = row.input_tokens,
            tout = row.output_tokens,
        ));
    }
    out.push_str("\nConfirm a verdict with `repodesk outcomes confirm <id> <good|bad>`.\n");
    out
}

fn format_stats(project: &str, stats: &[ProviderStat]) -> String {
    let mut out = format!("Outcome stats for project '{project}' (task kind → provider):\n\n");
    for stat in stats {
        let rate = match stat.success_rate {
            Some(rate) => format!("{:.0}%", rate * 100.0),
            None => "—".to_string(),
        };
        out.push_str(&format!(
            "  {kind} → {provider}: success {rate} ({good} good / {bad} bad, {neutral} neutral)  avg cost {cost:.3}\n",
            kind = stat.task_kind,
            provider = stat.provider,
            good = stat.good,
            bad = stat.bad,
            neutral = stat.neutral,
            cost = stat.avg_cost_units,
        ));
    }
    out
}
