//! SQLite-backed storage for the orchestrator outcome ledger (`run_outcomes`,
//! migration v3). Writes one row per executed step, lets a human confirm or flip
//! a verdict, and aggregates rows into per-(task_kind, provider) stats.

use std::collections::BTreeMap;

use chrono::Utc;
use rusqlite::params;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::orchestrator::types::{OrchestrationPlan, OrchestrationRun};
use crate::persistence::db::init_db;
use crate::routing::types::TaskKind;

use super::model::{OutcomeRecord, ProviderStat, Verdict};

fn db_err(context: &str, e: impl std::fmt::Display) -> RepoDeskError {
    RepoDeskError::Database(format!("{context}: {e}"))
}

/// Canonical snake_case label for a `TaskKind` (matches its serde rename).
fn task_kind_label(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Compress => "compress",
        TaskKind::Summarize => "summarize",
        TaskKind::Plan => "plan",
        TaskKind::Review => "review",
        TaskKind::Patch => "patch",
        TaskKind::Debug => "debug",
        TaskKind::Checks => "checks",
        TaskKind::Manual => "manual",
    }
}

/// Canonical snake_case label for a step status (matches its serde rename).
fn status_label(status: crate::orchestrator::types::SubAgentStatus) -> &'static str {
    use crate::orchestrator::types::SubAgentStatus::*;
    match status {
        Ok => "ok",
        Skipped => "skipped",
        Blocked => "blocked",
        Failed => "failed",
    }
}

/// Record every executed step of a finished run as outcome rows. The plan
/// supplies each step's `TaskKind` (the run only carries step ids). Dry runs
/// carry no learning signal and must not be recorded — callers gate on that.
pub fn record_run(plan: &OrchestrationPlan, run: &OrchestrationRun) -> RepoDeskResult<usize> {
    if run.dry_run {
        return Ok(0);
    }

    let kind_of: BTreeMap<&str, TaskKind> = plan
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step.kind))
        .collect();

    let mut conn = init_db()?;
    let now = Utc::now().to_rfc3339();
    let tx = conn
        .transaction()
        .map_err(|e| db_err("Failed to open outcome transaction", e))?;

    let mut written = 0usize;
    for result in &run.results {
        let kind = kind_of
            .get(result.task_id.as_str())
            .copied()
            .unwrap_or(TaskKind::Manual);
        let verdict = Verdict::auto_from_status(result.status);
        tx.execute(
            "INSERT INTO run_outcomes (
                created_at, project, task_id, run_id, step_id, task_kind, provider,
                model, status, input_tokens, output_tokens, cost_units,
                verdict, verdict_source, confirmed, notes
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'auto', 0, ?14
            )",
            params![
                now,
                run.project,
                run.task_id,
                run.run_id,
                result.task_id,
                task_kind_label(kind),
                result.provider,
                result.model,
                status_label(result.status),
                result.input_tokens as i64,
                result.output_tokens as i64,
                result.cost_units,
                verdict.as_label(),
                result.notes.join("; "),
            ],
        )
        .map_err(|e| db_err("Failed to insert outcome row", e))?;
        written += 1;
    }

    tx.commit()
        .map_err(|e| db_err("Failed to commit outcomes", e))?;
    Ok(written)
}

/// Confirm (or override) a verdict for one outcome row. Flips `verdict_source`
/// to `human` and marks it confirmed — the human-in-the-loop knob that the
/// adaptive router weights above provisional `auto` rows.
pub fn confirm_outcome(id: i64, verdict: Verdict) -> RepoDeskResult<()> {
    let conn = init_db()?;
    let changed = conn
        .execute(
            "UPDATE run_outcomes
                SET verdict = ?1, verdict_source = 'human', confirmed = 1
              WHERE id = ?2",
            params![verdict.as_label(), id],
        )
        .map_err(|e| db_err("Failed to confirm outcome", e))?;
    if changed == 0 {
        return Err(RepoDeskError::Database(format!(
            "No outcome row with id {id}"
        )));
    }
    Ok(())
}

/// Most recent outcome rows, newest first.
pub fn list_outcomes(limit: usize) -> RepoDeskResult<Vec<OutcomeRecord>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, project, task_id, run_id, step_id, task_kind,
                    provider, model, status, input_tokens, output_tokens, cost_units,
                    verdict, verdict_source, confirmed, notes
               FROM run_outcomes
              ORDER BY id DESC
              LIMIT ?1",
        )
        .map_err(|e| db_err("Failed to prepare outcome query", e))?;
    let rows = stmt
        .query_map([limit as i64], row_to_record)
        .map_err(|e| db_err("Failed to read outcomes", e))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| db_err("Failed to decode outcome row", e))?);
    }
    Ok(out)
}

/// Aggregate outcomes for the active project into per-(task_kind, provider)
/// stats, the signal the adaptive router reads. Confirmed `human` rows and
/// provisional `auto` rows are counted equally for now; weighting them is N8-B's
/// job. Sorted by task_kind then provider for stable output.
pub fn outcome_stats(project: &str) -> RepoDeskResult<Vec<ProviderStat>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT task_kind, provider,
                    SUM(verdict = 'good')    AS good,
                    SUM(verdict = 'bad')     AS bad,
                    SUM(verdict = 'neutral') AS neutral,
                    SUM(cost_units)          AS total_cost,
                    COUNT(*)                 AS total
               FROM run_outcomes
              WHERE project = ?1
              GROUP BY task_kind, provider
              ORDER BY task_kind, provider",
        )
        .map_err(|e| db_err("Failed to prepare stats query", e))?;

    let rows = stmt
        .query_map([project], |row| {
            let task_kind: String = row.get(0)?;
            let provider: String = row.get(1)?;
            let good: i64 = row.get(2)?;
            let bad: i64 = row.get(3)?;
            let neutral: i64 = row.get(4)?;
            let total_cost: f64 = row.get(5)?;
            let total: i64 = row.get(6)?;
            let scored = good + bad;
            let success_rate = if scored > 0 {
                Some(good as f64 / scored as f64)
            } else {
                None
            };
            let avg_cost = if total > 0 {
                total_cost / total as f64
            } else {
                0.0
            };
            Ok(ProviderStat {
                task_kind,
                provider,
                scored_runs: scored as usize,
                good: good as usize,
                bad: bad as usize,
                neutral: neutral as usize,
                success_rate,
                avg_cost_units: avg_cost,
            })
        })
        .map_err(|e| db_err("Failed to aggregate outcomes", e))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| db_err("Failed to decode stat row", e))?);
    }
    Ok(out)
}

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<OutcomeRecord> {
    let verdict: String = row.get(13)?;
    let confirmed: i64 = row.get(15)?;
    Ok(OutcomeRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        project: row.get(2)?,
        task_id: row.get(3)?,
        run_id: row.get(4)?,
        step_id: row.get(5)?,
        task_kind: row.get(6)?,
        provider: row.get(7)?,
        model: row.get(8)?,
        status: row.get(9)?,
        input_tokens: row.get::<_, i64>(10)? as usize,
        output_tokens: row.get::<_, i64>(11)? as usize,
        cost_units: row.get(12)?,
        verdict: Verdict::from_label(&verdict),
        verdict_source: row.get(14)?,
        confirmed: confirmed != 0,
        notes: row.get(16)?,
    })
}
