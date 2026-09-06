use repodesk_core::engineering::{
    VerificationAdvisorInput, VerificationCheckCandidate, VerificationDecisionKind,
    recommend_verification,
};

fn check(
    id: &str,
    title: &str,
    kind: &str,
    required: bool,
    estimated_seconds: Option<u64>,
    relevant_paths: &[&str],
) -> VerificationCheckCandidate {
    VerificationCheckCandidate {
        id: id.into(),
        title: title.into(),
        command: format!("cargo test -- {id}"),
        kind: kind.into(),
        required,
        estimated_seconds,
        estimated_cost_units: estimated_seconds.map(|seconds| seconds as f64 / 10.0),
        relevant_paths: relevant_paths.iter().map(|path| (*path).into()).collect(),
        last_status: None,
    }
}

fn input(checks: Vec<VerificationCheckCandidate>) -> VerificationAdvisorInput {
    VerificationAdvisorInput {
        project: "repodesk".into(),
        work_item_id: "work-123".into(),
        tree_identity: "tree-sha".into(),
        changed_files: vec!["apps/desktop/src/features/work/WorkSurface.tsx".into()],
        risk_label: "medium".into(),
        proof_obligations: vec!["work-surface-renders".into()],
        checks,
        estimated_budget_units: Some(20.0),
        prior_attempts: 0,
        prior_failures: 0,
        policy_version: "verification-policy-v1".into(),
    }
}

#[test]
fn required_check_is_selected_even_when_expensive() {
    let verification_input = input(vec![check(
        "release",
        "Release suite",
        "suite",
        true,
        Some(900),
        &["apps/desktop/src/features/work/WorkSurface.tsx"],
    )]);
    let recommendation = recommend_verification(&verification_input);

    assert_eq!(recommendation.decision, VerificationDecisionKind::RunNow);
    assert_eq!(recommendation.selected_check_ids, vec!["release"]);
    assert!(recommendation.deferred_checks.is_empty());
}

#[test]
fn focused_change_prefers_intersecting_targeted_check() {
    let verification_input = input(vec![
        check(
            "ui-targeted",
            "Work surface browser test",
            "targeted",
            false,
            Some(20),
            &["apps/desktop/src/features/work/WorkSurface.tsx"],
        ),
        check(
            "full-suite",
            "Full suite",
            "suite",
            false,
            Some(600),
            &["crates/"],
        ),
    ]);
    let recommendation = recommend_verification(&verification_input);

    assert_eq!(
        recommendation.decision,
        VerificationDecisionKind::RunTargeted
    );
    assert_eq!(recommendation.selected_check_ids, vec!["ui-targeted"]);
    assert_eq!(recommendation.deferred_checks.len(), 1);
    assert_eq!(recommendation.deferred_checks[0].check_id, "full-suite");
}

#[test]
fn expensive_unrelated_suite_becomes_explicit_debt() {
    let verification_input = input(vec![check(
        "integration",
        "Integration suite",
        "suite",
        false,
        Some(1_200),
        &["services/"],
    )]);
    let recommendation = recommend_verification(&verification_input);

    assert_eq!(
        recommendation.decision,
        VerificationDecisionKind::DeferWithDebt
    );
    assert!(recommendation.selected_check_ids.is_empty());
    assert_eq!(recommendation.deferred_checks[0].check_id, "integration");
    assert!(
        recommendation.deferred_checks[0]
            .reason
            .contains("unrelated")
    );
}

#[test]
fn repeated_failed_attempts_request_human_review() {
    let mut verification_input = input(vec![check(
        "ui-targeted",
        "Work surface browser test",
        "targeted",
        false,
        Some(20),
        &["apps/desktop/src/features/work/WorkSurface.tsx"],
    )]);
    verification_input.prior_attempts = 3;
    verification_input.prior_failures = 2;

    let recommendation = recommend_verification(&verification_input);

    assert_eq!(
        recommendation.decision,
        VerificationDecisionKind::AskForApproval
    );
    assert!(recommendation.selected_check_ids.is_empty());
    assert!(
        recommendation
            .rationale
            .iter()
            .any(|reason| reason.contains("failed"))
    );
}

#[test]
fn missing_history_returns_unknown_not_false_confidence() {
    let verification_input = input(vec![check(
        "ui-targeted",
        "Work surface browser test",
        "targeted",
        false,
        None,
        &["apps/desktop/src/features/work/WorkSurface.tsx"],
    )]);
    let recommendation = recommend_verification(&verification_input);

    assert_eq!(recommendation.uncertainty_label, "unknown");
    assert!(
        recommendation
            .rationale
            .iter()
            .any(|reason| reason.contains("history"))
    );
}

#[test]
fn same_input_and_policy_produce_same_recommendation() {
    let verification_input = input(vec![
        check(
            "ui-targeted",
            "Work surface browser test",
            "targeted",
            false,
            Some(20),
            &["apps/desktop/src/features/work/WorkSurface.tsx"],
        ),
        check(
            "full-suite",
            "Full suite",
            "suite",
            false,
            Some(600),
            &["crates/"],
        ),
    ]);

    assert_eq!(
        recommend_verification(&verification_input),
        recommend_verification(&verification_input)
    );
}
