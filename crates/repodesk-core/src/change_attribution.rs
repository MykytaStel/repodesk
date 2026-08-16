use serde::{Deserialize, Serialize};

use crate::change_evidence::ChangeEvidenceStatus;
use crate::worktree::RunWorktree;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAttributionStrength {
    ExactIsolated,
    ExactCleanWorkspace,
    DerivedPrePost,
    Manual,
    Unattributed,
    #[default]
    LegacyUnknown,
}

impl ChangeAttributionStrength {
    pub fn is_exact(self) -> bool {
        matches!(self, Self::ExactIsolated | Self::ExactCleanWorkspace)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeAttributionEvidence {
    #[serde(default)]
    pub strength: ChangeAttributionStrength,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub baseline_commit: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Classify attribution only from evidence recorded at the execution boundary.
///
/// `ChangeEvidenceStatus::Complete` proves that the captured path set is
/// complete; it does **not** by itself prove which producer created those
/// changes. Exact attribution therefore requires a managed worktree bound to
/// the same run + step. We keep weaker attribution states in the type for other
/// evidence producers, but this classifier never fabricates them from path
/// completeness alone.
pub fn classify_step_attribution(
    run_id: &str,
    step_id: &str,
    manual: bool,
    change_evidence_status: ChangeEvidenceStatus,
    workspace: Option<&RunWorktree>,
) -> ChangeAttributionEvidence {
    if manual {
        return evidence(
            ChangeAttributionStrength::Manual,
            None,
            None,
            "change entered through an explicit manual handoff",
        );
    }

    if !change_evidence_status.is_complete() {
        return evidence(
            ChangeAttributionStrength::Unattributed,
            None,
            None,
            "complete changeset provenance is unavailable",
        );
    }

    if let Some(workspace) = workspace {
        if workspace.run_id != run_id || workspace.step_id != step_id {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                None,
                None,
                "managed worktree identity does not match the producing run step",
            );
        }
        let workspace_id = workspace.workspace_id.trim();
        if workspace_id.is_empty() {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                None,
                None,
                "managed worktree identity is missing",
            );
        }
        let baseline = workspace.base_commit.trim();
        if baseline.is_empty() {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                Some(workspace_id.to_string()),
                None,
                "managed worktree baseline commit is missing",
            );
        }
        return evidence(
            ChangeAttributionStrength::ExactIsolated,
            Some(workspace_id.to_string()),
            Some(baseline.to_string()),
            "managed isolated worktree matches run and step identity with complete changeset evidence",
        );
    }

    evidence(
        ChangeAttributionStrength::Unattributed,
        None,
        None,
        "complete changeset evidence exists, but no producer boundary proves attribution",
    )
}

/// Aggregate contributing producer evidence with weakest-proof-wins semantics.
///
/// `DerivedPrePost` and `ExactCleanWorkspace` may be supplied by future or
/// external evidence producers only when they have their own mechanical proof;
/// this function merely combines already-classified evidence and never upgrades
/// a weaker contributor.
pub fn aggregate_change_attribution(
    contributors: &[ChangeAttributionEvidence],
) -> ChangeAttributionEvidence {
    let Some(first) = contributors.first() else {
        return ChangeAttributionEvidence::default();
    };

    if contributors
        .iter()
        .any(|item| item.strength == ChangeAttributionStrength::LegacyUnknown)
    {
        return evidence(
            ChangeAttributionStrength::LegacyUnknown,
            None,
            None,
            "one or more producer attribution records predate typed attribution evidence",
        );
    }

    if contributors
        .iter()
        .any(|item| item.strength == ChangeAttributionStrength::Unattributed)
    {
        return evidence(
            ChangeAttributionStrength::Unattributed,
            None,
            None,
            "one or more contributing changes cannot be attributed to a producer",
        );
    }

    let has_manual = contributors
        .iter()
        .any(|item| item.strength == ChangeAttributionStrength::Manual);
    if has_manual {
        if contributors
            .iter()
            .all(|item| item.strength == ChangeAttributionStrength::Manual)
        {
            return evidence(
                ChangeAttributionStrength::Manual,
                None,
                None,
                "all contributing changes entered through manual handoff",
            );
        }
        return evidence(
            ChangeAttributionStrength::Unattributed,
            None,
            None,
            "manual and automated producers contributed to the same changeset",
        );
    }

    if contributors
        .iter()
        .any(|item| item.strength == ChangeAttributionStrength::DerivedPrePost)
    {
        return evidence(
            ChangeAttributionStrength::DerivedPrePost,
            None,
            common_baseline(contributors),
            "the changeset includes producer evidence without exclusive workspace proof",
        );
    }

    if contributors.iter().all(|item| item.strength.is_exact()) {
        let baseline = common_baseline(contributors);
        if baseline.is_none() {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                None,
                None,
                "exact producer records do not share a compatible baseline",
            );
        }

        let strength = if contributors
            .iter()
            .all(|item| item.strength == ChangeAttributionStrength::ExactIsolated)
        {
            ChangeAttributionStrength::ExactIsolated
        } else if contributors
            .iter()
            .all(|item| item.strength == ChangeAttributionStrength::ExactCleanWorkspace)
        {
            ChangeAttributionStrength::ExactCleanWorkspace
        } else {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                None,
                baseline,
                "contributing exact producer records use incompatible attribution mechanisms",
            );
        };

        let workspace_id = if contributors.len() == 1 {
            first.workspace_id.clone()
        } else {
            None
        };
        return evidence(
            strength,
            workspace_id,
            baseline,
            "all contributing producer records carry compatible exact attribution evidence",
        );
    }

    evidence(
        ChangeAttributionStrength::Unattributed,
        None,
        None,
        "producer attribution evidence cannot be combined safely",
    )
}

fn common_baseline(contributors: &[ChangeAttributionEvidence]) -> Option<String> {
    let baseline = contributors.first()?.baseline_commit.as_deref()?.trim();
    if baseline.is_empty() {
        return None;
    }
    contributors
        .iter()
        .all(|item| item.baseline_commit.as_deref().map(str::trim) == Some(baseline))
        .then(|| baseline.to_string())
}

fn evidence(
    strength: ChangeAttributionStrength,
    workspace_id: Option<String>,
    baseline_commit: Option<String>,
    reason: &str,
) -> ChangeAttributionEvidence {
    ChangeAttributionEvidence {
        strength,
        workspace_id,
        baseline_commit,
        reason: Some(reason.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(run_id: &str, step_id: &str) -> RunWorktree {
        RunWorktree {
            workspace_id: "workspace-1".into(),
            run_id: run_id.into(),
            step_id: step_id.into(),
            path: "/private/tmp/should-never-leak".into(),
            base_commit: "abc123".into(),
            created_at: "2026-08-16T00:00:00Z".into(),
            metadata_path: Some("/private/tmp/metadata.json".into()),
        }
    }

    #[test]
    fn exact_isolated_requires_matching_run_step_and_complete_changeset() {
        let workspace = workspace("run-1", "impl");
        let exact = classify_step_attribution(
            "run-1",
            "impl",
            false,
            ChangeEvidenceStatus::Complete,
            Some(&workspace),
        );
        assert_eq!(exact.strength, ChangeAttributionStrength::ExactIsolated);
        assert_eq!(exact.workspace_id.as_deref(), Some("workspace-1"));
        assert_eq!(exact.baseline_commit.as_deref(), Some("abc123"));
        assert!(
            !exact
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains(&workspace.path)
        );

        let mismatched = classify_step_attribution(
            "run-other",
            "impl",
            false,
            ChangeEvidenceStatus::Complete,
            Some(&workspace),
        );
        assert_eq!(mismatched.strength, ChangeAttributionStrength::Unattributed);

        let incomplete = classify_step_attribution(
            "run-1",
            "impl",
            false,
            ChangeEvidenceStatus::Unavailable,
            Some(&workspace),
        );
        assert_eq!(incomplete.strength, ChangeAttributionStrength::Unattributed);
    }

    #[test]
    fn complete_path_capture_without_producer_boundary_stays_unattributed() {
        let attribution =
            classify_step_attribution("run-1", "impl", false, ChangeEvidenceStatus::Complete, None);
        assert_eq!(
            attribution.strength,
            ChangeAttributionStrength::Unattributed
        );
    }

    #[test]
    fn manual_handoff_is_explicit_not_exact() {
        let attribution = classify_step_attribution(
            "manual-1",
            "manual-handoff",
            true,
            ChangeEvidenceStatus::Complete,
            None,
        );
        assert_eq!(attribution.strength, ChangeAttributionStrength::Manual);
        assert!(!attribution.strength.is_exact());
    }

    #[test]
    fn aggregate_uses_weakest_proof_and_keeps_shared_baseline() {
        let exact = ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::ExactIsolated,
            workspace_id: Some("workspace-1".into()),
            baseline_commit: Some("base".into()),
            reason: None,
        };
        let derived = ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::DerivedPrePost,
            workspace_id: None,
            baseline_commit: Some("base".into()),
            reason: None,
        };
        let aggregate = aggregate_change_attribution(&[exact, derived]);
        assert_eq!(
            aggregate.strength,
            ChangeAttributionStrength::DerivedPrePost
        );
        assert_eq!(aggregate.baseline_commit.as_deref(), Some("base"));
    }

    #[test]
    fn mixed_exact_mechanisms_do_not_claim_exact_change_set_attribution() {
        let isolated = ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::ExactIsolated,
            workspace_id: Some("workspace-1".into()),
            baseline_commit: Some("base".into()),
            reason: None,
        };
        let clean = ChangeAttributionEvidence {
            strength: ChangeAttributionStrength::ExactCleanWorkspace,
            workspace_id: None,
            baseline_commit: Some("base".into()),
            reason: None,
        };
        assert_eq!(
            aggregate_change_attribution(&[isolated, clean]).strength,
            ChangeAttributionStrength::Unattributed
        );
    }
}
