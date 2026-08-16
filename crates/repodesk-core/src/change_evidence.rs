use serde::{Deserialize, Serialize};

/// Provenance quality for a step's captured repository changes.
///
/// `Complete` means an empty changeset is affirmative evidence that no tracked
/// paths changed. `Unavailable` means capture was attempted but failed.
/// `LegacyUnknown` keeps historical receipts/runs loadable without silently
/// upgrading missing provenance into trustworthy evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeEvidenceStatus {
    Complete,
    Unavailable,
    #[default]
    LegacyUnknown,
}

impl ChangeEvidenceStatus {
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
}
