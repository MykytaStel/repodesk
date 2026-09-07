//! Deterministic explanation of whether a verification receipt is reusable.
//!
//! Replay is deliberately read-only. It compares the durable receipt with the
//! current ChangeSet/tree facts and never executes a command or infers a green
//! result from missing evidence.

use serde::{Deserialize, Serialize};

use crate::workflow::TaskRunReceipt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationReplayStatus {
    Current,
    Stale,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationReplayReasonCode {
    ExactMatch,
    CurrentFailedEvidence,
    ReceiptMissing,
    VerificationMissing,
    VerificationRunMismatch,
    CurrentHeadUnavailable,
    CurrentIndexTreeUnavailable,
    CurrentChangesetUnavailable,
    CommittedTreeUnavailable,
    HeadChanged,
    IndexTreeChanged,
    ChangesetChanged,
    CommittedTreeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReplay {
    pub status: VerificationReplayStatus,
    pub reason_code: VerificationReplayReasonCode,
    pub reason: String,
    pub verification_id: Option<String>,
    pub run_id: Option<String>,
    pub verified_at: Option<String>,
    pub verified_head_sha: Option<String>,
    pub verified_index_tree_sha: Option<String>,
    pub current_head_sha: Option<String>,
    pub current_index_tree_sha: Option<String>,
    pub verified_changeset_digest: Option<String>,
    pub current_changeset_digest: Option<String>,
    pub command_count: usize,
    pub passed_commands: usize,
    pub failed_commands: usize,
    pub can_rerun: bool,
    pub recommended_action: String,
}

pub struct VerificationReplayInput<'a> {
    pub receipt: Option<&'a TaskRunReceipt>,
    pub verification_id: Option<&'a str>,
    pub current_head_sha: Option<&'a str>,
    pub current_index_tree_sha: Option<&'a str>,
    pub current_changeset_digest: Option<&'a str>,
    pub committed_tree_sha: Option<&'a str>,
}

pub fn derive_verification_replay(input: VerificationReplayInput<'_>) -> VerificationReplay {
    let Some(receipt) = input.receipt else {
        return result(
            VerificationReplayStatus::Missing,
            VerificationReplayReasonCode::ReceiptMissing,
            "No canonical verification receipt exists for this Work Item.",
            input,
            None,
            None,
            None,
            None,
            0,
            true,
            "Run verification to create a canonical receipt.",
        );
    };

    let Some(verification) = receipt.verification.as_ref() else {
        return result(
            VerificationReplayStatus::Missing,
            VerificationReplayReasonCode::VerificationMissing,
            "The canonical run receipt has no verification evidence.",
            input,
            Some(receipt.run_id.clone()),
            None,
            None,
            None,
            0,
            true,
            "Run verification to create a reusable receipt.",
        );
    };

    let counts = command_counts(verification);
    let common = (
        Some(receipt.run_id.clone()),
        Some(verification.head_sha.clone()),
        Some(verification.index_tree_sha.clone()),
        Some(verification.changeset_digest.clone()),
    );

    if verification.run_id != receipt.run_id {
        return result(
            VerificationReplayStatus::Stale,
            VerificationReplayReasonCode::VerificationRunMismatch,
            "Verification evidence belongs to a different execution run.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            true,
            "Run verification for the current Work Item.",
        );
    }

    if receipt.finish.is_some() {
        let Some(committed_tree_sha) = input.committed_tree_sha else {
            return result(
                VerificationReplayStatus::Unavailable,
                VerificationReplayReasonCode::CommittedTreeUnavailable,
                "The committed tree cannot be resolved, so the historical receipt cannot be replayed.",
                input,
                common.0,
                common.1,
                common.2,
                common.3,
                counts.0,
                false,
                "Resolve the committed tree before reusing this evidence.",
            );
        };
        if committed_tree_sha != verification.index_tree_sha {
            return result(
                VerificationReplayStatus::Stale,
                VerificationReplayReasonCode::CommittedTreeChanged,
                "The committed tree no longer matches the tree verified by this receipt.",
                input,
                common.0,
                common.1,
                common.2,
                common.3,
                counts.0,
                false,
                "Do not reuse this receipt; inspect the newer commit and verify again.",
            );
        }
        return result(
            VerificationReplayStatus::Current,
            if verification.success {
                VerificationReplayReasonCode::ExactMatch
            } else {
                VerificationReplayReasonCode::CurrentFailedEvidence
            },
            if verification.success {
                "The committed tree exactly matches the tree covered by verification."
            } else {
                "The committed tree matches, but the receipt contains current failed command evidence."
            },
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            false,
            if verification.success {
                "No rerun required for tree identity."
            } else {
                "Fix the failing command before relying on this change."
            },
        );
    }

    let Some(current_head_sha) = input.current_head_sha else {
        return result(
            VerificationReplayStatus::Unavailable,
            VerificationReplayReasonCode::CurrentHeadUnavailable,
            "The current Git HEAD cannot be resolved, so receipt freshness is unknown.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            false,
            "Resolve the current Git tree before deciding whether to rerun verification.",
        );
    };
    let Some(current_index_tree_sha) = input.current_index_tree_sha else {
        return result(
            VerificationReplayStatus::Unavailable,
            VerificationReplayReasonCode::CurrentIndexTreeUnavailable,
            "The current staged index tree cannot be resolved, so receipt freshness is unknown.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            false,
            "Resolve the current staged tree before deciding whether to rerun verification.",
        );
    };
    let Some(current_changeset_digest) = input.current_changeset_digest else {
        return result(
            VerificationReplayStatus::Unavailable,
            VerificationReplayReasonCode::CurrentChangesetUnavailable,
            "The current ChangeSet identity cannot be resolved, so receipt freshness is unknown.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            false,
            "Resolve the current ChangeSet before deciding whether to rerun verification.",
        );
    };

    if current_head_sha != verification.head_sha {
        return result(
            VerificationReplayStatus::Stale,
            VerificationReplayReasonCode::HeadChanged,
            "HEAD changed after verification, so the receipt no longer proves the current tree.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            true,
            "Verify the current reviewed ChangeSet again.",
        );
    }
    if current_index_tree_sha != verification.index_tree_sha {
        return result(
            VerificationReplayStatus::Stale,
            VerificationReplayReasonCode::IndexTreeChanged,
            "The staged index tree changed after verification, so the receipt is stale.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            true,
            "Review and verify the current staged ChangeSet again.",
        );
    }
    if current_changeset_digest != verification.changeset_digest {
        return result(
            VerificationReplayStatus::Stale,
            VerificationReplayReasonCode::ChangesetChanged,
            "The current ChangeSet path identity changed after verification.",
            input,
            common.0,
            common.1,
            common.2,
            common.3,
            counts.0,
            true,
            "Review the current ChangeSet and verify it again.",
        );
    }

    result(
        VerificationReplayStatus::Current,
        if verification.success {
            VerificationReplayReasonCode::ExactMatch
        } else {
            VerificationReplayReasonCode::CurrentFailedEvidence
        },
        if verification.success {
            "HEAD, staged tree, and ChangeSet identity exactly match the verification receipt."
        } else {
            "HEAD, staged tree, and ChangeSet identity match, but the receipt contains current failed command evidence."
        },
        input,
        common.0,
        common.1,
        common.2,
        common.3,
        counts.0,
        false,
        if verification.success {
            "No rerun required for tree identity."
        } else {
            "Fix the failing command before relying on this change."
        },
    )
}

fn command_counts(verification: &crate::workflow::VerificationReceipt) -> (usize, usize, usize) {
    let command_count = verification.commands.len();
    let passed = verification
        .commands
        .iter()
        .filter(|command| command.success)
        .count();
    (command_count, passed, command_count.saturating_sub(passed))
}

#[allow(clippy::too_many_arguments)]
fn result(
    status: VerificationReplayStatus,
    reason_code: VerificationReplayReasonCode,
    reason: &str,
    input: VerificationReplayInput<'_>,
    run_id: Option<String>,
    verified_head_sha: Option<String>,
    verified_index_tree_sha: Option<String>,
    verified_changeset_digest: Option<String>,
    command_count: usize,
    can_rerun: bool,
    recommended_action: &str,
) -> VerificationReplay {
    let (passed_commands, failed_commands) = input
        .receipt
        .and_then(|receipt| receipt.verification.as_ref())
        .map(|verification| {
            let passed = verification
                .commands
                .iter()
                .filter(|command| command.success)
                .count();
            (passed, verification.commands.len().saturating_sub(passed))
        })
        .unwrap_or((0, 0));
    VerificationReplay {
        status,
        reason_code,
        reason: reason.to_string(),
        verification_id: input.verification_id.map(str::to_string),
        run_id,
        verified_at: input
            .receipt
            .and_then(|receipt| receipt.verification.as_ref())
            .map(|verification| verification.verified_at.clone()),
        verified_head_sha,
        verified_index_tree_sha,
        current_head_sha: input.current_head_sha.map(str::to_string),
        current_index_tree_sha: input.current_index_tree_sha.map(str::to_string),
        verified_changeset_digest,
        current_changeset_digest: input.current_changeset_digest.map(str::to_string),
        command_count,
        passed_commands,
        failed_commands,
        can_rerun,
        recommended_action: recommended_action.to_string(),
    }
}
