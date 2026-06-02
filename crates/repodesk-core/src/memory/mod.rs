//! The Memory Brain: a single shared, ranked, provenance-aware memory that is
//! captured from every agent and injected back into every agent's context.
//!
//! Layout:
//! - [`model`]      — the [`MemoryEntry`] data model + provenance helpers.
//! - [`store`]      — SQLite CRUD (add/list/get/update/delete/pin/status/search).
//! - [`retrieval`]  — deterministic ranking + budgeted "memory slice" for prompts.
//! - [`consolidate`]— render the active brain to a human-readable `memory.md`.
//!
//! Capture / merge / conflict resolution (the human-approved proposal queue)
//! land in a following phase.

pub mod consolidate;
pub mod model;
pub mod retrieval;
pub mod store;

pub use consolidate::consolidate_project_memory;
pub use model::{MemoryEntry, NewMemoryInput, compute_content_hash};
pub use retrieval::{SliceRender, memory_slice, memory_slice_markdown};
pub use store::{add_memory, list_memory};
