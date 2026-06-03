//! Deterministic ranking + budgeted selection of memory for a task.
//!
//! This is the link that was missing: it turns the stored brain into a compact
//! "memory slice" that gets injected into every agent prompt (via `context.rs`
//! and `smart_context.rs`). Ranking is a pure function (easy to test); the
//! `memory_slice` wrapper does the IO.

use std::collections::{BTreeMap, HashSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::tokens::estimate_text;

use super::model::MemoryEntry;
use super::store;

// ── Scoring weights (tunable) ────────────────────────────────────────────────
const PINNED_BOOST: f64 = 10_000.0;
const SALIENCE_WEIGHT: f64 = 100.0;
const CONFIDENCE_WEIGHT: f64 = 20.0;
const KEYWORD_WEIGHT: f64 = 25.0;
const MAX_KEYWORD_OVERLAP: usize = 6;
const RECENCY_WEIGHT: f64 = 40.0;
const RECENCY_HALFLIFE_DAYS: f64 = 30.0;
const MAX_LINE_CHARS: usize = 240;

/// Keywords distilled from the active task, used for relevance overlap.
#[derive(Debug, Clone, Default)]
pub struct TaskSignals {
    pub keywords: HashSet<String>,
}

impl TaskSignals {
    pub fn from_text(text: &str) -> Self {
        Self {
            keywords: tokenize(text),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }
}

/// An entry with its computed relevance score.
#[derive(Debug, Clone)]
pub struct ScoredEntry {
    pub entry: MemoryEntry,
    pub score: f64,
    pub reasons: Vec<String>,
}

/// Result of selecting a budgeted slice of memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceRender {
    pub markdown: String,
    pub estimated_tokens: usize,
    pub included_ids: Vec<i64>,
    pub excluded_ids: Vec<i64>,
    pub total_active: usize,
}

impl SliceRender {
    pub fn is_empty(&self) -> bool {
        self.included_ids.is_empty()
    }
}

/// Rank active entries by relevance to the task. Pure + deterministic.
pub fn rank_for_task(signals: &TaskSignals, entries: &[MemoryEntry]) -> Vec<ScoredEntry> {
    let now = Utc::now();
    let mut scored: Vec<ScoredEntry> = entries
        .iter()
        .filter(|e| e.is_active())
        .map(|entry| {
            let mut score = 0.0;
            let mut reasons = Vec::new();

            let salience = entry.salience.clamp(0.0, 1.0);
            score += salience * SALIENCE_WEIGHT;

            score += entry.confidence.clamp(0.0, 1.0) * CONFIDENCE_WEIGHT;

            let days = (now - entry.timestamp).num_days().max(0) as f64;
            let recency = RECENCY_WEIGHT * (RECENCY_HALFLIFE_DAYS / (RECENCY_HALFLIFE_DAYS + days));
            score += recency;

            if !signals.is_empty() {
                let overlap = keyword_overlap(signals, entry).min(MAX_KEYWORD_OVERLAP);
                if overlap > 0 {
                    score += overlap as f64 * KEYWORD_WEIGHT;
                    reasons.push(format!("{overlap} task-keyword match(es)"));
                }
            }

            if entry.pinned {
                score += PINNED_BOOST;
                reasons.push("pinned".to_string());
            }

            ScoredEntry {
                entry: entry.clone(),
                score,
                reasons,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.entry.timestamp.cmp(&a.entry.timestamp))
            .then_with(|| b.entry.id.cmp(&a.entry.id))
    });

    scored
}

/// Greedily select ranked entries under a token budget and render the slice.
/// Pinned entries are always included (they bypass the budget) so durable
/// constraints are never silently dropped.
pub fn render_slice(scored: &[ScoredEntry], token_budget: usize) -> SliceRender {
    let total_active = scored.len();
    let mut running = HEADER_TOKEN_OVERHEAD;
    let mut included: Vec<&MemoryEntry> = Vec::new();
    let mut included_ids = Vec::new();
    let mut excluded_ids = Vec::new();

    for s in scored {
        let line = render_line(&s.entry);
        let line_tokens = estimate_text(&line).estimated_tokens;
        if s.entry.pinned || running + line_tokens <= token_budget {
            running += line_tokens;
            included.push(&s.entry);
            included_ids.push(s.entry.id);
        } else {
            excluded_ids.push(s.entry.id);
        }
    }

    let markdown = render_markdown(&included);
    let estimated_tokens = estimate_text(&markdown).estimated_tokens;

    SliceRender {
        markdown,
        estimated_tokens,
        included_ids,
        excluded_ids,
        total_active,
    }
}

/// IO wrapper: load active memory for `project`, rank against the active task,
/// and render a budgeted slice. Never fails on a missing/unset task — it just
/// ranks without keyword signals.
pub fn memory_slice(project: &str, token_budget: usize) -> RepoDeskResult<SliceRender> {
    let entries = store::list_active(project)?;
    let signals = active_task_signals();
    let scored = rank_for_task(&signals, &entries);
    Ok(render_slice(&scored, token_budget))
}

/// Convenience for context builders that only want the markdown body.
pub fn memory_slice_markdown(project: &str, token_budget: usize) -> RepoDeskResult<String> {
    Ok(memory_slice(project, token_budget)?.markdown)
}

/// Build task signals from the active task (title + task.md), best-effort.
fn active_task_signals() -> TaskSignals {
    let Ok(task) = crate::tasks::show_active_task() else {
        return TaskSignals::default();
    };
    let mut text = task.config.title.clone();
    if let Ok(md) = std::fs::read_to_string(&task.task_markdown_file) {
        text.push(' ');
        text.push_str(&md.chars().take(4_000).collect::<String>());
    }
    TaskSignals::from_text(&text)
}

const HEADER_TOKEN_OVERHEAD: usize = 40;

fn render_markdown(entries: &[&MemoryEntry]) -> String {
    if entries.is_empty() {
        return "No project memory recorded yet. Add decisions, constraints, and risks so every \
                agent shares the same context."
            .to_string();
    }

    let mut by_category: BTreeMap<String, Vec<&MemoryEntry>> = BTreeMap::new();
    for entry in entries {
        by_category
            .entry(entry.category.clone())
            .or_default()
            .push(entry);
    }

    let mut out = String::from(
        "_Curated project memory (RepoDesk Memory Brain): durable decisions, constraints, risks, \
         and patterns shared across all agents. This is not the full repository._\n",
    );

    for (category, items) in by_category {
        out.push_str(&format!("\n**{}**\n", title_case(&category)));
        for entry in items {
            out.push_str(&render_line(entry));
            out.push('\n');
        }
    }

    out
}

fn render_line(entry: &MemoryEntry) -> String {
    let pin = if entry.pinned { " (pinned)" } else { "" };
    format!(
        "- [{}] {}{}",
        entry.provenance(),
        one_line(&entry.content),
        pin
    )
}

fn one_line(content: &str) -> String {
    let collapsed = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_LINE_CHARS {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(MAX_LINE_CHARS).collect();
        format!("{truncated}…")
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn keyword_overlap(signals: &TaskSignals, entry: &MemoryEntry) -> usize {
    let mut entry_tokens = tokenize(&entry.content);
    for tag in &entry.tags {
        entry_tokens.insert(tag.to_lowercase());
    }
    entry_tokens
        .iter()
        .filter(|t| signals.keywords.contains(*t))
        .count()
}

/// Lowercase alphanumeric tokens of length >= 3, minus a small stopword set.
fn tokenize(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !is_stopword(w))
        .collect()
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "and"
            | "for"
            | "with"
            | "this"
            | "that"
            | "from"
            | "into"
            | "are"
            | "was"
            | "but"
            | "not"
            | "use"
            | "add"
            | "all"
            | "any"
            | "can"
            | "will"
            | "should"
            | "must"
            | "task"
            | "code"
            | "file"
            | "files"
            | "change"
            | "changes"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::{NewMemoryInput, status};
    use chrono::Duration;

    fn entry(id: i64, content: &str, category: &str, tags: &[&str]) -> MemoryEntry {
        let mut e = MemoryEntry {
            id,
            timestamp: Utc::now(),
            project: "demo".into(),
            content: content.into(),
            category: category.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            source: "human".into(),
            agent: String::new(),
            task_id: String::new(),
            status: status::ACTIVE.into(),
            pinned: false,
            salience: 0.5,
            confidence: 1.0,
            supersedes_id: None,
            content_hash: String::new(),
            updated_at: None,
        };
        e.timestamp = Utc::now();
        e
    }

    #[test]
    fn pinned_outranks_everything() {
        let mut pinned = entry(1, "obscure note", "general", &[]);
        pinned.pinned = true;
        pinned.salience = 0.0;
        let relevant = entry(2, "auth payments api design", "decision", &["auth"]);
        let signals = TaskSignals::from_text("improve auth payments flow");

        let ranked = rank_for_task(&signals, &[relevant, pinned]);
        assert_eq!(ranked[0].entry.id, 1, "pinned entry must rank first");
    }

    #[test]
    fn keyword_overlap_boosts_relevant_entries() {
        let relevant = entry(1, "decision about auth tokens", "decision", &["auth"]);
        let irrelevant = entry(2, "note about logging colors", "general", &["ui"]);
        let signals = TaskSignals::from_text("rework the auth tokens module");

        let ranked = rank_for_task(&signals, &[irrelevant, relevant]);
        assert_eq!(ranked[0].entry.id, 1);
    }

    #[test]
    fn recency_breaks_ties_when_no_signals() {
        let mut older = entry(1, "older note", "general", &[]);
        older.timestamp = Utc::now() - Duration::days(120);
        let newer = entry(2, "newer note", "general", &[]);

        let ranked = rank_for_task(&TaskSignals::default(), &[older, newer]);
        assert_eq!(ranked[0].entry.id, 2);
    }

    #[test]
    fn render_slice_respects_budget_but_keeps_pinned() {
        let signals = TaskSignals::default();
        let mut entries = Vec::new();
        for i in 0..50 {
            entries.push(entry(
                i,
                &format!("note number {i} with some descriptive filler text to cost tokens"),
                "general",
                &[],
            ));
        }
        let mut pinned = entry(999, "critical pinned constraint", "constraint", &[]);
        pinned.pinned = true;
        entries.push(pinned);

        let scored = rank_for_task(&signals, &entries);
        let render = render_slice(&scored, 60);

        assert!(
            render.included_ids.contains(&999),
            "pinned must survive budget"
        );
        assert!(
            !render.excluded_ids.is_empty(),
            "budget should drop some entries"
        );
        assert!(render.included_ids.len() < entries.len());
    }

    #[test]
    fn ignores_non_active_entries() {
        let mut archived = entry(1, "archived note", "general", &[]);
        archived.status = status::ARCHIVED.into();
        let ranked = rank_for_task(&TaskSignals::default(), &[archived]);
        assert!(ranked.is_empty());
    }

    // Keep NewMemoryInput referenced so the import is meaningful in this file's
    // test surface even as the suite grows.
    #[allow(dead_code)]
    fn _input_smoke() -> NewMemoryInput {
        NewMemoryInput::human("demo", "x", "general", &[])
    }
}
