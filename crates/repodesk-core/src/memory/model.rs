//! Data model for the Memory Brain.
//!
//! A [`MemoryEntry`] is a single durable note about a project. Beyond the
//! original `content / category / tags`, entries now carry **provenance**
//! (who created them — a human or a specific AI agent), a **lifecycle status**,
//! and **ranking signals** (pinned, salience, confidence) used by retrieval.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where a memory entry came from.
pub mod source {
    pub const HUMAN: &str = "human";
    pub const AI: &str = "ai";
    pub const SYSTEM: &str = "system";
}

/// Lifecycle status of an entry.
pub mod status {
    pub const ACTIVE: &str = "active";
    pub const SUPERSEDED: &str = "superseded";
    pub const ARCHIVED: &str = "archived";
}

/// A single durable memory note.
///
/// New fields use `#[serde(default)]` so any older serialized payloads (or
/// the lighter frontend type) still deserialize cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,

    /// `human` | `ai` | `system`.
    #[serde(default = "default_source")]
    pub source: String,
    /// Which AI produced this (`codex`, `chatgpt`, `gemini`, `ollama`, …) or
    /// empty for human/system entries.
    #[serde(default)]
    pub agent: String,
    /// Task id this entry was captured under, if any.
    #[serde(default)]
    pub task_id: String,
    /// `active` | `superseded` | `archived`.
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub pinned: bool,
    /// Importance, 0.0..=1.0.
    #[serde(default = "default_salience")]
    pub salience: f64,
    /// How much we trust this note, 0.0..=1.0.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    /// If this entry replaces another, the id it supersedes.
    #[serde(default)]
    pub supersedes_id: Option<i64>,
    /// Stable hash of normalized content, used for exact-duplicate detection.
    #[serde(default)]
    pub content_hash: String,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

fn default_source() -> String {
    source::HUMAN.to_string()
}
fn default_status() -> String {
    status::ACTIVE.to_string()
}
fn default_salience() -> f64 {
    0.5
}
fn default_confidence() -> f64 {
    1.0
}

impl MemoryEntry {
    /// Best label for *who* produced this entry (agent name if known).
    pub fn provenance(&self) -> String {
        if self.source == source::AI && !self.agent.is_empty() {
            self.agent.clone()
        } else {
            self.source.clone()
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == status::ACTIVE
    }
}

/// Input for creating a new entry. Use [`NewMemoryInput::human`] for the common
/// case; capture/merge flows fill in `source`, `agent`, and `task_id`.
#[derive(Debug, Clone)]
pub struct NewMemoryInput {
    pub project: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,
    pub source: String,
    pub agent: String,
    pub task_id: String,
    pub salience: f64,
    pub confidence: f64,
    pub status: String,
    pub supersedes_id: Option<i64>,
}

impl NewMemoryInput {
    /// A human-authored entry with sensible defaults.
    pub fn human(project: &str, content: &str, category: &str, tags: &[String]) -> Self {
        Self {
            project: project.to_string(),
            content: content.to_string(),
            category: category.to_string(),
            tags: tags.to_vec(),
            source: source::HUMAN.to_string(),
            agent: String::new(),
            task_id: String::new(),
            salience: default_salience(),
            confidence: default_confidence(),
            status: status::ACTIVE.to_string(),
            supersedes_id: None,
        }
    }

    /// An AI-captured entry attributed to `agent`, scoped to `task_id`.
    pub fn from_ai(
        project: &str,
        agent: &str,
        task_id: &str,
        content: &str,
        category: &str,
        tags: &[String],
    ) -> Self {
        Self {
            source: source::AI.to_string(),
            agent: agent.to_string(),
            task_id: task_id.to_string(),
            ..Self::human(project, content, category, tags)
        }
    }
}

// ── Proposals (the human-approved brain-mutation queue) ──────────────────────

/// Kind of change a [`MemoryProposal`] represents.
pub mod proposal_kind {
    /// A candidate entry extracted from an AI response.
    pub const CAPTURE: &str = "capture";
    /// Two or more entries with identical normalized content.
    pub const DEDUP: &str = "dedup";
    /// Near-duplicate entries that should be combined into one.
    pub const MERGE: &str = "merge";
    /// Entries that appear to contradict each other.
    pub const CONFLICT: &str = "conflict";
}

pub mod proposal_status {
    pub const PENDING: &str = "pending";
    pub const ACCEPTED: &str = "accepted";
    pub const REJECTED: &str = "rejected";
}

/// The new (or merged) entry a proposal would create when accepted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProposedEntry {
    pub content: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub agent: String,
}

/// Structured body of a proposal (stored as JSON in the DB).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProposalPayload {
    /// Human-readable explanation of why this was proposed.
    #[serde(default)]
    pub rationale: String,
    /// For captures: which AI produced the source text.
    #[serde(default)]
    pub agent: String,
    /// Existing entry ids involved (dedup / merge / conflict).
    #[serde(default)]
    pub source_ids: Vec<i64>,
    /// The entry to create on accept (capture / merge / reconciled conflict).
    #[serde(default)]
    pub proposed: Option<ProposedEntry>,
}

/// A pending suggestion to change the brain. Nothing is applied until a human
/// accepts it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProposal {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub project: String,
    pub task_id: String,
    /// One of [`proposal_kind`].
    pub kind: String,
    /// One of [`proposal_status`].
    pub status: String,
    pub payload: ProposalPayload,
    pub applied_entry_id: Option<i64>,
}

/// Input for creating a proposal.
#[derive(Debug, Clone)]
pub struct NewProposal {
    pub project: String,
    pub task_id: String,
    pub kind: String,
    pub payload: ProposalPayload,
}

impl NewProposal {
    pub fn capture(project: &str, task_id: &str, agent: &str, proposed: ProposedEntry) -> Self {
        Self {
            project: project.to_string(),
            task_id: task_id.to_string(),
            kind: proposal_kind::CAPTURE.to_string(),
            payload: ProposalPayload {
                rationale: format!("Captured from {agent} response"),
                agent: agent.to_string(),
                source_ids: Vec::new(),
                proposed: Some(proposed),
            },
        }
    }
}

/// Stable, version-independent hash of an entry's content used for exact-dup
/// detection. Normalizes whitespace and case so trivially different copies
/// collide. `DefaultHasher` is seeded with fixed keys, so this is deterministic
/// across runs.
pub fn compute_content_hash(content: &str) -> String {
    let normalized: String = content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_ignores_whitespace_and_case() {
        let a = compute_content_hash("Use   SQLite\nfor State");
        let b = compute_content_hash("use sqlite for state");
        assert_eq!(a, b);
        assert_ne!(a, compute_content_hash("use postgres for state"));
    }

    #[test]
    fn provenance_prefers_agent_for_ai() {
        let mut entry = sample();
        entry.source = source::AI.to_string();
        entry.agent = "chatgpt".to_string();
        assert_eq!(entry.provenance(), "chatgpt");
        entry.source = source::HUMAN.to_string();
        entry.agent = String::new();
        assert_eq!(entry.provenance(), "human");
    }

    fn sample() -> MemoryEntry {
        MemoryEntry {
            id: 1,
            timestamp: Utc::now(),
            project: "demo".into(),
            content: "note".into(),
            category: "general".into(),
            tags: vec![],
            source: source::HUMAN.into(),
            agent: String::new(),
            task_id: String::new(),
            status: status::ACTIVE.into(),
            pinned: false,
            salience: 0.5,
            confidence: 1.0,
            supersedes_id: None,
            content_hash: String::new(),
            updated_at: None,
        }
    }
}
