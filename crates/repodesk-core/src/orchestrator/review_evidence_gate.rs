//! Evidence gate in front of the transactional review boundary.
//!
//! A persisted run file proves that an agent returned, but Review may mutate the
//! active checkout only when the Work-flow execution receipt for that exact run
//! is present and coherent. Missing persistence is a recovery problem, not a
//! reason to re-run the agent or to apply an unbound changeset.

use crate::errors::RepoDeskResult;

use super::execution_evidence::require_review_evidence_ready;
use super::review::{ReviewAction, RunReview};
use super::review_transaction;

pub fn review_run(run_id: &str, action: ReviewAction) -> RepoDeskResult<RunReview> {
    require_review_evidence_ready(run_id)?;
    review_transaction::review_run(run_id, action)
}
