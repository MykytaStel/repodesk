//! Dedup / near-duplicate merge / conflict detection, plus the accept-or-reject
//! path that is the *only* way automated suggestions mutate the brain.
//!
//! Everything here produces **proposals**. A human reviews them and calls
//! [`accept_proposal`] / [`reject_proposal`]. Detection is deterministic
//! (content-hash + token Jaccard + negation polarity); an Ollama-assisted
//! reconciliation pass is layered on in a later phase.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::persistence::event_journal::{LogEventInput, log_event};

use super::llm::BrainLlm;
use super::model::{
    MemoryEntry, MemoryProposal, NewMemoryInput, NewProposal, ProposalPayload, ProposedEntry,
    proposal_kind, proposal_status, source, status,
};
use super::store;

// Similarity thresholds (token Jaccard over content words).
const NEAR_DUP_MERGE: f64 = 0.6;
const CONFLICT_MIN_OVERLAP: f64 = 0.4;

const NEGATIONS: &[&str] = &[
    "not", "never", "no", "dont", "avoid", "without", "cannot", "cant",
];

/// Counts of proposals created by a scan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanSummary {
    pub dedup: usize,
    pub merge: usize,
    pub conflict: usize,
    pub created: Vec<MemoryProposal>,
}

impl ScanSummary {
    pub fn total(&self) -> usize {
        self.dedup + self.merge + self.conflict
    }
}

/// Scan a project's active memory and create dedup/merge/conflict proposals.
/// Skips a candidate when an equivalent pending proposal already exists.
pub fn scan(project: &str) -> RepoDeskResult<ScanSummary> {
    let entries = store::list_active(project)?;
    let existing = store::list_proposals(project, Some(proposal_status::PENDING))?;
    let mut summary = ScanSummary::default();

    // 1. Exact duplicates grouped by content hash.
    let mut by_hash: std::collections::BTreeMap<String, Vec<&MemoryEntry>> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        by_hash
            .entry(entry.content_hash.clone())
            .or_default()
            .push(entry);
    }
    let mut deduped: HashSet<i64> = HashSet::new();
    for group in by_hash.values() {
        if group.len() < 2 {
            continue;
        }
        // Keep the best (pinned, then newest); supersede the rest.
        let mut sorted: Vec<&MemoryEntry> = group.clone();
        sorted.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.timestamp.cmp(&a.timestamp))
        });
        let ids: Vec<i64> = sorted.iter().map(|e| e.id).collect();
        for id in &ids {
            deduped.insert(*id);
        }
        let payload = ProposalPayload {
            rationale: format!("{} entries share identical content; keep one.", ids.len()),
            source_ids: ids.clone(),
            ..Default::default()
        };
        if let Some(p) = create_if_new(project, proposal_kind::DEDUP, payload, &existing)? {
            summary.created.push(p);
            summary.dedup += 1;
        }
    }

    // 2. Pairwise near-dup (merge) and conflict over the remaining entries.
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let a = &entries[i];
            let b = &entries[j];
            if a.content_hash == b.content_hash {
                continue; // handled by dedup
            }
            let sim = jaccard(&word_set(&a.content), &word_set(&b.content));
            if sim < CONFLICT_MIN_OVERLAP {
                continue;
            }

            if polarity_conflict(&a.content, &b.content) {
                let payload = ProposalPayload {
                    rationale: format!(
                        "Possible contradiction between [{}] and [{}].",
                        a.provenance(),
                        b.provenance()
                    ),
                    source_ids: vec![a.id, b.id],
                    ..Default::default()
                };
                if let Some(p) =
                    create_if_new(project, proposal_kind::CONFLICT, payload, &existing)?
                {
                    summary.created.push(p);
                    summary.conflict += 1;
                }
            } else if sim >= NEAR_DUP_MERGE && !deduped.contains(&a.id) && !deduped.contains(&b.id)
            {
                let payload = ProposalPayload {
                    rationale: format!(
                        "Near-duplicate notes ({:.0}% overlap) — combine.",
                        sim * 100.0
                    ),
                    source_ids: vec![a.id, b.id],
                    proposed: Some(merged_entry(a, b)),
                    ..Default::default()
                };
                if let Some(p) = create_if_new(project, proposal_kind::MERGE, payload, &existing)? {
                    summary.created.push(p);
                    summary.merge += 1;
                }
            }
        }
    }

    Ok(summary)
}

fn create_if_new(
    project: &str,
    kind: &str,
    payload: ProposalPayload,
    existing: &[MemoryProposal],
) -> RepoDeskResult<Option<MemoryProposal>> {
    let key = sorted_ids(&payload.source_ids);
    let already = existing
        .iter()
        .any(|p| p.kind == kind && sorted_ids(&p.payload.source_ids) == key);
    if already {
        return Ok(None);
    }
    let proposal = store::add_proposal(NewProposal {
        project: project.to_string(),
        task_id: String::new(),
        kind: kind.to_string(),
        payload,
    })?;
    Ok(Some(proposal))
}

/// Accept a proposal, mutating the brain. `keep_id` selects the surviving entry
/// for dedup/conflict proposals (defaults to the first source for dedup).
pub fn accept_proposal(id: i64, keep_id: Option<i64>) -> RepoDeskResult<MemoryProposal> {
    let proposal = store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} not found")))?;
    if proposal.status != proposal_status::PENDING {
        return Err(RepoDeskError::Database(format!(
            "proposal {id} is not pending (status: {})",
            proposal.status
        )));
    }

    let applied_entry_id = match proposal.kind.as_str() {
        proposal_kind::CAPTURE | proposal_kind::MERGE => {
            let proposed = proposal.payload.proposed.clone().ok_or_else(|| {
                RepoDeskError::Database("proposal has no proposed entry".to_string())
            })?;
            let new = store::add_entry(new_input_from(
                &proposal.project,
                &proposal.task_id,
                &proposed,
            ))?;
            // For merges, retire the sources in favor of the new entry.
            for src in &proposal.payload.source_ids {
                store::mark_superseded(*src, new.id)?;
            }
            new.id
        }
        proposal_kind::DEDUP => {
            let keep = keep_id
                .or_else(|| proposal.payload.source_ids.first().copied())
                .ok_or_else(|| RepoDeskError::Database("dedup has no sources".to_string()))?;
            for src in &proposal.payload.source_ids {
                if *src != keep {
                    store::mark_superseded(*src, keep)?;
                }
            }
            keep
        }
        proposal_kind::CONFLICT => {
            match keep_id {
                // A winner was chosen: supersede the losing entries.
                Some(winner) => {
                    for src in &proposal.payload.source_ids {
                        if *src != winner {
                            store::mark_superseded(*src, winner)?;
                        }
                    }
                    winner
                }
                // No winner, but a reconciled entry was proposed: treat as merge.
                None => {
                    let proposed = proposal.payload.proposed.clone().ok_or_else(|| {
                        RepoDeskError::Database(
                            "conflict resolution requires --keep <entry-id>".to_string(),
                        )
                    })?;
                    let new = store::add_entry(new_input_from(
                        &proposal.project,
                        &proposal.task_id,
                        &proposed,
                    ))?;
                    for src in &proposal.payload.source_ids {
                        store::mark_superseded(*src, new.id)?;
                    }
                    new.id
                }
            }
        }
        other => {
            return Err(RepoDeskError::Database(format!(
                "unknown proposal kind: {other}"
            )));
        }
    };

    store::set_proposal_status(id, proposal_status::ACCEPTED, Some(applied_entry_id))?;
    audit(
        &proposal.project,
        "accepted",
        &proposal.kind,
        id,
        Some(applied_entry_id),
    );

    store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} vanished")))
}

/// Reject a proposal — the brain is left unchanged.
pub fn reject_proposal(id: i64) -> RepoDeskResult<MemoryProposal> {
    let proposal = store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} not found")))?;
    if proposal.status != proposal_status::PENDING {
        return Err(RepoDeskError::Database(format!(
            "proposal {id} is not pending (status: {})",
            proposal.status
        )));
    }
    store::set_proposal_status(id, proposal_status::REJECTED, None)?;
    audit(&proposal.project, "rejected", &proposal.kind, id, None);
    store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} vanished")))
}

/// Ask Ollama to reconcile a conflict's two entries into one note, storing the
/// result as the proposal's `proposed` entry (so it can then be accepted with no
/// `--keep`). Errors when Ollama is unavailable.
pub async fn reconcile_conflict(id: i64, llm: &BrainLlm) -> RepoDeskResult<MemoryProposal> {
    let proposal = store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} not found")))?;
    if proposal.kind != proposal_kind::CONFLICT {
        return Err(RepoDeskError::Database(format!(
            "proposal {id} is not a conflict"
        )));
    }
    let ids = &proposal.payload.source_ids;
    if ids.len() < 2 {
        return Err(RepoDeskError::Database(
            "conflict has fewer than two sources".to_string(),
        ));
    }
    let a = store::get_entry(ids[0])?
        .ok_or_else(|| RepoDeskError::Database(format!("entry {} not found", ids[0])))?;
    let b = store::get_entry(ids[1])?
        .ok_or_else(|| RepoDeskError::Database(format!("entry {} not found", ids[1])))?;

    let reconciled = llm.reconcile(&a.content, &b.content).await.ok_or_else(|| {
        RepoDeskError::Api("Ollama is unavailable or returned no reconciliation".to_string())
    })?;

    let mut payload = proposal.payload.clone();
    payload.proposed = Some(ProposedEntry {
        content: reconciled,
        category: a.category.clone(),
        tags: Vec::new(),
        source: source::SYSTEM.to_string(),
        agent: "ollama".to_string(),
    });
    payload.rationale = format!(
        "Ollama-reconciled from [{}] and [{}]; accept to apply.",
        a.provenance(),
        b.provenance()
    );
    store::update_proposal_payload(id, &payload)?;

    store::get_proposal(id)?
        .ok_or_else(|| RepoDeskError::Database(format!("proposal {id} vanished")))
}

fn new_input_from(project: &str, task_id: &str, proposed: &ProposedEntry) -> NewMemoryInput {
    NewMemoryInput {
        project: project.to_string(),
        content: proposed.content.clone(),
        category: proposed.category.clone(),
        tags: proposed.tags.clone(),
        source: if proposed.source.is_empty() {
            source::AI.to_string()
        } else {
            proposed.source.clone()
        },
        agent: proposed.agent.clone(),
        task_id: task_id.to_string(),
        salience: 0.5,
        confidence: 1.0,
        status: status::ACTIVE.to_string(),
        supersedes_id: None,
    }
}

fn merged_entry(a: &MemoryEntry, b: &MemoryEntry) -> ProposedEntry {
    // Keep the longer content as the base; union tags; prefer a's category.
    let (long, short) = if a.content.len() >= b.content.len() {
        (a, b)
    } else {
        (b, a)
    };
    let mut tags = long.tags.clone();
    for t in &short.tags {
        if !tags.contains(t) {
            tags.push(t.clone());
        }
    }
    let agent = if long.agent.is_empty() {
        short.agent.clone()
    } else {
        long.agent.clone()
    };
    ProposedEntry {
        content: long.content.clone(),
        category: long.category.clone(),
        tags,
        source: source::SYSTEM.to_string(),
        agent,
    }
}

fn audit(project: &str, action: &str, kind: &str, id: i64, applied: Option<i64>) {
    let _ = log_event(LogEventInput {
        module_name: "memory".to_string(),
        level: "info".to_string(),
        message: format!("{action} {kind} proposal {id} for {project}"),
        metadata: vec![
            ("action".to_string(), action.to_string()),
            ("kind".to_string(), kind.to_string()),
            (
                "applied_entry_id".to_string(),
                applied.map(|v| v.to_string()).unwrap_or_default(),
            ),
        ],
    });
}

fn sorted_ids(ids: &[i64]) -> Vec<i64> {
    let mut v = ids.to_vec();
    v.sort_unstable();
    v
}

fn word_set(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 2)
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 { 0.0 } else { inter / union }
}

/// True when the two notes are about the same topic but one negates and the
/// other affirms (a likely contradiction).
fn polarity_conflict(a: &str, b: &str) -> bool {
    let wa = word_set(a);
    let wb = word_set(b);
    if jaccard(&wa, &wb) < CONFLICT_MIN_OVERLAP {
        return false;
    }
    let neg_a = wa.iter().any(|w| NEGATIONS.contains(&w.as_str()));
    let neg_b = wb.iter().any(|w| NEGATIONS.contains(&w.as_str()));
    neg_a != neg_b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::test_support::with_temp_home;

    #[test]
    fn detects_exact_duplicates_and_accept_supersedes() {
        with_temp_home(|| {
            let project = "merge_demo";
            let a = store::add_memory(project, "Use SQLite for state", "decision", &[]).unwrap();
            let b =
                store::add_memory(project, "use   sqlite   for state", "decision", &[]).unwrap();

            let summary = scan(project).unwrap();
            assert_eq!(summary.dedup, 1, "should find one exact-dup group");

            let proposals = store::list_proposals(project, Some("pending")).unwrap();
            let dedup = proposals.iter().find(|p| p.kind == "dedup").unwrap();

            // Accepting is idempotent-ish: re-accepting errors (no longer pending).
            accept_proposal(dedup.id, None).unwrap();
            assert!(accept_proposal(dedup.id, None).is_err());

            // Exactly one of the two remains active.
            let active = store::list_active(project).unwrap();
            assert_eq!(active.len(), 1);
            assert!(active[0].id == a.id || active[0].id == b.id);
        });
    }

    #[test]
    fn reject_leaves_brain_unchanged() {
        with_temp_home(|| {
            let project = "merge_reject";
            store::add_memory(project, "Tokens rotate every 24h", "constraint", &[]).unwrap();
            store::add_memory(project, "Tokens rotate every 24h", "constraint", &[]).unwrap();
            scan(project).unwrap();
            let p = store::list_proposals(project, Some("pending")).unwrap()[0].clone();

            reject_proposal(p.id).unwrap();
            assert_eq!(store::list_active(project).unwrap().len(), 2);
            assert_eq!(store::count_pending(project).unwrap(), 0);
        });
    }

    #[test]
    fn scan_is_idempotent_no_duplicate_proposals() {
        with_temp_home(|| {
            let project = "merge_idem";
            store::add_memory(project, "Always run checks locally", "constraint", &[]).unwrap();
            store::add_memory(project, "always run checks locally", "constraint", &[]).unwrap();
            assert_eq!(scan(project).unwrap().total(), 1);
            // Second scan must not create another proposal for the same group.
            assert_eq!(scan(project).unwrap().total(), 0);
        });
    }

    #[test]
    fn detects_polarity_conflict() {
        assert!(polarity_conflict(
            "Auth tokens must rotate every 24 hours",
            "Auth tokens should not rotate every 24 hours"
        ));
        assert!(!polarity_conflict(
            "Use SQLite for local state",
            "Prefer Postgres for the cloud service"
        ));
    }
}
