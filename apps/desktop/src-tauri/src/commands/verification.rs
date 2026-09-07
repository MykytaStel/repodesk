use std::collections::BTreeMap;
use std::path::Path;

use chrono::Utc;
use repodesk_core::engineering::events::{EngineeringEvent, EngineeringEventKind};
use repodesk_core::engineering::instrumentation::record_decision_receipt;
use repodesk_core::engineering::{
    DecisionReceipt, EvidenceRef, VerificationAdvisorInput, VerificationCheckCandidate,
    VerificationDebt, VerificationDecisionKind, VerificationRecommendation, recommend_verification,
};
use repodesk_core::git_workspace::build_git_workspace_snapshot_for_path;
use repodesk_core::projects::get_active_project;
use repodesk_core::tasks::show_active_task;
use repodesk_core::workflow::index_tree_sha;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ErrorPayload;

pub type DecisionReceiptDto = DecisionReceipt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDecisionInput {
    pub decision_id: Option<String>,
    pub project: String,
    pub work_item_id: String,
    pub execution_id: Option<String>,
    pub tree_identity: String,
    pub changeset_identity: Option<String>,
    pub decision_kind: VerificationDecisionKind,
    pub policy_version: String,
    pub observed_facts: BTreeMap<String, Value>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub selected_actions: Vec<String>,
    pub skipped_actions: Vec<String>,
    pub estimated_cost_units: Option<f64>,
    pub estimated_wall_clock_ms: Option<u64>,
    pub risk_label: String,
    pub uncertainty_label: String,
    pub verification_debt: Vec<VerificationDebt>,
    pub human_override: Option<String>,
}

impl RecordDecisionInput {
    pub fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("project", self.project.as_str()),
            ("work_item_id", self.work_item_id.as_str()),
            ("tree_identity", self.tree_identity.as_str()),
            ("policy_version", self.policy_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{label} cannot be empty"));
            }
        }
        if let Some(decision_id) = self.decision_id.as_deref()
            && decision_id.trim().is_empty()
        {
            return Err("decision_id cannot be empty".into());
        }
        if let Some(reason) = self.human_override.as_deref()
            && reason.trim().is_empty()
        {
            return Err("human_override cannot be empty".into());
        }
        if self.decision_kind == VerificationDecisionKind::DeferWithDebt {
            if self.verification_debt.is_empty() {
                return Err("deferred verification must include explicit debt".into());
            }
            if self
                .verification_debt
                .iter()
                .any(|debt| debt.reason.trim().is_empty())
            {
                return Err("every deferred check must include a debt reason".into());
            }
        }
        Ok(())
    }

    pub fn into_receipt(self) -> Result<DecisionReceiptDto, String> {
        self.validate()?;
        let mut receipt = DecisionReceipt::try_new(
            self.project,
            self.work_item_id,
            self.tree_identity,
            self.decision_kind,
            self.policy_version,
        )
        .map_err(|error| error.to_string())?;
        if let Some(decision_id) = self.decision_id {
            receipt.decision_id = decision_id;
        }
        receipt.execution_id = self.execution_id;
        receipt.changeset_identity = self.changeset_identity;
        receipt.observed_facts = self.observed_facts;
        receipt.evidence_refs = self.evidence_refs;
        receipt.selected_actions = self.selected_actions;
        receipt.skipped_actions = self.skipped_actions;
        receipt.estimated_cost_units = self.estimated_cost_units;
        receipt.estimated_wall_clock_ms = self.estimated_wall_clock_ms;
        receipt.risk_label = self.risk_label;
        receipt.uncertainty_label = self.uncertainty_label;
        receipt.verification_debt = self.verification_debt;
        receipt.human_override = self.human_override;
        Ok(receipt)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSourceStatus {
    pub source: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationAdvisorSnapshot {
    pub project: String,
    pub work_item_id: String,
    pub current_tree_identity: Option<String>,
    pub input: VerificationAdvisorInput,
    pub recommendation: VerificationRecommendation,
    pub latest_receipt: Option<DecisionReceiptDto>,
    pub sources: Vec<VerificationSourceStatus>,
}

fn decision_event_kind(receipt: &DecisionReceipt) -> EngineeringEventKind {
    if receipt.outcome.is_some() {
        return EngineeringEventKind::DecisionOutcomeRecorded;
    }
    if receipt.human_override.is_some() {
        return EngineeringEventKind::DecisionOverridden;
    }
    match receipt.decision_kind {
        VerificationDecisionKind::RunNow | VerificationDecisionKind::RunTargeted => {
            EngineeringEventKind::VerificationSelected
        }
        VerificationDecisionKind::DeferWithDebt => EngineeringEventKind::VerificationDeferred,
        VerificationDecisionKind::AskForApproval
        | VerificationDecisionKind::PauseAndReview
        | VerificationDecisionKind::StopWithPartialResult => {
            EngineeringEventKind::VerificationRecommended
        }
    }
}

pub fn append_decision_event(
    run_dir: &Path,
    receipt: &DecisionReceipt,
) -> repodesk_core::errors::RepoDeskResult<()> {
    let work_item_id =
        repodesk_core::engineering::WorkItemId::try_new(receipt.work_item_id.clone())
            .map_err(|error| repodesk_core::errors::RepoDeskError::Api(error.to_string()))?;
    let mut event = EngineeringEvent::new(
        receipt.project.clone(),
        work_item_id,
        decision_event_kind(receipt),
    )
    .with_attribute("decision_receipt", serde_json::to_value(receipt)?)
    .with_attribute("decision_id", json!(receipt.decision_id))
    .with_attribute("decision_kind", json!(receipt.decision_kind))
    .with_attribute("tree_identity", json!(receipt.tree_identity))
    .with_attribute("policy_version", json!(receipt.policy_version))
    .with_attribute("risk_label", json!(receipt.risk_label))
    .with_attribute("uncertainty_label", json!(receipt.uncertainty_label))
    .with_attribute(
        "verification_debt_count",
        json!(receipt.verification_debt.len()),
    );

    for evidence in &receipt.evidence_refs {
        event = event.with_evidence(evidence.clone());
    }
    if let Some(execution_id) = receipt.execution_id.as_deref()
        && let Ok(execution_id) = repodesk_core::engineering::ExecutionId::try_new(execution_id)
    {
        event = event.with_execution(execution_id);
    }
    if let Some(changeset_id) = receipt.changeset_identity.as_deref()
        && let Ok(changeset_id) = repodesk_core::engineering::ChangeSetId::try_new(changeset_id)
    {
        event = event.with_changeset(changeset_id);
    }

    repodesk_core::engineering::append_event(run_dir, &event).map(|_| ())
}

pub fn read_latest_decision_receipt(
    run_dir: &Path,
    decision_id: Option<&str>,
) -> repodesk_core::errors::RepoDeskResult<Option<DecisionReceiptDto>> {
    let events = repodesk_core::engineering::read_events(run_dir)?;
    Ok(events.iter().rev().find_map(|event| {
        let value = event.attributes.get("decision_receipt")?;
        let receipt = serde_json::from_value::<DecisionReceipt>(value.clone()).ok()?;
        if decision_id.is_none_or(|id| receipt.decision_id == id) {
            Some(receipt)
        } else {
            None
        }
    }))
}

#[tauri::command]
pub fn work_verification_advisor() -> Result<VerificationAdvisorSnapshot, ErrorPayload> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let current_tree_identity = index_tree_sha(&project.path);
    let git_snapshot =
        build_git_workspace_snapshot_for_path(&project.name, &project.path, Utc::now());
    let changed_files = git_snapshot
        .changed_files
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    let events = repodesk_core::engineering::read_events(&task.config.run_dir)?;
    let checks = project
        .checks
        .iter()
        .map(|check| VerificationCheckCandidate {
            id: check.id.clone(),
            title: check.title.clone(),
            command: check.command.clone(),
            kind: check.kind.clone(),
            required: check.required,
            estimated_seconds: None,
            estimated_cost_units: None,
            relevant_paths: check.relevant_paths.clone(),
            last_status: None,
        })
        .collect::<Vec<_>>();
    let input = VerificationAdvisorInput {
        project: project.name.clone(),
        work_item_id: task.config.id.clone(),
        tree_identity: current_tree_identity
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        changed_files,
        risk_label: "unknown".into(),
        proof_obligations: Vec::new(),
        checks,
        estimated_budget_units: None,
        prior_attempts: events
            .iter()
            .filter(|event| event.kind == EngineeringEventKind::VerificationStarted)
            .count(),
        prior_failures: events
            .iter()
            .filter(|event| {
                event.kind == EngineeringEventKind::VerificationFinished
                    && event.attributes.get("success").and_then(Value::as_bool) == Some(false)
            })
            .count(),
        policy_version: repodesk_core::engineering::ADAPTIVE_VERIFICATION_POLICY_VERSION.into(),
    };
    let recommendation = recommend_verification(&input);
    let mut sources = vec![
        VerificationSourceStatus {
            source: "git_tree".into(),
            status: if current_tree_identity.is_some() { "measured" } else { "unknown" }.into(),
            detail: current_tree_identity
                .as_ref()
                .map(|_| "Current index tree identity is available.".into())
                .unwrap_or_else(|| "Git tree identity is unavailable; freshness is unknown.".into()),
        },
        VerificationSourceStatus {
            source: "check_history".into(),
            status: "partial".into(),
            detail: "Recent verification attempts are known, but check duration/cost history is not measured yet.".into(),
        },
        VerificationSourceStatus {
            source: "repopilot".into(),
            status: "unknown".into(),
            detail: "RepoPilot findings are not available in this advisor snapshot.".into(),
        },
    ];
    if git_snapshot.warnings.is_empty() {
        sources.push(VerificationSourceStatus {
            source: "git_status".into(),
            status: "measured".into(),
            detail: "Changed paths were read from the local Git worktree.".into(),
        });
    } else {
        sources.push(VerificationSourceStatus {
            source: "git_status".into(),
            status: "unknown".into(),
            detail: git_snapshot.warnings.join(" "),
        });
    }

    Ok(VerificationAdvisorSnapshot {
        project: project.name,
        work_item_id: task.config.id,
        current_tree_identity,
        input,
        recommendation,
        latest_receipt: read_latest_decision_receipt(&task.config.run_dir, None)?,
        sources,
    })
}

#[tauri::command]
pub fn work_record_decision(
    input: RecordDecisionInput,
) -> Result<DecisionReceiptDto, ErrorPayload> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    if input.project != project.name || input.work_item_id != task.config.id {
        return Err(ErrorPayload::configuration(
            "Decision scope does not match the active project and Work Item",
        ));
    }
    let receipt = input.into_receipt().map_err(ErrorPayload::configuration)?;
    record_decision_receipt(&task.config, &receipt).map_err(ErrorPayload::from)?;
    Ok(receipt)
}

#[tauri::command]
pub fn work_decision_receipt(
    decision_id: Option<String>,
) -> Result<Option<DecisionReceiptDto>, ErrorPayload> {
    if let Some(decision_id) = decision_id.as_deref()
        && decision_id.trim().is_empty()
    {
        return Err(ErrorPayload::configuration(
            "decision_id cannot be empty when provided",
        ));
    }
    let task = show_active_task()?;
    read_latest_decision_receipt(&task.config.run_dir, decision_id.as_deref())
        .map_err(ErrorPayload::from)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use repodesk_core::engineering::{
        DecisionReceipt, EvidenceKind, EvidenceRef, VerificationDebt, VerificationDecisionKind,
    };
    use serde_json::json;
    use serial_test::serial;
    use tempfile::TempDir;

    use super::{DecisionReceiptDto, RecordDecisionInput, append_decision_event};

    fn sample_input() -> RecordDecisionInput {
        RecordDecisionInput {
            decision_id: None,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            execution_id: None,
            tree_identity: "tree-1".into(),
            changeset_identity: None,
            decision_kind: VerificationDecisionKind::DeferWithDebt,
            policy_version: "verification-policy-v1".into(),
            observed_facts: BTreeMap::from([(String::from("test_count"), json!(null))]),
            evidence_refs: vec![
                EvidenceRef::try_new(EvidenceKind::Verification, "checks.log").unwrap(),
            ],
            selected_actions: Vec::new(),
            skipped_actions: vec!["integration".into()],
            estimated_cost_units: None,
            estimated_wall_clock_ms: None,
            risk_label: "medium".into(),
            uncertainty_label: "unknown".into(),
            verification_debt: vec![VerificationDebt {
                check_id: "integration".into(),
                title: "Integration suite".into(),
                reason: "Not relevant to this focused change".into(),
                required_before: "acceptance".into(),
            }],
            human_override: None,
        }
    }

    #[test]
    fn blank_decision_ids_and_tree_identities_are_rejected() {
        let mut input = sample_input();
        input.decision_id = Some(" ".into());
        assert!(input.validate().is_err());

        input.decision_id = None;
        input.tree_identity.clear();
        assert!(input.validate().is_err());
    }

    #[test]
    fn deferred_checks_require_a_debt_reason() {
        let mut input = sample_input();
        input.verification_debt[0].reason = " ".into();
        assert!(input.validate().is_err());

        input.verification_debt[0].reason = "Not relevant".into();
        input.verification_debt.clear();
        assert!(input.validate().is_err());
    }

    #[test]
    fn decision_receipt_round_trips_through_command_dto() {
        let receipt = sample_input().into_receipt().unwrap();
        let dto: DecisionReceiptDto = receipt.clone();
        let encoded = serde_json::to_string(&dto).unwrap();
        let decoded: DecisionReceiptDto = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, dto);
        assert_eq!(decoded.observed_facts["test_count"], json!(null));
        assert_eq!(decoded.verification_debt.len(), 1);
    }

    #[test]
    #[serial]
    fn recording_decisions_appends_without_mutating_historical_events() {
        let home = TempDir::new().unwrap();
        // SAFETY: this test is serialized because REPODESK_HOME is process-global.
        unsafe {
            std::env::set_var("REPODESK_HOME", home.path());
        }
        repodesk_core::init::init_home().unwrap();
        let run_dir = home.path().join("runs/repodesk/task-1");
        std::fs::create_dir_all(&run_dir).unwrap();

        let first = sample_input().into_receipt().unwrap();
        append_decision_event(&run_dir, &first).unwrap();
        let before = repodesk_core::engineering::read_events(&run_dir).unwrap();
        assert_eq!(before.len(), 1);

        let second = DecisionReceipt::new(
            "repodesk",
            "task-1",
            "tree-2",
            VerificationDecisionKind::RunTargeted,
            "verification-policy-v1",
        );
        append_decision_event(&run_dir, &second).unwrap();
        let after = repodesk_core::engineering::read_events(&run_dir).unwrap();

        assert_eq!(after.len(), 2);
        assert_eq!(after[0].id, before[0].id);
        assert_ne!(after[1].id, before[0].id);
    }
}
