use repodesk_core::change_attribution::{
    ChangeAttributionEvidence, ChangeAttributionStrength, aggregate_change_attribution,
    classify_step_attribution,
};
use repodesk_core::change_evidence::ChangeEvidenceStatus;
use repodesk_core::worktree::RunWorktree;

fn managed_worktree(run_id: &str, step_id: &str, base_commit: &str) -> RunWorktree {
    RunWorktree {
        workspace_id: format!("{run_id}-{step_id}-workspace"),
        run_id: run_id.to_string(),
        step_id: step_id.to_string(),
        path: "/tmp/repodesk-managed-worktree".to_string(),
        base_commit: base_commit.to_string(),
        created_at: "2026-08-16T12:00:00Z".to_string(),
        metadata_path: None,
    }
}

#[test]
fn matching_managed_worktree_with_complete_changeset_is_exact_isolated() {
    let workspace = managed_worktree("run-1", "write", "abc123");

    let evidence = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Complete,
        Some(&workspace),
    );

    assert_eq!(evidence.strength, ChangeAttributionStrength::ExactIsolated);
    assert_eq!(
        evidence.workspace_id.as_deref(),
        Some(workspace.workspace_id.as_str())
    );
    assert_eq!(evidence.baseline_commit.as_deref(), Some("abc123"));
    assert!(evidence.strength.is_exact());
}

#[test]
fn mismatched_worktree_identity_never_claims_exact_attribution() {
    let wrong_run = managed_worktree("other-run", "write", "abc123");
    let wrong_step = managed_worktree("run-1", "other-step", "abc123");

    for workspace in [&wrong_run, &wrong_step] {
        let evidence = classify_step_attribution(
            "run-1",
            "write",
            false,
            ChangeEvidenceStatus::Complete,
            Some(workspace),
        );
        assert_ne!(evidence.strength, ChangeAttributionStrength::ExactIsolated);
        assert!(!evidence.strength.is_exact());
    }
}

#[test]
fn missing_baseline_never_claims_exact_attribution() {
    let workspace = managed_worktree("run-1", "write", "   ");

    let evidence = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Complete,
        Some(&workspace),
    );

    assert_ne!(evidence.strength, ChangeAttributionStrength::ExactIsolated);
    assert!(!evidence.strength.is_exact());
}

#[test]
fn complete_non_isolated_capture_is_derived_not_exact() {
    let evidence = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Complete,
        None,
    );

    assert_eq!(evidence.strength, ChangeAttributionStrength::DerivedPrePost);
    assert!(!evidence.strength.is_exact());
}

#[test]
fn unavailable_capture_cannot_be_upgraded_to_derived_or_exact() {
    let workspace = managed_worktree("run-1", "write", "abc123");

    let evidence = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Unavailable,
        Some(&workspace),
    );

    assert_eq!(evidence.strength, ChangeAttributionStrength::Unattributed);
    assert!(!evidence.strength.is_exact());
}

#[test]
fn explicit_manual_handoff_is_manual_attribution() {
    let evidence = classify_step_attribution(
        "run-1",
        "manual",
        true,
        ChangeEvidenceStatus::Complete,
        None,
    );

    assert_eq!(evidence.strength, ChangeAttributionStrength::Manual);
    assert!(!evidence.strength.is_exact());
}

#[test]
fn historical_default_is_conservative_legacy_unknown() {
    let evidence = ChangeAttributionEvidence::default();
    assert_eq!(evidence.strength, ChangeAttributionStrength::LegacyUnknown);
    assert!(!evidence.strength.is_exact());
}

#[test]
fn exact_clean_workspace_is_policy_exact_but_not_emitted_by_current_classifier() {
    assert!(ChangeAttributionStrength::ExactCleanWorkspace.is_exact());

    let evidence = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Complete,
        None,
    );
    assert_ne!(
        evidence.strength,
        ChangeAttributionStrength::ExactCleanWorkspace
    );
}

#[test]
fn multi_writer_aggregation_is_conservative() {
    let first = classify_step_attribution(
        "run-1",
        "write-a",
        false,
        ChangeEvidenceStatus::Complete,
        Some(&managed_worktree("run-1", "write-a", "abc123")),
    );
    let second = classify_step_attribution(
        "run-1",
        "write-b",
        false,
        ChangeEvidenceStatus::Complete,
        Some(&managed_worktree("run-1", "write-b", "abc123")),
    );
    let derived = classify_step_attribution(
        "run-1",
        "write-c",
        false,
        ChangeEvidenceStatus::Complete,
        None,
    );

    let compatible_exact = aggregate_change_attribution(&[first.clone(), second]);
    assert_eq!(
        compatible_exact.strength,
        ChangeAttributionStrength::ExactIsolated
    );
    assert_eq!(compatible_exact.baseline_commit.as_deref(), Some("abc123"));

    let mixed = aggregate_change_attribution(&[first, derived]);
    assert_eq!(mixed.strength, ChangeAttributionStrength::DerivedPrePost);
    assert!(!mixed.strength.is_exact());
}

#[test]
fn mixed_manual_and_agent_producers_are_not_exact() {
    let exact = classify_step_attribution(
        "run-1",
        "write",
        false,
        ChangeEvidenceStatus::Complete,
        Some(&managed_worktree("run-1", "write", "abc123")),
    );
    let manual = classify_step_attribution(
        "run-1",
        "manual",
        true,
        ChangeEvidenceStatus::Complete,
        None,
    );

    let aggregate = aggregate_change_attribution(&[exact, manual]);
    assert!(!aggregate.strength.is_exact());
}
