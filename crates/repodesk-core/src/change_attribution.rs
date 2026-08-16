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
        let baseline = workspace.base_commit.trim();
        if baseline.is_empty() {
            return evidence(
                ChangeAttributionStrength::Unattributed,
                Some(workspace.workspace_id.clone()),
                None,
                "managed worktree baseline commit is missing",
            );
        }
        return evidence(
            ChangeAttributionStrength::ExactIsolated,
            Some(workspace.workspace_id.clone()),
            Some(baseline.to_string()),
            "managed isolated worktree matches run and step identity with complete changeset evidence",
        );
    }

    evidence(
        ChangeAttributionStrength::DerivedPrePost,
        None,
        None,
        "complete pre/post changeset evidence exists without exclusive workspace proof",
    )
}

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
