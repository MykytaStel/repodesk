//! Explainable AI-usage efficiency derived from existing Engineering Events.
//!
//! This module deliberately avoids a synthetic productivity or quality score.
//! It exposes observable ratios and deterministic signals so the user can see
//! exactly why RepoDesk recommends reducing context churn, agent fan-out, or
//! repeated execution attempts.

use serde::{Deserialize, Serialize};

use super::context_compactness::derive_context_compactness;
use super::events::EngineeringEvent;
use super::intelligence::EngineeringIntelligence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiUsageSignalSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiUsageSignalCode {
    RepeatedContext,
    AgentFanout,
    ExecutionChurn,
    PromptHeavy,
    ChangeRejection,
    VerificationInstability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiUsageSignal {
    pub code: AiUsageSignalCode,
    pub severity: AiUsageSignalSeverity,
    pub title: String,
    pub detail: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AiContextEfficiency {
    pub builds: usize,
    pub measured_builds: usize,
    pub total_candidate_tokens: usize,
    pub total_included_tokens: usize,
    pub total_saved_tokens: usize,
    pub latest_candidate_tokens: Option<usize>,
    pub latest_included_tokens: Option<usize>,
    pub latest_compacted_tokens: Option<usize>,
    pub latest_compactness_ratio: Option<f64>,
    pub latest_repeated_tokens: Option<usize>,
    pub latest_repeated_context_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AiOrchestrationEfficiency {
    pub managed_executions: usize,
    pub manual_executions: usize,
    pub unique_workers: usize,
    pub unique_coding_agents: usize,
    pub handoffs: usize,
    pub handoffs_per_managed_execution: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AiOutcomeEfficiency {
    pub completed_executions: usize,
    pub partial_executions: usize,
    pub failed_executions: usize,
    pub accepted_files: usize,
    pub total_tokens: usize,
    pub tokens_per_finished_execution: Option<f64>,
    pub tokens_per_accepted_file: Option<f64>,
    pub cost_per_completed_execution: Option<f64>,
    pub input_output_ratio: Option<f64>,
    pub execution_completion_rate: Option<f64>,
    pub changeset_acceptance_rate: Option<f64>,
    pub verification_pass_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AiUsageReport {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub cost_units: f64,
    pub context: AiContextEfficiency,
    pub orchestration: AiOrchestrationEfficiency,
    pub outcomes: AiOutcomeEfficiency,
    pub signals: Vec<AiUsageSignal>,
}

pub fn derive_ai_usage_report(
    events: &[EngineeringEvent],
    intelligence: &EngineeringIntelligence,
) -> AiUsageReport {
    let compactness = derive_context_compactness(events);
    let latest = compactness.latest.as_ref();
    let total_tokens = intelligence
        .ai_usage
        .input_tokens
        .saturating_add(intelligence.ai_usage.output_tokens);
    let finished_outcomes = intelligence
        .execution
        .completed
        .saturating_add(intelligence.execution.partial)
        .saturating_add(intelligence.execution.failed);

    let context = AiContextEfficiency {
        builds: compactness.builds,
        measured_builds: compactness.measured_builds,
        total_candidate_tokens: compactness.total_candidate_tokens,
        total_included_tokens: compactness.total_included_tokens,
        total_saved_tokens: compactness.total_compacted_tokens,
        latest_candidate_tokens: latest.map(|value| value.candidate_tokens),
        latest_included_tokens: latest.map(|value| value.included_tokens),
        latest_compacted_tokens: latest.map(|value| value.compacted_tokens),
        latest_compactness_ratio: latest.and_then(|value| value.compactness_ratio),
        latest_repeated_tokens: latest.and_then(|value| value.repeated_tokens),
        latest_repeated_context_ratio: latest.and_then(|value| value.repeated_context_ratio),
    };
    let orchestration = AiOrchestrationEfficiency {
        managed_executions: intelligence.execution.managed,
        manual_executions: intelligence.execution.manual,
        unique_workers: intelligence.execution.unique_workers,
        unique_coding_agents: intelligence.execution.unique_coding_agents,
        handoffs: intelligence.execution.handoffs,
        handoffs_per_managed_execution: ratio(
            intelligence.execution.handoffs,
            intelligence.execution.managed,
        ),
    };
    let outcomes = AiOutcomeEfficiency {
        completed_executions: intelligence.execution.completed,
        partial_executions: intelligence.execution.partial,
        failed_executions: intelligence.execution.failed,
        accepted_files: intelligence.changes.accepted_files,
        total_tokens,
        tokens_per_finished_execution: ratio(total_tokens, finished_outcomes),
        tokens_per_accepted_file: ratio(total_tokens, intelligence.changes.accepted_files),
        cost_per_completed_execution: ratio_f64(
            intelligence.ai_usage.cost_units,
            intelligence.execution.completed,
        ),
        input_output_ratio: ratio(
            intelligence.ai_usage.input_tokens,
            intelligence.ai_usage.output_tokens,
        ),
        execution_completion_rate: intelligence.rates.execution_completion_rate,
        changeset_acceptance_rate: intelligence.rates.changeset_acceptance_rate,
        verification_pass_rate: intelligence.rates.verification_pass_rate,
    };

    let mut report = AiUsageReport {
        input_tokens: intelligence.ai_usage.input_tokens,
        output_tokens: intelligence.ai_usage.output_tokens,
        total_tokens,
        cost_units: intelligence.ai_usage.cost_units,
        context,
        orchestration,
        outcomes,
        signals: Vec::new(),
    };
    report.signals = derive_signals(&report, intelligence);
    report
}

fn derive_signals(
    report: &AiUsageReport,
    intelligence: &EngineeringIntelligence,
) -> Vec<AiUsageSignal> {
    let mut signals = Vec::new();

    if let (Some(ratio), Some(tokens)) = (
        report.context.latest_repeated_context_ratio,
        report.context.latest_repeated_tokens,
    ) && ratio >= 0.65
        && tokens >= 1_000
        && report.context.builds >= 2
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::RepeatedContext,
            severity: AiUsageSignalSeverity::Warning,
            title: "Most of the latest context was repeated".into(),
            detail: format!(
                "{:.0}% of the latest included context ({} tokens) matched the previous build.",
                ratio * 100.0,
                tokens
            ),
            recommendation: "Reuse a prepared packet when the task boundary has not changed, or remove stable material from the per-run context.".into(),
        });
    }

    if let Some(handoffs_per_run) = report.orchestration.handoffs_per_managed_execution
        && report.orchestration.handoffs >= 2
        && handoffs_per_run >= 1.5
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::AgentFanout,
            severity: AiUsageSignalSeverity::Warning,
            title: "Agent hand-off fan-out is high".into(),
            detail: format!(
                "{} hand-offs across {} managed execution(s), or {:.1} hand-offs per managed run.",
                report.orchestration.handoffs,
                report.orchestration.managed_executions,
                handoffs_per_run
            ),
            recommendation: "Collapse adjacent agent steps that consume the same context and have no independent verification boundary.".into(),
        });
    } else if report.orchestration.unique_coding_agents >= 3
        && report.orchestration.managed_executions > 0
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::AgentFanout,
            severity: AiUsageSignalSeverity::Info,
            title: "Several coding agents participated in one Work Item".into(),
            detail: format!(
                "{} distinct coding agents were observed across {} managed execution(s).",
                report.orchestration.unique_coding_agents,
                report.orchestration.managed_executions
            ),
            recommendation: "Keep multiple agents only where specialization or independent review justifies the extra context hand-offs.".into(),
        });
    }

    let finished = report
        .outcomes
        .completed_executions
        .saturating_add(report.outcomes.partial_executions)
        .saturating_add(report.outcomes.failed_executions);
    if finished >= 2
        && report
            .outcomes
            .execution_completion_rate
            .is_some_and(|rate| rate < 0.60)
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::ExecutionChurn,
            severity: AiUsageSignalSeverity::Warning,
            title: "Execution retries are producing little completion".into(),
            detail: format!(
                "{} of {} finished execution outcome(s) completed successfully.",
                report.outcomes.completed_executions, finished
            ),
            recommendation: "Tighten the Work Item scope or inspect the first failed/partial run before delegating another agent attempt.".into(),
        });
    }

    if report.input_tokens >= 5_000
        && report
            .outcomes
            .input_output_ratio
            .is_some_and(|ratio| ratio >= 12.0)
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::PromptHeavy,
            severity: AiUsageSignalSeverity::Info,
            title: "AI usage is strongly input-heavy".into(),
            detail: format!(
                "{} input tokens produced {} output tokens ({:.1}:1 input/output).",
                report.input_tokens,
                report.output_tokens,
                report.outcomes.input_output_ratio.unwrap_or_default()
            ),
            recommendation: "Inspect Context Evidence for large stable sources and keep only task-relevant repository material in the execution packet.".into(),
        });
    }

    let reviewed = intelligence
        .changes
        .accepted_changesets
        .saturating_add(intelligence.changes.rejected_changesets);
    if reviewed >= 2
        && report
            .outcomes
            .changeset_acceptance_rate
            .is_some_and(|rate| rate < 0.50)
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::ChangeRejection,
            severity: AiUsageSignalSeverity::Warning,
            title: "Agent ChangeSets are frequently rejected".into(),
            detail: format!(
                "{} of {} reviewed ChangeSet(s) were accepted.",
                intelligence.changes.accepted_changesets, reviewed
            ),
            recommendation: "Reduce write scope, improve acceptance criteria, or route the task through a more suitable coding agent before retrying.".into(),
        });
    }

    if intelligence.verification.finished >= 2
        && report
            .outcomes
            .verification_pass_rate
            .is_some_and(|rate| rate < 0.70)
    {
        signals.push(AiUsageSignal {
            code: AiUsageSignalCode::VerificationInstability,
            severity: AiUsageSignalSeverity::Warning,
            title: "Verification is repeatedly failing".into(),
            detail: format!(
                "{} of {} finished verification attempt(s) passed.",
                intelligence.verification.passed, intelligence.verification.finished
            ),
            recommendation: "Inspect the first failing command and its changed files before spending more tokens on another execution.".into(),
        });
    }

    signals
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

fn ratio_f64(numerator: f64, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator / denominator as f64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::engineering::context_compactness::ContextComponentTelemetry;
    use crate::engineering::domain::{ExecutionId, WorkItemId};
    use crate::engineering::events::{EngineeringEvent, EngineeringEventKind};
    use crate::engineering::intelligence::derive_engineering_intelligence;

    fn event(kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new("repodesk", WorkItemId::try_new("task-1").unwrap(), kind)
    }

    fn context_event(tokens: usize, fingerprint: &str) -> EngineeringEvent {
        event(EngineeringEventKind::ContextBuilt)
            .with_attribute("estimated_tokens", json!(tokens))
            .with_attribute("candidate_tokens", json!(tokens + 800))
            .with_attribute(
                "components",
                json!([ContextComponentTelemetry {
                    name: "task".into(),
                    candidate_tokens: tokens,
                    included_tokens: tokens,
                    trimmed: false,
                    fingerprint: fingerprint.into(),
                }]),
            )
    }

    #[test]
    fn repeated_context_and_handoffs_are_explainable_signals() {
        let execution = ExecutionId::try_new("run-1").unwrap();
        let events = vec![
            context_event(2_000, "same"),
            context_event(2_000, "same"),
            event(EngineeringEventKind::ExecutionStarted)
                .with_execution(execution.clone())
                .with_attribute("execution_mode", json!("managed")),
            event(EngineeringEventKind::ExecutionFinished)
                .with_execution(execution.clone())
                .with_attribute("execution_mode", json!("managed"))
                .with_attribute("status", json!("completed"))
                .with_attribute("input_tokens", json!(6_000))
                .with_attribute("output_tokens", json!(400)),
            event(EngineeringEventKind::WorkerHandoff)
                .with_execution(execution.clone())
                .with_attribute("from_worker", json!("one"))
                .with_attribute("to_worker", json!("two"))
                .with_attribute("source_step", json!("a"))
                .with_attribute("target_step", json!("b")),
            event(EngineeringEventKind::WorkerHandoff)
                .with_execution(execution)
                .with_attribute("from_worker", json!("two"))
                .with_attribute("to_worker", json!("three"))
                .with_attribute("source_step", json!("b"))
                .with_attribute("target_step", json!("c")),
        ];
        let intelligence = derive_engineering_intelligence(&events);
        let report = derive_ai_usage_report(&events, &intelligence);

        assert_eq!(report.context.latest_repeated_tokens, Some(2_000));
        assert_eq!(report.orchestration.handoffs, 2);
        assert!(report.signals.iter().any(|signal| signal.code == AiUsageSignalCode::RepeatedContext));
        assert!(report.signals.iter().any(|signal| signal.code == AiUsageSignalCode::AgentFanout));
        assert!(report.signals.iter().any(|signal| signal.code == AiUsageSignalCode::PromptHeavy));
    }

    #[test]
    fn empty_history_has_no_fake_efficiency_values() {
        let intelligence = EngineeringIntelligence::default();
        let report = derive_ai_usage_report(&[], &intelligence);

        assert_eq!(report.total_tokens, 0);
        assert_eq!(report.outcomes.tokens_per_finished_execution, None);
        assert_eq!(report.outcomes.cost_per_completed_execution, None);
        assert!(report.signals.is_empty());
    }
}