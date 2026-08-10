use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{RepoDeskError, RepoDeskResult};

use super::types::{
    ObserveOutcome, RecoveryActionKind, RecoveryAttempt, RecoveryAttemptResult,
    RecoveryFailureCode, RecoveryObservation, RecoveryRecord, RecoverySnapshot, RecoveryState,
    RepairCompletion,
};

const AUTOMATIC_ATTEMPT_LIMIT: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryEngine {
    pub(super) project: String,
    records: BTreeMap<String, RecoveryRecord>,
    history: VecDeque<RecoveryAttempt>,
    active_attempts: BTreeMap<String, String>,
    pub(super) history_limit: usize,
    sequence: u64,
    warnings: Vec<String>,
}

impl RecoveryEngine {
    pub fn new(project: String, history_limit: usize) -> Self {
        Self {
            project,
            records: BTreeMap::new(),
            history: VecDeque::new(),
            active_attempts: BTreeMap::new(),
            history_limit,
            sequence: 0,
            warnings: Vec::new(),
        }
    }

    pub fn observe(&mut self, observation: RecoveryObservation) -> ObserveOutcome {
        if self
            .records
            .get(&observation.capability_id)
            .is_some_and(|current| observation.generation < current.generation)
        {
            return ObserveOutcome::IgnoredStale;
        }

        let diagnosis_revision = diagnosis_revision(&observation);
        let automatic_attempts = self
            .records
            .get(&observation.capability_id)
            .filter(|current| current.diagnosis_revision == diagnosis_revision)
            .map_or(0, |current| current.automatic_attempts);
        let mut actions = observation.actions;
        if observation.state == RecoveryState::Healthy {
            actions.clear();
        }
        if observation.code == Some(RecoveryFailureCode::UnknownFailure) {
            actions.retain(|candidate| {
                candidate.kind == RecoveryActionKind::Manual && candidate.recipe_id.is_none()
            });
        }
        if automatic_attempts >= AUTOMATIC_ATTEMPT_LIMIT {
            actions.retain(|candidate| candidate.kind != RecoveryActionKind::Automatic);
        }

        let record = RecoveryRecord {
            capability_id: observation.capability_id,
            module_id: observation.module_id,
            generation: observation.generation,
            diagnosis_revision,
            observed_at: observation.observed_at,
            state: observation.state,
            severity: observation.severity,
            code: observation.code,
            title: observation.title,
            explanation: observation.explanation,
            affected: observation.affected,
            unaffected: observation.unaffected,
            evidence: observation.evidence,
            actions,
            automatic_attempts,
        };
        self.records
            .insert(record.capability_id.clone(), record.clone());
        ObserveOutcome::Applied(Box::new(record))
    }

    pub fn begin_repair(
        &mut self,
        capability_id: &str,
        action_id: &str,
        started_at: DateTime<Utc>,
    ) -> RepoDeskResult<RecoveryRecord> {
        if self.active_attempts.contains_key(capability_id) {
            return Err(RepoDeskError::Api(format!(
                "Recovery is already running for '{capability_id}'"
            )));
        }

        let record = self.records.get_mut(capability_id).ok_or_else(|| {
            RepoDeskError::Api(format!(
                "Recovery capability '{capability_id}' was not found"
            ))
        })?;
        let action = record
            .actions
            .iter()
            .find(|candidate| candidate.id == action_id)
            .cloned()
            .ok_or_else(|| {
                RepoDeskError::Api(format!(
                    "Recovery action '{action_id}' is not available for '{capability_id}'"
                ))
            })?;
        if action.kind == RecoveryActionKind::Automatic {
            if record.automatic_attempts >= AUTOMATIC_ATTEMPT_LIMIT {
                return Err(RepoDeskError::Api(format!(
                    "Automatic recovery attempt limit reached for '{capability_id}'"
                )));
            }
            record.automatic_attempts += 1;
        }

        self.sequence += 1;
        let attempt_id = format!("recovery-attempt-{}", self.sequence);
        self.history.push_back(RecoveryAttempt {
            id: attempt_id.clone(),
            capability_id: capability_id.into(),
            diagnosis_revision: record.diagnosis_revision.clone(),
            action_id: action_id.into(),
            started_at,
            finished_at: None,
            result: None,
            verification_summary: None,
        });
        self.active_attempts
            .insert(capability_id.into(), attempt_id);
        record.state = RecoveryState::Repairing;
        let updated = record.clone();
        self.enforce_history_limit();
        Ok(updated)
    }

    pub fn finish_repair(
        &mut self,
        capability_id: &str,
        completion: RepairCompletion,
    ) -> RepoDeskResult<RecoveryRecord> {
        let attempt_id = self.active_attempts.remove(capability_id).ok_or_else(|| {
            RepoDeskError::Api(format!("No recovery is running for '{capability_id}'"))
        })?;
        let (finished_at, summary, result) = completion.parts();
        let attempt = self
            .history
            .iter_mut()
            .find(|candidate| candidate.id == attempt_id)
            .ok_or_else(|| {
                RepoDeskError::Api(format!("Recovery attempt '{attempt_id}' was not found"))
            })?;
        attempt.finished_at = Some(finished_at);
        attempt.result = Some(result);
        attempt.verification_summary = Some(summary.to_string());

        let record = self.records.get_mut(capability_id).ok_or_else(|| {
            RepoDeskError::Api(format!(
                "Recovery capability '{capability_id}' was not found"
            ))
        })?;
        record.observed_at = finished_at;
        match result {
            RecoveryAttemptResult::Verified => {
                record.state = RecoveryState::Healthy;
                record.code = None;
                record.actions.clear();
            }
            RecoveryAttemptResult::Cancelled => {
                record.state = RecoveryState::NeedsApproval;
            }
            RecoveryAttemptResult::Failed | RecoveryAttemptResult::VerificationFailed => {
                if record.automatic_attempts >= AUTOMATIC_ATTEMPT_LIMIT {
                    record.state = RecoveryState::Blocked;
                    record
                        .actions
                        .retain(|candidate| candidate.kind != RecoveryActionKind::Automatic);
                } else {
                    record.state = RecoveryState::Degraded;
                }
            }
        }
        Ok(record.clone())
    }

    pub fn snapshot(&self) -> RecoverySnapshot {
        let records = self.records.values().cloned().collect::<Vec<_>>();
        let actionable_count = records
            .iter()
            .filter(|record| {
                matches!(
                    record.state,
                    RecoveryState::Degraded | RecoveryState::NeedsApproval | RecoveryState::Blocked
                )
            })
            .count();
        let generated_at = records
            .iter()
            .map(|record| record.observed_at)
            .max()
            .unwrap_or_else(Utc::now);
        RecoverySnapshot {
            project: self.project.clone(),
            records,
            actionable_count,
            warnings: self.warnings.clone(),
            generated_at,
        }
    }

    pub fn history(&self) -> Vec<RecoveryAttempt> {
        self.history.iter().cloned().collect()
    }

    pub fn prune_history(&mut self, cutoff: DateTime<Utc>) {
        self.history.retain(|attempt| {
            attempt
                .finished_at
                .is_none_or(|finished_at| finished_at >= cutoff)
        });
        self.enforce_history_limit();
    }

    fn enforce_history_limit(&mut self) {
        while self.history.len() > self.history_limit {
            if let Some(removed) = self.history.pop_front() {
                self.active_attempts
                    .retain(|_, attempt_id| attempt_id != &removed.id);
            }
        }
    }

    pub(super) fn enforce_loaded_history_limit(&mut self) {
        self.enforce_history_limit();
    }
}

fn diagnosis_revision(observation: &RecoveryObservation) -> String {
    let code = observation
        .code
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "healthy".into());
    format!(
        "{}:{}:{}",
        observation.capability_id, observation.generation, code
    )
}
