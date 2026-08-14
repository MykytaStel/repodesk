//! The Memory Brain: a single shared, ranked, provenance-aware memory that is
//! captured from every agent and injected back into every agent's context.
//!
//! Layout:
//! - [`model`]      — the [`MemoryEntry`] data model + provenance helpers.
//! - [`store`]      — SQLite CRUD (add/list/get/update/delete/pin/status/search).
//! - [`retrieval`]  — deterministic ranking + budgeted "memory slice" for prompts.
//! - [`context_source`] — fail-closed policy for structured vs legacy context material.
//! - [`consolidate`]— render the active brain to a human-readable `memory.md`.
//!
//! - [`capture`]    — extract memory candidates from an AI response (proposals).
//! - [`merge`]      — dedup/near-dup/conflict detection + accept/reject (the only
//!   path that mutates the brain from automated suggestions).

pub mod capture;
pub mod consolidate;
pub mod context_source;
pub mod llm;
pub mod merge;
pub mod model;
pub mod retrieval;
pub mod store;

#[cfg(test)]
pub(crate) mod test_support;

pub use capture::{capture_from_text, capture_from_text_smart};
pub use consolidate::consolidate_project_memory;
pub use context_source::{MemoryContextSource, context_source_for_slice};
pub use llm::BrainLlm;
pub use merge::{ScanSummary, accept_proposal, reconcile_conflict, reject_proposal, scan};
pub use model::{
    MemoryEntry, MemoryProposal, NewMemoryInput, NewProposal, ProposalPayload, ProposedEntry,
    compute_content_hash,
};
pub use retrieval::{SliceRender, memory_slice, memory_slice_markdown};
pub use store::{add_memory, list_memory, list_proposals};
