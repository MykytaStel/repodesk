//! Intended-vs-actual feedback for evidence-backed AI strategies.
//!
//! Strategy selection is not treated as successful merely because an agent call
//! returned. A run becomes a settled strategy outcome only when downstream
//! evidence says enough to classify it: execution failure/partial, human review
//! rejection, verification failure, or a successful bounded commit. Pending
//! review/verification/commit remains pending and never poisons future routing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ai_strategy::AiStrategyProfile;
use super::events::{EngineeringEvent, EngineeringEventKind};

pub const STRATEGY_FEEDBACK_MIN_SETTLED_RUNS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyOutcomeState {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyRunFeedback {
    pub run_id: String,
    pub requested_mode: String,
    pub profile: AiStrategyProfile,
    pub plan_shape: String,
    pub baseline_steps: usize,
    pub planned_steps: usize,
    pub baseline_estimated_tokens: Option<usize>,
    pub planned_estimated_tokens: Option<usize>,
    pub predicted_saved_tokens: usize,
    pub actual_tokens: Option<usize>,
    pub baseline_estimated_cost_units: Option<f64>,
    pub planned_estimated_cost_units: Option<f64>,
    pub actual_cost_units: Option<f64>,
    pub token_estimate_error_ratio: Option<f64>,
    pub execution_status: Option<String>,
    pub review_decision: Option<String>,
    pub verification_success: Option<bool>,
    pub committed: bool,
    pub outcome: StrategyOutcomeState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyProfileFeedback {
    pub profile: AiStrategyProfile,
    pub runs: usize,
    pub settled_runs: usize,
    pub succeeded_runs: usize,
    pub failed_runs: usize,
    pub pending_runs: usize,
    pub success_rate: Option<f64>,
    pub total_actual_tokens: usize,
    pub total_actual_cost_units: f64,
    pub average_actual_tokens: Option<f64>,
    pub average_actual_cost_units: Option<f64>,
    pub average_token_estimate_error_ratio: Option<f64>,
    pub adaptation_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StrategyFeedbackReport {
    pub strategy_runs: usize,
    pub settled_runs: usize,
    pub pending_runs: usize,
    pub profiles: Vec<StrategyProfileFeedback>,
    pub recent_runs: Vec<StrategyRunFeedback>,
}

#[derive(Debug, Clone, Default)]
struct RunFacts {
    requested_mode: Option<String>,
    profile: Option<AiStrategyProfile>,
    plan_shape: Option<String>,
    baseline_steps: usize,
    planned_steps: usize,
    baseline_estimated_tokens: Option<usize>,
    planned_estimated_tokens: Option<usize>,
    predicted_saved_tokens: usize,
    baseline_estimated_cost_units: Option<f64>,
    planned_estimated_cost_units: Option<f64>,
    actual_tokens: Option<usize>,
    actual_cost_units: Option<f64>,
    execution_status: Option<String>,
    has_changeset: bool,
    review_decision: Option<String>,
    verification_success: Option<bool>,
    committed: bool,
}

pub fn derive_strategy_feedback(events: &[EngineeringEvent]) -> StrategyFeedbackReport {
    let mut runs = BTreeMap::<String, RunFacts>::new();

    for event in events {
        let Some(run_id) = event.execution_id.as_ref().map(ToString::to_string) else {
            continue;
        };
        let facts = runs.entry(run_id).or_default();

        match event.kind {
            EngineeringEventKind::AiStrategySelected => {
                facts.requested_mode = attribute_string(event, "requested_mode");
                facts.profile = attribute_string(event, "resolved_profile")
                    .as_deref()
                    .and_then(parse_profile);
                facts.plan_shape = attribute_string(event, "plan_shape");
                facts.baseline_steps = attribute_usize(event, "baseline_steps").unwrap_or(0);
                facts.planned_steps = attribute_usize(event, "planned_steps").unwrap_or(0);
                facts.baseline_estimated_tokens =
                    attribute_usize(event, "baseline_estimated_tokens");
                facts.planned_estimated_tokens = attribute_usize(event, "planned_estimated_tokens");
                facts.predicted_saved_tokens =
                    attribute_usize(event, "estimated_saved_tokens").unwrap_or(0);
                facts.baseline_estimated_cost_units =
                    attribute_f64(event, "baseline_estimated_cost_units");
                facts.planned_estimated_cost_units =
                    attribute_f64(event, "planned_estimated_cost_units");
            }
            EngineeringEventKind::ExecutionFinished => {
                facts.execution_status = attribute_string(event, "status");
                let input = attribute_usize(event, "input_tokens").unwrap_or(0);
                let output = attribute_usize(event, "output_tokens").unwrap_or(0);
                facts.actual_tokens = Some(input.saturating_add(output));
                facts.actual_cost_units = attribute_f64(event, "cost_units");
            }
            EngineeringEventKind::ChangeSetCreated => facts.has_changeset = true,
            EngineeringEventKind::ChangeSetReviewed => {
                facts.review_decision = attribute_string(event, "decision");
            }
            EngineeringEventKind::VerificationFinished => {
                facts.verification_success = attribute_bool(event, "success");
            }
            EngineeringEventKind::CommitCreated => facts.committed = true,
            _ => {}
        }
    }

    // A run is a strategy run only if an explicit strategy-selection event exists.
    let mut recent_runs = runs
        .into_iter()
        .filter_map(|(run_id, facts)| run_feedback(run_id, facts))
        .collect::<Vec<_>>();
    recent_runs.sort_by(|left, right| left.run_id.cmp(&right.run_id));

    // Stable string keys avoid introducing artificial Ord semantics on the
    // serialized strategy enum purely for this internal aggregation detail.
    let mut aggregates = BTreeMap::<String, ProfileAggregate>::new();
    for run in &recent_runs {
        aggregates
            .entry(run.profile.as_label().to_string())
            .or_default()
            .observe(run);
    }

    let profiles = [
        AiStrategyProfile::Lean,
        AiStrategyProfile::Balanced,
        AiStrategyProfile::LocalFirst,
        AiStrategyProfile::Quality,
    ]
    .into_iter()
    .filter_map(|profile| {
        aggregates
            .remove(profile.as_label())
            .map(|value| value.finish(profile))
    })
    .collect::<Vec<_>>();

    let strategy_runs = recent_runs.len();
    let settled_runs = recent_runs
        .iter()
        .filter(|run| run.outcome != StrategyOutcomeState::Pending)
        .count();
    let pending_runs = strategy_runs.saturating_sub(settled_runs);

    // Bound transport size; aggregation above always sees the full ledger.
    if recent_runs.len() > 20 {
        recent_runs = recent_runs.split_off(recent_runs.len() - 20);
    }

    StrategyFeedbackReport {
        strategy_runs,
        settled_runs,
        pending_runs,
        profiles,
        recent_runs,
    }
}

fn parse_profile(value: &str) -> Option<AiStrategyProfile> {
    match value.trim().to_ascii_lowercase().as_str() {
        "lean" => Some(AiStrategyProfile::Lean),
        "balanced" => Some(AiStrategyProfile::Balanced),
        "local_first" | "local-first" => Some(AiStrategyProfile::LocalFirst),
        "quality" => Some(AiStrategyProfile::Quality),
        _ => None,
    }
}

fn run_feedback(run_id: String, facts: RunFacts) -> Option<StrategyRunFeedback> {
    let profile = facts.profile?;
    let outcome = classify_outcome(&facts);
    let token_estimate_error_ratio = match (facts.planned_estimated_tokens, facts.actual_tokens) {
        (Some(planned), Some(actual)) if planned > 0 => {
            Some(actual.abs_diff(planned) as f64 / planned as f64)
        }
        _ => None,
    };

    Some(StrategyRunFeedback {
        run_id,
        requested_mode: facts.requested_mode.unwrap_or_else(|| "unknown".into()),
        profile,
        plan_shape: facts.plan_shape.unwrap_or_else(|| "unknown".into()),
        baseline_steps: facts.baseline_steps,
        planned_steps: facts.planned_steps,
        baseline_estimated_tokens: facts.baseline_estimated_tokens,
        planned_estimated_tokens: facts.planned_estimated_tokens,
        predicted_saved_tokens: facts.predicted_saved_tokens,
        actual_tokens: facts.actual_tokens,
        baseline_estimated_cost_units: facts.baseline_estimated_cost_units,
        planned_estimated_cost_units: facts.planned_estimated_cost_units,
        actual_cost_units: facts.actual_cost_units,
        token_estimate_error_ratio,
        execution_status: facts.execution_status,
        review_decision: facts.review_decision,
        verification_success: facts.verification_success,
        committed: facts.committed,
        outcome,
    })
}

fn classify_outcome(facts: &RunFacts) -> StrategyOutcomeState {
    if matches!(facts.execution_status.as_deref(), Some("failed" | "partial")) {
        return StrategyOutcomeState::Failed;
    }
    if facts.review_decision.as_deref() == Some("rejected") {
        return StrategyOutcomeState::Failed;
    }
    if facts.verification_success == Some(false) {
        return StrategyOutcomeState::Failed;
    }
    if facts.committed {
        return StrategyOutcomeState::Succeeded;
    }
    if !facts.has_changeset && facts.execution_status.as_deref() == Some("completed") {
        return StrategyOutcomeState::Succeeded;
    }
    StrategyOutcomeState::Pending
}

#[derive(Debug, Clone, Default)]
struct ProfileAggregate {
    runs: usize,
    settled: usize,
    succeeded: usize,
    failed: usize,
    pending: usize,
    actual_tokens: usize,
    actual_token_samples: usize,
    actual_cost: f64,
    actual_cost_samples: usize,
    estimate_error_sum: f64,
    estimate_error_samples: usize,
}

impl ProfileAggregate {
    fn observe(&mut self, run: &StrategyRunFeedback) {
        self.runs += 1;
        match run.outcome {
            StrategyOutcomeState::Pending => self.pending += 1,
            StrategyOutcomeState::Succeeded => {
                self.settled += 1;
                self.succeeded += 1;
            }
            StrategyOutcomeState::Failed => {
                self.settled += 1;
                self.failed += 1;
            }
        }
        if let Some(tokens) = run.actual_tokens {
            self.actual_tokens = self.actual_tokens.saturating_add(tokens);
            self.actual_token_samples += 1;
        }
        if let Some(cost) = run.actual_cost_units {
            self.actual_cost += cost;
            self.actual_cost_samples += 1;
        }
        if let Some(error) = run.token_estimate_error_ratio {
            self.estimate_error_sum += error;
            self.estimate_error_samples += 1;
        }
    }

    fn finish(self, profile: AiStrategyProfile) -> StrategyProfileFeedback {
        StrategyProfileFeedback {
            profile,
            runs: self.runs,
            settled_runs: self.settled,
            succeeded_runs: self.succeeded,
            failed_runs: self.failed,
            pending_runs: self.pending,
            success_rate: ratio(self.succeeded, self.settled),
            total_actual_tokens: self.actual_tokens,
            total_actual_cost_units: self.actual_cost,
            average_actual_tokens: average_usize(self.actual_tokens, self.actual_token_samples),
            average_actual_cost_units: average_f64(self.actual_cost, self.actual_cost_samples),
            average_token_estimate_error_ratio: average_f64(
                self.estimate_error_sum,
                self.estimate_error_samples,
            ),
            adaptation_ready: self.settled >= STRATEGY_FEEDBACK_MIN_SETTLED_RUNS,
        }
    }
}

fn attribute_string(event: &EngineeringEvent, key: &str) -> Option<String> {
    event
        .attributes
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn attribute_usize(event: &EngineeringEvent, key: &str) -> Option<usize> {
    event
        .attributes
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn attribute_f64(event: &EngineeringEvent, key: &str) -> Option<f64> {
    event.attributes.get(key).and_then(Value::as_f64)
}

fn attribute_bool(event: &EngineeringEvent, key: &str) -> Option<bool> {
    event.attributes.get(key).and_then(Value::as_bool)
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

fn average_usize(total: usize, count: usize) -> Option<f64> {
    (count > 0).then_some(total as f64 / count as f64)
}

fn average_f64(total: f64, count: usize) -> Option<f64> {
    (count > 0).then_some(total / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::domain::{ExecutionId, WorkItemId};
    use serde_json::json;

    fn event(kind: EngineeringEventKind, run_id: &str) -> EngineeringEvent {
        EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            kind,
        )
        .with_execution(ExecutionId::try_new(run_id).unwrap())
    }

    fn strategy(run_id: &str, profile: &str, planned_tokens: usize) -> EngineeringEvent {
        event(EngineeringEventKind::AiStrategySelected, run_id)
            .with_attribute("requested_mode", json!("auto"))
            .with_attribute("resolved_profile", json!(profile))
            .with_attribute("plan_shape", json!("single_writer"))
            .with_attribute("baseline_steps", json!(3))
            .with_attribute("planned_steps", json!(1))
            .with_attribute("planned_estimated_tokens", json!(planned_tokens))
            .with_attribute("estimated_saved_tokens", json!(5000))
    }

    fn finished(run_id: &str, status: &str, tokens: usize) -> EngineeringEvent {
        event(EngineeringEventKind::ExecutionFinished, run_id)
            .with_attribute("status", json!(status))
            .with_attribute("input_tokens", json!(tokens.saturating_sub(100)))
            .with_attribute("output_tokens", json!(100))
            .with_attribute("cost_units", json!(0.2))
    }

    #[test]
    fn pending_review_does_not_count_as_failure() {
        let report = derive_strategy_feedback(&[
            strategy("run-1", "lean", 1000),
            finished("run-1", "completed", 900),
            event(EngineeringEventKind::ChangeSetCreated, "run-1"),
        ]);
        let run = &report.recent_runs[0];
        assert_eq!(run.outcome, StrategyOutcomeState::Pending);
        assert_eq!(report.settled_runs, 0);
        assert_eq!(report.profiles[0].failed_runs, 0);
    }

    #[test]
    fn rejected_changeset_settles_as_failure() {
        let report = derive_strategy_feedback(&[
            strategy("run-1", "lean", 1000),
            finished("run-1", "completed", 900),
            event(EngineeringEventKind::ChangeSetCreated, "run-1"),
            event(EngineeringEventKind::ChangeSetReviewed, "run-1")
                .with_attribute("decision", json!("rejected")),
        ]);
        assert_eq!(report.recent_runs[0].outcome, StrategyOutcomeState::Failed);
        assert_eq!(report.profiles[0].success_rate, Some(0.0));
    }

    #[test]
    fn committed_run_settles_as_success_and_tracks_estimate_error() {
        let report = derive_strategy_feedback(&[
            strategy("run-1", "balanced", 1000),
            finished("run-1", "completed", 1100),
            event(EngineeringEventKind::ChangeSetCreated, "run-1"),
            event(EngineeringEventKind::ChangeSetReviewed, "run-1")
                .with_attribute("decision", json!("accepted")),
            event(EngineeringEventKind::VerificationFinished, "run-1")
                .with_attribute("success", json!(true)),
            event(EngineeringEventKind::CommitCreated, "run-1"),
        ]);
        let run = &report.recent_runs[0];
        assert_eq!(run.outcome, StrategyOutcomeState::Succeeded);
        assert_eq!(run.token_estimate_error_ratio, Some(0.1));
        assert_eq!(report.profiles[0].success_rate, Some(1.0));
    }

    #[test]
    fn profile_adaptation_requires_three_settled_runs() {
        let mut events = Vec::new();
        for index in 0..3 {
            let run_id = format!("run-{index}");
            events.push(strategy(&run_id, "lean", 1000));
            events.push(finished(&run_id, "completed", 900));
            events.push(event(EngineeringEventKind::CommitCreated, &run_id));
        }
        let report = derive_strategy_feedback(&events);
        assert!(report.profiles[0].adaptation_ready);
        assert_eq!(report.profiles[0].settled_runs, 3);
    }

    #[test]
    fn events_without_strategy_selection_are_ignored() {
        let report = derive_strategy_feedback(&[finished("legacy", "completed", 1000)]);
        assert_eq!(report.strategy_runs, 0);
        assert!(report.profiles.is_empty());
    }
}
