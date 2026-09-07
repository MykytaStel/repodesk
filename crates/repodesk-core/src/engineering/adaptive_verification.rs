use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::verification_history::VerificationHistoryConfidence;
use super::{VerificationDebt, VerificationDecisionKind};

pub const ADAPTIVE_VERIFICATION_POLICY_VERSION: &str = "verification-policy-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationCheckCandidate {
    pub id: String,
    pub title: String,
    pub command: String,
    pub kind: String,
    pub required: bool,
    pub estimated_seconds: Option<u64>,
    pub estimated_cost_units: Option<f64>,
    pub relevant_paths: Vec<String>,
    pub last_status: Option<String>,
    pub measured_runs: usize,
    pub failed_runs: usize,
    pub median_duration_ms: Option<u64>,
    pub history_confidence: VerificationHistoryConfidence,
    pub latest_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationAdvisorInput {
    pub project: String,
    pub work_item_id: String,
    pub tree_identity: String,
    pub changed_files: Vec<String>,
    pub risk_label: String,
    pub proof_obligations: Vec<String>,
    pub checks: Vec<VerificationCheckCandidate>,
    pub estimated_budget_units: Option<f64>,
    pub prior_attempts: usize,
    pub prior_failures: usize,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationRecommendation {
    pub decision: VerificationDecisionKind,
    pub rationale: Vec<String>,
    pub selected_check_ids: Vec<String>,
    pub deferred_checks: Vec<VerificationDebt>,
    pub estimated_cost_units: Option<f64>,
    pub estimated_wall_clock_ms: Option<u64>,
    pub risk_label: String,
    pub uncertainty_label: String,
    pub policy_version: String,
}

pub fn recommend_verification(input: &VerificationAdvisorInput) -> VerificationRecommendation {
    let policy_version = if input.policy_version.trim().is_empty() {
        ADAPTIVE_VERIFICATION_POLICY_VERSION.to_string()
    } else {
        input.policy_version.clone()
    };
    let risk_label = if input.risk_label.trim().is_empty() {
        "unknown".to_string()
    } else {
        input.risk_label.clone()
    };

    if input.prior_failures >= 2 {
        return recommendation_for_human_review(input, risk_label, policy_version);
    }

    let mut checks = input.checks.iter().collect::<Vec<_>>();
    checks.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| {
                path_intersects(&input.changed_files, &left.relevant_paths).cmp(&path_intersects(
                    &input.changed_files,
                    &right.relevant_paths,
                ))
            })
            .then_with(|| left.estimated_seconds.cmp(&right.estimated_seconds))
            .then_with(|| left.id.cmp(&right.id))
    });

    let required = checks
        .iter()
        .filter(|check| check.required)
        .copied()
        .collect::<Vec<_>>();

    if !required.is_empty() {
        let deferred_checks = checks
            .iter()
            .filter(|check| !check.required)
            .map(|check| debt_for(check, "not required for the current verification decision"))
            .collect::<Vec<_>>();

        return build_recommendation(
            VerificationDecisionKind::RunNow,
            vec!["A required verification check must run before this change is accepted."],
            required,
            deferred_checks,
            input,
            risk_label,
            policy_version,
        );
    }

    let intersecting = checks
        .iter()
        .filter(|check| path_intersects(&input.changed_files, &check.relevant_paths))
        .copied()
        .collect::<Vec<_>>();

    if let Some(targeted) = intersecting
        .iter()
        .find(|check| check.kind.eq_ignore_ascii_case("targeted"))
        .copied()
        .or_else(|| intersecting.first().copied())
    {
        let selected = vec![targeted];
        let deferred_checks = checks
            .iter()
            .filter(|check| check.id != targeted.id)
            .map(|check| {
                let reason = if path_intersects(&input.changed_files, &check.relevant_paths) {
                    "not selected because a more focused verification check is available"
                } else {
                    "unrelated to the changed paths for this work item"
                };
                debt_for(check, reason)
            })
            .collect::<Vec<_>>();

        return build_recommendation(
            VerificationDecisionKind::RunTargeted,
            vec!["A focused check intersects the changed paths and is the smallest useful proof."],
            selected,
            deferred_checks,
            input,
            risk_label,
            policy_version,
        );
    }

    if checks.is_empty() {
        return build_recommendation(
            VerificationDecisionKind::StopWithPartialResult,
            vec!["No verification checks are configured; the result cannot be proved locally."],
            Vec::new(),
            Vec::new(),
            input,
            risk_label,
            policy_version,
        );
    }

    let deferred_checks = checks
        .iter()
        .map(|check| debt_for(check, "unrelated to the changed paths for this work item"))
        .collect::<Vec<_>>();

    build_recommendation(
        VerificationDecisionKind::DeferWithDebt,
        vec!["No configured check intersects the changed paths; verification is explicit debt."],
        Vec::new(),
        deferred_checks,
        input,
        risk_label,
        policy_version,
    )
}

fn recommendation_for_human_review(
    input: &VerificationAdvisorInput,
    risk_label: String,
    policy_version: String,
) -> VerificationRecommendation {
    let mut rationale = vec![
        "Repeated failed verification attempts require a human review before another run.".into(),
    ];
    if input.prior_attempts > input.prior_failures {
        rationale.push(format!(
            "The current history contains {} failed attempts out of {} total attempts.",
            input.prior_failures, input.prior_attempts
        ));
    } else {
        rationale.push(format!(
            "The current history contains {} failed verification attempts.",
            input.prior_failures
        ));
    }

    VerificationRecommendation {
        decision: VerificationDecisionKind::AskForApproval,
        rationale,
        selected_check_ids: Vec::new(),
        deferred_checks: Vec::new(),
        estimated_cost_units: None,
        estimated_wall_clock_ms: None,
        risk_label,
        uncertainty_label: uncertainty_for_checks(input.checks.iter()),
        policy_version,
    }
}

fn build_recommendation(
    decision: VerificationDecisionKind,
    rationale: Vec<&str>,
    selected: Vec<&VerificationCheckCandidate>,
    deferred_checks: Vec<VerificationDebt>,
    input: &VerificationAdvisorInput,
    risk_label: String,
    policy_version: String,
) -> VerificationRecommendation {
    let rationale = rationale
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let selected_check_ids = selected
        .iter()
        .map(|check| check.id.clone())
        .collect::<Vec<_>>();
    let estimated_cost_units = selected
        .iter()
        .map(|check| check.estimated_cost_units)
        .try_fold(0.0, |total, estimate| estimate.map(|value| total + value));
    let estimated_wall_clock_ms = selected
        .iter()
        .map(|check| {
            check.median_duration_ms.or_else(|| {
                check
                    .estimated_seconds
                    .map(|seconds| seconds.saturating_mul(1_000))
            })
        })
        .try_fold(0_u64, |total, estimate| {
            estimate.map(|value| total.saturating_add(value))
        });

    let mut recommendation = VerificationRecommendation {
        decision,
        rationale,
        selected_check_ids,
        deferred_checks,
        estimated_cost_units,
        estimated_wall_clock_ms,
        risk_label,
        uncertainty_label: uncertainty_for_checks(selected.iter().copied()),
        policy_version,
    };

    if let Some(budget) = input.estimated_budget_units
        && recommendation
            .estimated_cost_units
            .is_some_and(|estimate| estimate > budget)
        && decision == VerificationDecisionKind::RunTargeted
    {
        recommendation.rationale.push(format!(
            "The selected proof is estimated above the available budget of {budget:.2} units."
        ));
    }

    if recommendation.uncertainty_label == "unknown" {
        recommendation
            .rationale
            .push("Recent check history is incomplete, so confidence remains unknown.".into());
    }

    recommendation
}

fn debt_for(check: &VerificationCheckCandidate, reason: &str) -> VerificationDebt {
    VerificationDebt {
        check_id: check.id.clone(),
        title: check.title.clone(),
        reason: reason.to_string(),
        required_before: "acceptance".to_string(),
    }
}

fn uncertainty_for_checks<'a>(
    checks: impl IntoIterator<Item = &'a VerificationCheckCandidate>,
) -> String {
    let checks = checks.into_iter().collect::<Vec<_>>();
    if checks.is_empty() {
        "unknown".to_string()
    } else if checks
        .iter()
        .any(|check| check_confidence(check) == "unknown")
    {
        "unknown".to_string()
    } else if checks
        .iter()
        .any(|check| check_confidence(check) == "provisional")
    {
        "provisional".to_string()
    } else {
        "calibrated".to_string()
    }
}

fn check_confidence(check: &VerificationCheckCandidate) -> &'static str {
    match check.history_confidence {
        VerificationHistoryConfidence::Unknown => {
            if check.estimated_seconds.is_some()
                && check.last_status.is_some()
                && check.median_duration_ms.is_none()
            {
                "calibrated"
            } else {
                "unknown"
            }
        }
        VerificationHistoryConfidence::Provisional => "provisional",
        VerificationHistoryConfidence::Calibrated => "calibrated",
    }
}

fn path_intersects(changed_files: &[String], relevant_paths: &[String]) -> bool {
    changed_files.iter().any(|changed| {
        relevant_paths.iter().any(|relevant| {
            let changed = normalize_path(changed);
            let relevant = normalize_path(relevant);
            changed == relevant
                || changed.starts_with(&format!("{relevant}/"))
                || relevant.starts_with(&format!("{changed}/"))
        })
    })
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_matches('/').to_ascii_lowercase()
}
