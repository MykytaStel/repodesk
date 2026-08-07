//! Deterministic Engineering Intelligence derived from the append-only event ledger.
//!
//! This module reports facts, totals, and simple explainable rates. It does not
//! produce an opaque productivity, quality, or AI score. The event ledger remains
//! the source of truth and this read model can always be rebuilt by replaying it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engineering::domain::{EvidenceKind, WorkItemId, WorkerKind, WorkerRef};
use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, read_events};
use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EngineeringIntelligence {
    pub project: Option<String>,
    pub work_item_id: Option<WorkItemId>,
    pub event_count: usize,
    pub execution: ExecutionIntelligence,
    pub ai_usage: AiUsageIntelligence,
    pub context: ContextIntelligence,
    pub changes: ChangeIntelligence,
    pub verification: VerificationIntelligence,
    pub completion: CompletionIntelligence,
    pub rates: IntelligenceRates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExecutionIntelligence {
    pub attempts: usize,
    pub finished: usize,
    pub completed: usize,
    pub partial: usize,
    pub failed: usize,
    pub dry_runs: usize,
    pub unfinished: usize,
    pub managed: usize,
    pub manual: usize,
    pub unknown_mode: usize,
    pub unique_workers: usize,
    pub unique_coding_agents: usize,
    pub handoffs: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AiUsageIntelligence {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_units: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContextIntelligence {
    pub builds: usize,
    pub total_estimated_tokens: usize,
    pub latest_estimated_tokens: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChangeIntelligence {
    pub proposed_changesets: usize,
    pub proposed_files: usize,
    pub reviewed_changesets: usize,
    pub accepted_changesets: usize,
    pub rejected_changesets: usize,
    pub pending_review_changesets: usize,
    pub accepted_files: usize,
    pub rejected_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VerificationIntelligence {
    pub attempts: usize,
    pub finished: usize,
    pub passed: usize,
    pub failed: usize,
    pub unfinished: usize,
    pub commands_run: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompletionIntelligence {
    pub committed: bool,
    pub commits: usize,
    pub committed_files: usize,
    pub latest_commit_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct IntelligenceRates {
    /// Completed executions / (completed + partial + failed).
    /// Dry runs and unfinished executions are excluded.
    pub execution_completion_rate: Option<f64>,
    /// Accepted changesets / (accepted + rejected changesets).
    pub changeset_acceptance_rate: Option<f64>,
    /// Passed verifications / all finished verification attempts.
    pub verification_pass_rate: Option<f64>,
}

#[derive(Debug, Default)]
struct ExecutionFact {
    finished: bool,
    status: Option<String>,
    dry_run: bool,
    mode: Option<String>,
    input_tokens: usize,
    output_tokens: usize,
    cost_units: f64,
}

#[derive(Debug, Default)]
struct ChangeFact {
    created: bool,
    proposed_files: usize,
    decision: Option<String>,
    reviewed_files: usize,
}

#[derive(Debug, Default)]
struct VerificationFact {
    finished: bool,
    success: Option<bool>,
    command_count: usize,
}

#[derive(Debug, Default)]
struct CommitFact {
    file_count: usize,
    sha: Option<String>,
    order: usize,
}

pub fn load_engineering_intelligence(run_dir: &Path) -> RepoDeskResult<EngineeringIntelligence> {
    let events = read_events(run_dir)?;
    Ok(derive_engineering_intelligence(&events))
}

/// Replay an event sequence into a deterministic, explainable read model.
///
/// Identity-bearing entities are deduplicated by their typed IDs. If a producer
/// records the same finished execution, review, verification, or commit more
/// than once, the latest event for that entity wins instead of inflating totals.
pub fn derive_engineering_intelligence(events: &[EngineeringEvent]) -> EngineeringIntelligence {
    let mut report = EngineeringIntelligence {
        project: events.first().map(|event| event.project.clone()),
        work_item_id: events.first().map(|event| event.work_item_id.clone()),
        event_count: events.len(),
        ..EngineeringIntelligence::default()
    };

    let mut executions: BTreeMap<String, ExecutionFact> = BTreeMap::new();
    let mut changesets: BTreeMap<String, ChangeFact> = BTreeMap::new();
    let mut verifications: BTreeMap<String, VerificationFact> = BTreeMap::new();
    let mut commits: BTreeMap<String, CommitFact> = BTreeMap::new();
    let mut worker_ids = BTreeSet::new();
    let mut coding_agent_ids = BTreeSet::new();
    let mut handoffs = BTreeSet::new();

    for (order, event) in events.iter().enumerate() {
        observe_workers(event, &mut worker_ids, &mut coding_agent_ids);

        match event.kind {
            EngineeringEventKind::ContextBuilt => {
                report.context.builds += 1;
                if let Some(tokens) = attribute_usize(event, "estimated_tokens") {
                    report.context.total_estimated_tokens =
                        report.context.total_estimated_tokens.saturating_add(tokens);
                    report.context.latest_estimated_tokens = Some(tokens);
                }
            }
            EngineeringEventKind::ExecutionStarted => {
                if let Some(key) = execution_key(event) {
                    update_execution_common(executions.entry(key).or_default(), event);
                }
            }
            EngineeringEventKind::ExecutionFinished => {
                if let Some(key) = execution_key(event) {
                    let fact = executions.entry(key).or_default();
                    update_execution_common(fact, event);
                    fact.finished = true;
                    fact.status = attribute_str(event, "status").map(str::to_string);
                    fact.input_tokens = attribute_usize(event, "input_tokens").unwrap_or(0);
                    fact.output_tokens = attribute_usize(event, "output_tokens").unwrap_or(0);
                    fact.cost_units = attribute_f64(event, "cost_units").unwrap_or(0.0);
                }
            }
            EngineeringEventKind::WorkerHandoff => {
                observe_handoff(event, &mut handoffs, &mut worker_ids);
            }
            EngineeringEventKind::ChangeSetCreated => {
                if let Some(key) = changeset_key(event) {
                    let fact = changesets.entry(key).or_default();
                    fact.created = true;
                    fact.proposed_files = attribute_usize(event, "file_count").unwrap_or(0);
                }
            }
            EngineeringEventKind::ChangeSetReviewed => {
                if let Some(key) = changeset_key(event) {
                    let fact = changesets.entry(key).or_default();
                    fact.decision = attribute_str(event, "decision").map(str::to_string);
                    fact.reviewed_files = attribute_usize(event, "file_count").unwrap_or(0);
                }
            }
            EngineeringEventKind::VerificationStarted => {
                if let Some(key) = verification_key(event) {
                    verifications.entry(key).or_default();
                }
            }
            EngineeringEventKind::VerificationFinished => {
                if let Some(key) = verification_key(event) {
                    let fact = verifications.entry(key).or_default();
                    fact.finished = true;
                    fact.success = attribute_bool(event, "success");
                    fact.command_count = attribute_usize(event, "command_count").unwrap_or(0);
                }
            }
            EngineeringEventKind::CommitCreated => {
                let key = event
                    .execution_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| event.id.to_string());
                let fact = commits.entry(key).or_default();
                fact.file_count = attribute_usize(event, "file_count").unwrap_or(0);
                fact.sha = commit_sha(event);
                fact.order = order;
            }
            _ => {}
        }
    }

    fold_executions(&executions, &mut report);
    fold_changesets(&changesets, &mut report);
    fold_verifications(&verifications, &mut report);
    fold_commits(&commits, &mut report);

    report.execution.unique_workers = worker_ids.len();
    report.execution.unique_coding_agents = coding_agent_ids.len();
    report.execution.handoffs = handoffs.len();
    report.rates = derive_rates(&report);

    report
}

fn update_execution_common(fact: &mut ExecutionFact, event: &EngineeringEvent) {
    if let Some(dry_run) = attribute_bool(event, "dry_run") {
        fact.dry_run = dry_run;
    }
    if let Some(mode) = attribute_str(event, "execution_mode") {
        fact.mode = Some(mode.to_string());
    }
}

fn observe_handoff(
    event: &EngineeringEvent,
    handoffs: &mut BTreeSet<String>,
    worker_ids: &mut BTreeSet<String>,
) {
    let execution = event
        .execution_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let from = attribute_str(event, "from_worker").unwrap_or_default();
    let to = attribute_str(event, "to_worker").unwrap_or_default();
    let source_step = attribute_str(event, "source_step").unwrap_or_default();
    let target_step = attribute_str(event, "target_step").unwrap_or_default();

    handoffs.insert(format!(
        "{execution}\u{1f}{from}\u{1f}{to}\u{1f}{source_step}\u{1f}{target_step}"
    ));
    if !from.is_empty() {
        worker_ids.insert(from.to_string());
    }
    if !to.is_empty() {
        worker_ids.insert(to.to_string());
    }
}

fn fold_executions(
    executions: &BTreeMap<String, ExecutionFact>,
    report: &mut EngineeringIntelligence,
) {
    report.execution.attempts = executions.len();

    for fact in executions.values() {
        if fact.finished {
            report.execution.finished += 1;
        } else {
            report.execution.unfinished += 1;
        }

        match fact.status.as_deref() {
            Some("completed") => report.execution.completed += 1,
            Some("partial") => report.execution.partial += 1,
            Some("failed") => report.execution.failed += 1,
            Some("dry_run") => report.execution.dry_runs += 1,
            _ if fact.dry_run => report.execution.dry_runs += 1,
            _ => {}
        }

        match fact.mode.as_deref() {
            Some("managed") => report.execution.managed += 1,
            Some("manual") => report.execution.manual += 1,
            _ => report.execution.unknown_mode += 1,
        }

        report.ai_usage.input_tokens = report
            .ai_usage
            .input_tokens
            .saturating_add(fact.input_tokens);
        report.ai_usage.output_tokens = report
            .ai_usage
            .output_tokens
            .saturating_add(fact.output_tokens);
        report.ai_usage.cost_units += fact.cost_units;
    }
}

fn fold_changesets(
    changesets: &BTreeMap<String, ChangeFact>,
    report: &mut EngineeringIntelligence,
) {
    for fact in changesets.values() {
        if fact.created {
            report.changes.proposed_changesets += 1;
            report.changes.proposed_files = report
                .changes
                .proposed_files
                .saturating_add(fact.proposed_files);
        }

        match fact.decision.as_deref() {
            Some("accepted") => {
                report.changes.reviewed_changesets += 1;
                report.changes.accepted_changesets += 1;
                report.changes.accepted_files = report
                    .changes
                    .accepted_files
                    .saturating_add(fact.reviewed_files);
            }
            Some("rejected") => {
                report.changes.reviewed_changesets += 1;
                report.changes.rejected_changesets += 1;
                report.changes.rejected_files = report
                    .changes
                    .rejected_files
                    .saturating_add(fact.reviewed_files);
            }
            _ => {}
        }
    }

    report.changes.pending_review_changesets = report
        .changes
        .proposed_changesets
        .saturating_sub(report.changes.reviewed_changesets);
}

fn fold_verifications(
    verifications: &BTreeMap<String, VerificationFact>,
    report: &mut EngineeringIntelligence,
) {
    report.verification.attempts = verifications.len();

    for fact in verifications.values() {
        if fact.finished {
            report.verification.finished += 1;
            report.verification.commands_run = report
                .verification
                .commands_run
                .saturating_add(fact.command_count);
        } else {
            report.verification.unfinished += 1;
        }

        match fact.success {
            Some(true) => report.verification.passed += 1,
            Some(false) => report.verification.failed += 1,
            None => {}
        }
    }
}

fn fold_commits(
    commits: &BTreeMap<String, CommitFact>,
    report: &mut EngineeringIntelligence,
) {
    report.completion.commits = commits.len();
    report.completion.committed = !commits.is_empty();

    let mut latest: Option<(usize, String)> = None;
    for fact in commits.values() {
        report.completion.committed_files = report
            .completion
            .committed_files
            .saturating_add(fact.file_count);

        if let Some(sha) = &fact.sha
            && latest
                .as_ref()
                .is_none_or(|(latest_order, _)| fact.order > *latest_order)
        {
            latest = Some((fact.order, sha.clone()));
        }
    }
    report.completion.latest_commit_sha = latest.map(|(_, sha)| sha);
}

fn derive_rates(report: &EngineeringIntelligence) -> IntelligenceRates {
    let execution_finished = report
        .execution
        .completed
        .saturating_add(report.execution.partial)
        .saturating_add(report.execution.failed);
    let reviewed = report
        .changes
        .accepted_changesets
        .saturating_add(report.changes.rejected_changesets);

    IntelligenceRates {
        execution_completion_rate: ratio(report.execution.completed, execution_finished),
        changeset_acceptance_rate: ratio(report.changes.accepted_changesets, reviewed),
        verification_pass_rate: ratio(report.verification.passed, report.verification.finished),
    }
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

fn observe_workers(
    event: &EngineeringEvent,
    worker_ids: &mut BTreeSet<String>,
    coding_agent_ids: &mut BTreeSet<String>,
) {
    if let Some(worker) = &event.worker {
        observe_worker(worker, worker_ids, coding_agent_ids);
    }

    let Some(value) = event.attributes.get("workers") else {
        return;
    };
    let Ok(workers) = serde_json::from_value::<Vec<WorkerRef>>(value.clone()) else {
        return;
    };
    for worker in &workers {
        observe_worker(worker, worker_ids, coding_agent_ids);
    }
}

fn observe_worker(
    worker: &WorkerRef,
    worker_ids: &mut BTreeSet<String>,
    coding_agent_ids: &mut BTreeSet<String>,
) {
    worker_ids.insert(worker.id.clone());
    if worker.kind == WorkerKind::CodingAgent {
        coding_agent_ids.insert(worker.id.clone());
    }
}

fn commit_sha(event: &EngineeringEvent) -> Option<String> {
    event
        .evidence
        .iter()
        .find(|evidence| evidence.kind == EvidenceKind::Commit)
        .map(|evidence| evidence.locator.clone())
}

fn execution_key(event: &EngineeringEvent) -> Option<String> {
    event.execution_id.as_ref().map(ToString::to_string)
}

fn changeset_key(event: &EngineeringEvent) -> Option<String> {
    event.changeset_id.as_ref().map(ToString::to_string)
}

fn verification_key(event: &EngineeringEvent) -> Option<String> {
    event.verification_id.as_ref().map(ToString::to_string)
}

fn attribute_str<'a>(event: &'a EngineeringEvent, key: &str) -> Option<&'a str> {
    event.attributes.get(key).and_then(Value::as_str)
}

fn attribute_bool(event: &EngineeringEvent, key: &str) -> Option<bool> {
    event.attributes.get(key).and_then(Value::as_bool)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::engineering::domain::{
        ChangeSetId, EvidenceRef, ExecutionId, VerificationId, WorkItemId,
    };

    fn base_event(kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new(
            "repodesk",
            WorkItemId::try_new("task-1").unwrap(),
            kind,
        )
    }

    fn worker(kind: WorkerKind, id: &str) -> WorkerRef {
        WorkerRef {
            kind,
            id: id.to_string(),
            provider: None,
            model: None,
        }
    }

    fn execution_event(
        kind: EngineeringEventKind,
        id: &str,
        workers: &[WorkerRef],
        mode: &str,
    ) -> EngineeringEvent {
        base_event(kind)
            .with_execution(ExecutionId::try_new(id).unwrap())
            .with_attribute("workers", json!(workers))
            .with_attribute("execution_mode", json!(mode))
    }

    #[test]
    fn derives_explainable_work_item_metrics() {
        let codex = worker(WorkerKind::CodingAgent, "codex_cli");
        let manual = worker(WorkerKind::Manual, "manual");
        let verify_1 = VerificationId::try_new("verify-1").unwrap();
        let verify_2 = VerificationId::try_new("verify-2").unwrap();

        let events = vec![
            base_event(EngineeringEventKind::WorkItemCreated),
            base_event(EngineeringEventKind::ContextBuilt)
                .with_attribute("estimated_tokens", json!(1200)),
            base_event(EngineeringEventKind::ContextBuilt)
                .with_attribute("estimated_tokens", json!(800)),
            execution_event(
                EngineeringEventKind::ExecutionStarted,
                "run-1",
                std::slice::from_ref(&codex),
                "managed",
            ),
            execution_event(
                EngineeringEventKind::ExecutionFinished,
                "run-1",
                std::slice::from_ref(&codex),
                "managed",
            )
            .with_attribute("status", json!("completed"))
            .with_attribute("input_tokens", json!(100))
            .with_attribute("output_tokens", json!(25))
            .with_attribute("cost_units", json!(0.3)),
            base_event(EngineeringEventKind::WorkerHandoff)
                .with_execution(ExecutionId::try_new("run-1").unwrap())
                .with_worker(codex.clone())
                .with_attribute("from_worker", json!("planner"))
                .with_attribute("to_worker", json!("codex_cli"))
                .with_attribute("source_step", json!("plan"))
                .with_attribute("target_step", json!("implement")),
            base_event(EngineeringEventKind::ChangeSetCreated)
                .with_changeset(ChangeSetId::try_new("run-1-changeset").unwrap())
                .with_attribute("file_count", json!(3)),
            base_event(EngineeringEventKind::ChangeSetReviewed)
                .with_changeset(ChangeSetId::try_new("run-1-changeset").unwrap())
                .with_attribute("decision", json!("accepted"))
                .with_attribute("file_count", json!(3)),
            base_event(EngineeringEventKind::VerificationStarted)
                .with_verification(verify_1.clone()),
            base_event(EngineeringEventKind::VerificationFinished)
                .with_verification(verify_1)
                .with_attribute("success", json!(true))
                .with_attribute("command_count", json!(2)),
            base_event(EngineeringEventKind::CommitCreated)
                .with_execution(ExecutionId::try_new("run-1").unwrap())
                .with_attribute("file_count", json!(3))
                .with_evidence(EvidenceRef::try_new(EvidenceKind::Commit, "abc123").unwrap()),
            execution_event(
                EngineeringEventKind::ExecutionStarted,
                "run-2",
                std::slice::from_ref(&manual),
                "manual",
            ),
            execution_event(
                EngineeringEventKind::ExecutionFinished,
                "run-2",
                std::slice::from_ref(&manual),
                "manual",
            )
            .with_attribute("status", json!("failed")),
            base_event(EngineeringEventKind::ChangeSetCreated)
                .with_changeset(ChangeSetId::try_new("run-2-changeset").unwrap())
                .with_attribute("file_count", json!(1)),
            base_event(EngineeringEventKind::ChangeSetReviewed)
                .with_changeset(ChangeSetId::try_new("run-2-changeset").unwrap())
                .with_attribute("decision", json!("rejected"))
                .with_attribute("file_count", json!(1)),
            base_event(EngineeringEventKind::VerificationStarted).with_verification(verify_2),
        ];

        let report = derive_engineering_intelligence(&events);

        assert_eq!(report.project.as_deref(), Some("repodesk"));
        assert_eq!(report.work_item_id.as_ref().unwrap().as_str(), "task-1");
        assert_eq!(report.execution.attempts, 2);
        assert_eq!(report.execution.completed, 1);
        assert_eq!(report.execution.failed, 1);
        assert_eq!(report.execution.managed, 1);
        assert_eq!(report.execution.manual, 1);
        assert_eq!(report.execution.unique_workers, 3);
        assert_eq!(report.execution.unique_coding_agents, 1);
        assert_eq!(report.execution.handoffs, 1);
        assert_eq!(report.ai_usage.input_tokens, 100);
        assert_eq!(report.ai_usage.output_tokens, 25);
        assert!((report.ai_usage.cost_units - 0.3).abs() < f64::EPSILON);
        assert_eq!(report.context.builds, 2);
        assert_eq!(report.context.total_estimated_tokens, 2000);
        assert_eq!(report.context.latest_estimated_tokens, Some(800));
        assert_eq!(report.changes.proposed_changesets, 2);
        assert_eq!(report.changes.proposed_files, 4);
        assert_eq!(report.changes.accepted_changesets, 1);
        assert_eq!(report.changes.rejected_changesets, 1);
        assert_eq!(report.changes.accepted_files, 3);
        assert_eq!(report.changes.rejected_files, 1);
        assert_eq!(report.verification.attempts, 2);
        assert_eq!(report.verification.finished, 1);
        assert_eq!(report.verification.passed, 1);
        assert_eq!(report.verification.unfinished, 1);
        assert_eq!(report.verification.commands_run, 2);
        assert!(report.completion.committed);
        assert_eq!(report.completion.commits, 1);
        assert_eq!(report.completion.latest_commit_sha.as_deref(), Some("abc123"));
        assert_eq!(report.rates.execution_completion_rate, Some(0.5));
        assert_eq!(report.rates.changeset_acceptance_rate, Some(0.5));
        assert_eq!(report.rates.verification_pass_rate, Some(1.0));
    }

    #[test]
    fn duplicate_entity_events_do_not_double_count_and_latest_commit_uses_ledger_order() {
        let codex = worker(WorkerKind::CodingAgent, "codex_cli");
        let started = execution_event(
            EngineeringEventKind::ExecutionStarted,
            "z-run",
            std::slice::from_ref(&codex),
            "managed",
        );
        let finished = execution_event(
            EngineeringEventKind::ExecutionFinished,
            "z-run",
            std::slice::from_ref(&codex),
            "managed",
        )
        .with_attribute("status", json!("completed"))
        .with_attribute("input_tokens", json!(50))
        .with_attribute("output_tokens", json!(10))
        .with_attribute("cost_units", json!(0.1));
        let first_commit = base_event(EngineeringEventKind::CommitCreated)
            .with_execution(ExecutionId::try_new("z-run").unwrap())
            .with_evidence(EvidenceRef::try_new(EvidenceKind::Commit, "older").unwrap());
        let latest_commit = base_event(EngineeringEventKind::CommitCreated)
            .with_execution(ExecutionId::try_new("a-run").unwrap())
            .with_evidence(EvidenceRef::try_new(EvidenceKind::Commit, "latest").unwrap());

        let report = derive_engineering_intelligence(&[
            started,
            finished.clone(),
            finished,
            first_commit,
            latest_commit,
        ]);

        assert_eq!(report.execution.attempts, 1);
        assert_eq!(report.ai_usage.input_tokens, 50);
        assert_eq!(report.ai_usage.output_tokens, 10);
        assert!((report.ai_usage.cost_units - 0.1).abs() < f64::EPSILON);
        assert_eq!(report.completion.commits, 2);
        assert_eq!(report.completion.latest_commit_sha.as_deref(), Some("latest"));
    }

    #[test]
    fn legacy_events_do_not_invent_workers_or_execution_mode() {
        let events = vec![
            base_event(EngineeringEventKind::ExecutionStarted)
                .with_execution(ExecutionId::try_new("legacy-run").unwrap())
                .with_attribute("worker_count", json!(2)),
            base_event(EngineeringEventKind::ExecutionFinished)
                .with_execution(ExecutionId::try_new("legacy-run").unwrap())
                .with_attribute("status", json!("completed"))
                .with_attribute("worker_count", json!(2)),
        ];

        let report = derive_engineering_intelligence(&events);
        assert_eq!(report.execution.attempts, 1);
        assert_eq!(report.execution.completed, 1);
        assert_eq!(report.execution.unique_workers, 0);
        assert_eq!(report.execution.unknown_mode, 1);
    }

    #[test]
    fn empty_history_produces_empty_report_and_no_rates() {
        let report = derive_engineering_intelligence(&[]);
        assert_eq!(report, EngineeringIntelligence::default());
        assert_eq!(report.rates.execution_completion_rate, None);
        assert_eq!(report.rates.changeset_acceptance_rate, None);
        assert_eq!(report.rates.verification_pass_rate, None);
    }
}