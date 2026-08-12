//! Deterministic observability for one reconstructed run.
//!
//! RunEvidenceSnapshot remains the evidence-first factual projection. This
//! module adds an explainable disposition (what gate stopped progress / what is
//! next) and efficiency metrics without treating either as new workflow truth.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::context_compactness::derive_context_compactness;
use super::events::{EngineeringEvent, EngineeringEventKind};
use super::run_evidence::RunEvidenceSnapshot;
use crate::orchestrator::{RunStatus, SubAgentStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunDispositionState {
    Complete,
    Ready,
    Attention,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunDispositionStage {
    Execution,
    Review,
    Verification,
    Acceptance,
    Commit,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunDisposition {
    pub state: RunDispositionState,
    pub stage: RunDispositionStage,
    pub code: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunContextObservability {
    pub candidate_tokens: Option<usize>,
    pub included_tokens: Option<usize>,
    pub compacted_tokens: Option<usize>,
    pub compactness_ratio: Option<f64>,
    pub repeated_tokens: Option<usize>,
    pub repeated_context_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStrategyObservability {
    pub requested_mode: String,
    pub resolved_profile: String,
    pub plan_shape: String,
    pub plan_fingerprint: String,
    pub baseline_steps: usize,
    pub planned_steps: usize,
    pub estimated_saved_tokens: usize,
    pub context_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunEfficiency {
    pub workers: usize,
    pub successful_workers: usize,
    pub failed_workers: usize,
    pub blocked_workers: usize,
    pub skipped_workers: usize,
    pub handoffs: usize,
    pub unique_providers: usize,
    pub unique_models: usize,
    pub total_tokens: usize,
    pub tokens_per_changed_file: Option<f64>,
    pub cost_per_changed_file: Option<f64>,
    pub input_output_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunObservabilityReport {
    pub run_id: String,
    pub disposition: RunDisposition,
    pub strategy: Option<RunStrategyObservability>,
    pub context: RunContextObservability,
    pub efficiency: RunEfficiency,
}

pub fn derive_run_observability(
    evidence: &RunEvidenceSnapshot,
    events: &[EngineeringEvent],
) -> RunObservabilityReport {
    let total_tokens = evidence
        .total_input_tokens
        .saturating_add(evidence.total_output_tokens);
    let mut providers = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut successful_workers = 0usize;
    let mut failed_workers = 0usize;
    let mut blocked_workers = 0usize;
    let mut skipped_workers = 0usize;

    for worker in &evidence.workers {
        if !worker.provider.trim().is_empty() {
            providers.insert(worker.provider.as_str());
        }
        if !worker.model.trim().is_empty() {
            models.insert(worker.model.as_str());
        }
        match worker.status {
            SubAgentStatus::Ok => successful_workers += 1,
            SubAgentStatus::Failed => failed_workers += 1,
            SubAgentStatus::Blocked => blocked_workers += 1,
            SubAgentStatus::Skipped => skipped_workers += 1,
        }
    }

    let handoffs = events
        .iter()
        .filter(|event| event.kind == EngineeringEventKind::WorkerHandoff)
        .filter(|event| event_belongs_to_run(event, &evidence.run_id))
        .count();

    let strategy = strategy_observability_for_run(events, &evidence.run_id);
    let context = context_observability_for_run(events, evidence);
    let efficiency = RunEfficiency {
        workers: evidence.workers.len(),
        successful_workers,
        failed_workers,
        blocked_workers,
        skipped_workers,
        handoffs,
        unique_providers: providers.len(),
        unique_models: models.len(),
        total_tokens,
        tokens_per_changed_file: ratio(total_tokens as f64, evidence.changed_files.len()),
        cost_per_changed_file: ratio(evidence.total_cost_units, evidence.changed_files.len()),
        input_output_ratio: ratio(
            evidence.total_input_tokens as f64,
            evidence.total_output_tokens,
        ),
    };
    let disposition = derive_disposition(evidence, &efficiency);

    RunObservabilityReport {
        run_id: evidence.run_id.clone(),
        disposition,
        strategy,
        context,
        efficiency,
    }
}

fn strategy_observability_for_run(
    events: &[EngineeringEvent],
    run_id: &str,
) -> Option<RunStrategyObservability> {
    let event = events.iter().rev().find(|event| {
        event.kind == EngineeringEventKind::AiStrategySelected
            && event_belongs_to_run(event, run_id)
    })?;

    Some(RunStrategyObservability {
        requested_mode: attribute_string(event, "requested_mode")?,
        resolved_profile: attribute_string(event, "resolved_profile")?,
        plan_shape: attribute_string(event, "plan_shape")?,
        plan_fingerprint: attribute_string(event, "plan_fingerprint")?,
        baseline_steps: attribute_usize(event, "baseline_steps")?,
        planned_steps: attribute_usize(event, "planned_steps")?,
        estimated_saved_tokens: attribute_usize(event, "estimated_saved_tokens").unwrap_or(0),
        context_fingerprint: attribute_string(event, "context_fingerprint"),
    })
}

fn event_belongs_to_run(event: &EngineeringEvent, run_id: &str) -> bool {
    event
        .execution_id
        .as_ref()
        .is_some_and(|id| id.as_str() == run_id)
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

fn context_observability_for_run(
    events: &[EngineeringEvent],
    evidence: &RunEvidenceSnapshot,
) -> RunContextObservability {
    let Some(started_at) = events
        .iter()
        .find(|event| {
            event.kind == EngineeringEventKind::ExecutionStarted
                && event_belongs_to_run(event, &evidence.run_id)
        })
        .map(|event| event.occurred_at)
    else {
        return RunContextObservability {
            included_tokens: evidence.context.estimated_tokens,
            ..RunContextObservability::default()
        };
    };

    let prior = events
        .iter()
        .filter(|event| event.occurred_at <= started_at)
        .cloned()
        .collect::<Vec<_>>();
    let compactness = derive_context_compactness(&prior);
    let Some(latest) = compactness.latest else {
        return RunContextObservability {
            included_tokens: evidence.context.estimated_tokens,
            ..RunContextObservability::default()
        };
    };

    RunContextObservability {
        candidate_tokens: Some(latest.candidate_tokens),
        included_tokens: Some(latest.included_tokens),
        compacted_tokens: Some(latest.compacted_tokens),
        compactness_ratio: latest.compactness_ratio,
        repeated_tokens: latest.repeated_tokens,
        repeated_context_ratio: latest.repeated_context_ratio,
    }
}

fn derive_disposition(
    evidence: &RunEvidenceSnapshot,
    efficiency: &RunEfficiency,
) -> RunDisposition {
    if evidence.dry_run || evidence.status == RunStatus::DryRun {
        return disposition(
            RunDispositionState::Attention,
            RunDispositionStage::Execution,
            "dry_run",
            "Dry run only",
            "The orchestration plan was evaluated without producing an authoritative execution result.",
        );
    }

    if evidence.status == RunStatus::Failed {
        let detail = if efficiency.failed_workers > 0 {
            format!(
                "{} worker step(s) failed; inspect the first failed worker before retrying.",
                efficiency.failed_workers
            )
        } else if efficiency.blocked_workers > 0 {
            format!(
                "{} worker step(s) were blocked by a guard, safety rule, budget, or cost ceiling.",
                efficiency.blocked_workers
            )
        } else {
            "The execution finished with a failed status and no stronger downstream evidence exists.".into()
        };
        return disposition(
            RunDispositionState::Blocked,
            RunDispositionStage::Execution,
            "execution_failed",
            "Execution failed",
            &detail,
        );
    }

    if evidence.status == RunStatus::Partial {
        return disposition(
            RunDispositionState::Attention,
            RunDispositionStage::Execution,
            "execution_partial",
            "Execution completed only partially",
            &format!(
                "{} successful, {} failed, {} blocked and {} skipped worker step(s) were recorded.",
                efficiency.successful_workers,
                efficiency.failed_workers,
                efficiency.blocked_workers,
                efficiency.skipped_workers
            ),
        );
    }

    if evidence.changed_files.is_empty() {
        return disposition(
            RunDispositionState::Complete,
            RunDispositionStage::Complete,
            "completed_without_changes",
            "Execution completed without a ChangeSet",
            "No repository files were attributed to this run, so there is no review/verification/commit chain to finish.",
        );
    }

    match evidence.review.state.as_str() {
        "rejected" => {
            return disposition(
                RunDispositionState::Blocked,
                RunDispositionStage::Review,
                "review_rejected",
                "Human review rejected the ChangeSet",
                "The exact run ChangeSet was rejected and must not advance to verification or commit.",
            );
        }
        "accepted" => {}
        _ => {
            return disposition(
                RunDispositionState::Ready,
                RunDispositionStage::Review,
                "awaiting_review",
                "Ready for human review",
                "Execution produced changes, but the exact ChangeSet has not been accepted yet.",
            );
        }
    }

    match evidence.verification.state.as_str() {
        "failed" => {
            return disposition(
                RunDispositionState::Blocked,
                RunDispositionStage::Verification,
                "verification_failed",
                "Verification failed",
                "The reviewed tree has failing verification evidence. Fix the failing command before another agent retry.",
            );
        }
        "stale" => {
            return disposition(
                RunDispositionState::Blocked,
                RunDispositionStage::Verification,
                "verification_stale",
                "Verification evidence is stale",
                "The reviewed tree changed after verification; RepoDesk will not reuse that proof.",
            );
        }
        "passed" => {}
        "running" => {
            return disposition(
                RunDispositionState::Attention,
                RunDispositionStage::Verification,
                "verification_running",
                "Verification is still running",
                "Wait for command-level verification evidence before evaluating acceptance or commit readiness.",
            );
        }
        _ => {
            return disposition(
                RunDispositionState::Ready,
                RunDispositionStage::Verification,
                "awaiting_verification",
                "Ready for verification",
                "The ChangeSet was accepted, but no current verification receipt proves the reviewed tree.",
            );
        }
    }

    if evidence.acceptance.configured && evidence.acceptance.failed > 0 {
        return disposition(
            RunDispositionState::Blocked,
            RunDispositionStage::Acceptance,
            "acceptance_failed",
            "Acceptance criteria failed",
            &format!(
                "{} acceptance criterion/criteria are linked to failing verification evidence.",
                evidence.acceptance.failed
            ),
        );
    }

    if evidence.acceptance.configured && evidence.acceptance.unproven > 0 {
        return disposition(
            RunDispositionState::Attention,
            RunDispositionStage::Acceptance,
            "acceptance_unproven",
            "Acceptance evidence is incomplete",
            &format!(
                "{} acceptance criterion/criteria still need explicit proof from the current verification receipt.",
                evidence.acceptance.unproven
            ),
        );
    }

    if evidence.commit.committed {
        return disposition(
            RunDispositionState::Complete,
            RunDispositionStage::Complete,
            "committed",
            "Run is complete",
            "Execution, review, verification and commit evidence are all present for this run.",
        );
    }

    disposition(
        RunDispositionState::Ready,
        RunDispositionStage::Commit,
        "ready_to_commit",
        "Verified ChangeSet is ready to finish",
        "Review and verification evidence are current; the remaining workflow action is the bounded commit.",
    )
}

fn disposition(
    state: RunDispositionState,
    stage: RunDispositionStage,
    code: &str,
    title: &str,
    detail: &str,
) -> RunDisposition {
    RunDisposition {
        state,
        stage,
        code: code.into(),
        title: title.into(),
        detail: detail.into(),
    }
}

fn ratio(numerator: f64, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::acceptance_evidence::AcceptanceEvidenceReport;
    use crate::engineering::domain::{ExecutionId, WorkItemId};
    use crate::engineering::run_evidence::{
        RunCommitEvidence, RunContextEvidence, RunReviewEvidence, RunVerificationEvidence,
        RunWorkerEvidence,
    };
    use serde_json::json;

    fn base_evidence() -> RunEvidenceSnapshot {
        RunEvidenceSnapshot {
            run_id: "run-1".into(),
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            goal: "Ship observability".into(),
            status: RunStatus::Completed,
            dry_run: false,
            started_at: "2026-08-12T10:00:00Z".into(),
            finished_at: "2026-08-12T10:01:00Z".into(),
            total_input_tokens: 2_000,
            total_output_tokens: 500,
            total_cost_units: 0.25,
            workers: vec![RunWorkerEvidence {
                step_id: "implement".into(),
                agent: "codex".into(),
                provider: "openai".into(),
                model: "model".into(),
                status: SubAgentStatus::Ok,
                changed_files: vec!["src/lib.rs".into()],
                input_tokens: 2_000,
                output_tokens: 500,
                cost_units: 0.25,
            }],
            changed_files: vec!["src/lib.rs".into()],
            context: RunContextEvidence {
                estimated_tokens: Some(1_200),
                evidence: Vec::new(),
                source: "engineering_event".into(),
            },
            review: RunReviewEvidence {
                state: "not_reviewed".into(),
                reviewed_paths: Vec::new(),
                source: "unavailable".into(),
            },
            verification: RunVerificationEvidence {
                state: "not_run".into(),
                verification_id: None,
                commands: Vec::new(),
                evidence: Vec::new(),
                verified_at: None,
                source: "unavailable".into(),
            },
            commit: RunCommitEvidence {
                committed: false,
                commit_sha: None,
                committed_paths: Vec::new(),
                source: "unavailable".into(),
            },
            acceptance: AcceptanceEvidenceReport {
                configured: false,
                work_item_id: "task-1".into(),
                current_run_id: Some("run-1".into()),
                criteria: Vec::new(),
                proven: 0,
                failed: 0,
                unproven: 0,
            },
        }
    }

    #[test]
    fn completed_changes_stop_at_human_review_until_accepted() {
        let report = derive_run_observability(&base_evidence(), &[]);
        assert_eq!(report.disposition.code, "awaiting_review");
        assert_eq!(report.disposition.state, RunDispositionState::Ready);
        assert_eq!(report.efficiency.tokens_per_changed_file, Some(2_500.0));
        assert!(report.strategy.is_none());
    }

    #[test]
    fn accepted_but_unverified_changes_point_to_verification() {
        let mut evidence = base_evidence();
        evidence.review.state = "accepted".into();
        let report = derive_run_observability(&evidence, &[]);
        assert_eq!(report.disposition.code, "awaiting_verification");
        assert_eq!(report.disposition.stage, RunDispositionStage::Verification);
    }

    #[test]
    fn failed_worker_is_reported_as_execution_blocker() {
        let mut evidence = base_evidence();
        evidence.status = RunStatus::Failed;
        evidence.workers[0].status = SubAgentStatus::Failed;
        let report = derive_run_observability(&evidence, &[]);
        assert_eq!(report.disposition.code, "execution_failed");
        assert_eq!(report.efficiency.failed_workers, 1);
        assert_eq!(report.disposition.state, RunDispositionState::Blocked);
    }

    #[test]
    fn selected_strategy_is_bound_to_the_same_execution() {
        let strategy_event = EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            EngineeringEventKind::AiStrategySelected,
        )
        .with_execution(ExecutionId::try_new("run-1").unwrap())
        .with_attribute("requested_mode", json!("auto"))
        .with_attribute("resolved_profile", json!("lean"))
        .with_attribute("plan_shape", json!("single_writer"))
        .with_attribute("plan_fingerprint", json!("fingerprint"))
        .with_attribute("baseline_steps", json!(3))
        .with_attribute("planned_steps", json!(1))
        .with_attribute("estimated_saved_tokens", json!(5000));

        let report = derive_run_observability(&base_evidence(), &[strategy_event]);
        let strategy = report.strategy.expect("strategy evidence");
        assert_eq!(strategy.requested_mode, "auto");
        assert_eq!(strategy.resolved_profile, "lean");
        assert_eq!(strategy.baseline_steps, 3);
        assert_eq!(strategy.planned_steps, 1);
        assert_eq!(strategy.estimated_saved_tokens, 5_000);
    }
}
