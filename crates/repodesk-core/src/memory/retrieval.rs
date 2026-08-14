//! Deterministic ranking + budgeted selection of memory for a task.
//!
//! This is the link that was missing: it turns the stored brain into a compact
//! "memory slice" that gets injected into every agent prompt (via `context.rs`
//! and `smart_context.rs`). The ranking core accepts an explicit clock so the
//! same inputs can be replayed exactly; the `memory_slice` wrapper supplies the
//! current time for normal interactive use.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
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

/// Result of selecting a hard-budgeted slice of memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceRender {
    pub markdown: String,
    pub estimated_tokens: usize,
    pub included_ids: Vec<i64>,
    pub excluded_ids: Vec<i64>,
    /// Pinned entries that could not fit under the hard token ceiling. This is
    /// explicit because silently dropping a durable constraint is materially
    /// different from dropping an ordinary low-ranked memory.
    #[serde(default)]
    pub pinned_overflow_ids: Vec<i64>,
    pub total_active: usize,
    #[serde(default)]
    pub budget_exhausted: bool,
}

impl SliceRender {
    pub fn is_empty(&self) -> bool {
        self.included_ids.is_empty()
    }
}

/// Rank active entries by relevance using an explicit point in time.
///
/// This is the replayable ranking primitive: identical `signals`, `entries`,
/// and `now` produce identical scores and ordering.
pub fn rank_for_task_at(
    signals: &TaskSignals,
    entries: &[MemoryEntry],
    now: DateTime<Utc>,
) -> Vec<ScoredEntry> {
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
            let recency =
                RECENCY_WEIGHT * (RECENCY_HALFLIFE_DAYS / (RECENCY_HALFLIFE_DAYS + days));
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

/// Interactive convenience wrapper. Call [`rank_for_task_at`] when replay or
/// deterministic evidence reconstruction matters.
pub fn rank_for_task(signals: &TaskSignals, entries: &[MemoryEntry]) -> Vec<ScoredEntry> {
    rank_for_task_at(signals, entries, Utc::now())
}

/// Select ranked entries under a **hard** token ceiling and render the slice.
///
/// Pinned entries rank first, but they do not bypass the ceiling. If durable
/// constraints themselves exceed the budget, their ids are reported in
/// `pinned_overflow_ids` so the caller can block or request a larger context
/// instead of silently violating the bounded-context contract.
pub fn render_slice(scored: &[ScoredEntry], token_budget: usize) -> SliceRender {
    let total_active = scored.len();
    let mut running = HEADER_TOKEN_OVERHEAD.min(token_budget);
    let mut included: Vec<&MemoryEntry> = Vec::new();
    let mut included_ids = Vec::new();
    let mut excluded_ids = Vec::new();
    let mut pinned_overflow_ids = Vec::new();

    for scored_entry in scored {
        let line = render_line(&scored_entry.entry);
        let line_tokens = estimate_text(&line).estimated_tokens;
        if running.saturating_add(line_tokens) <= token_budget {
            running = running.saturating_add(line_tokens);
            included.push(&scored_entry.entry);
            included_ids.push(scored_entry.entry.id);
        } else {
            excluded_ids.push(scored_entry.entry.id);
            if scored_entry.entry.pinned {
                pinned_overflow_ids.push(scored_entry.entry.id);
            }
        }
    }

    // The line-level estimate above is intentionally cheap, but category labels
    // and markdown framing also cost tokens. Enforce the contract against the
    // final rendered text and evict the lowest-ranked included entries until it
    // truly fits. Because `included` preserves rank order, `.pop()` removes the
    // least valuable selected entry first.
    let (markdown, estimated_tokens) = loop {
        let rendered = render_markdown(&included);
        let estimate = estimate_text(&rendered).estimated_tokens;
        if estimate <= token_budget {
            break (rendered, estimate);
        }

        let Some(removed) = included.pop() else {
            // Even the empty-state prose is larger than the requested budget.
            // An empty string is the only representation that can uphold a zero
            // or extremely small hard ceiling.
            break (String::new(), 0);
        };
        included_ids.retain(|id| *id != removed.id);
        if !excluded_ids.contains(&removed.id) {
            excluded_ids.push(removed.id);
        }
        if removed.pinned && !pinned_overflow_ids.contains(&removed.id) {
            pinned_overflow_ids.push(removed.id);
        }
    };

    SliceRender {
        markdown,
        estimated_tokens,
        included_ids,
        excluded_ids,
        pinned_overflow_ids,
        total_active,
        budget_exhausted: !scored.is_empty() && (!excluded_ids.is_empty() || estimated_tokens >= token_budget),
    }
}

/// IO wrapper: load active memory for `project`, rank against the active task,
/// and render a budgeted slice. Never fails on a missing/unset task — it just
/// ranks without keyword signals.
pub fn memory_slice(project: &str, token_budget: usize) -> RepoDeskResult<SliceRender> {
    let entries = store::list_active(project)?;
    let signals = active_task_signals();
    let scored = rank_for_task_at(&signals, &entries, Utc::now());
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
    use chrono::{Duration, TimeZone};

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 14, 9, 0, 0).unwrap()
    }

    fn entry_at(
        id: i64,
        content: &str,
        category: &str,
        tags: &[&str],
        timestamp: DateTime<Utc>,
    ) -> MemoryEntry {
        MemoryEntry {
            id,
            timestamp,
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
        }
    }

    fn entry(id: i64, content: &str, category: &str, tags: &[&str]) -> MemoryEntry {
        entry_at(id, content, category, tags, fixed_now())
    }

    #[test]
    fn pinned_outranks_everything() {
        let mut pinned = entry(1, "obscure note", "general", &[]);
        pinned.pinned = true;
        pinned.salience = 0.0;
        let relevant = entry(2, "auth payments api design", "decision", &["auth"]);
        let signals = TaskSignals::from_text("improve auth payments flow");

        let ranked = rank_for_task_at(&signals, &[relevant, pinned], fixed_now());
        assert_eq!(ranked[0].entry.id, 1, "pinned entry must rank first");
    }

    #[test]
    fn keyword_overlap_boosts_relevant_entries() {
        let relevant = entry(1, "decision about auth tokens", "decision", &["auth"]);
        let irrelevant = entry(2, "note about logging colors", "general", &["ui"]);
        let signals = TaskSignals::from_text("rework the auth tokens module");

        let ranked = rank_for_task_at(&signals, &[irrelevant, relevant], fixed_now());
        assert_eq!(ranked[0].entry.id, 1);
    }

    #[test]
    fn recency_breaks_ties_when_no_signals() {
        let now = fixed_now();
        let older = entry_at(1, "older note", "general", &[], now - Duration::days(120));
        let newer = entry_at(2, "newer note", "general", &[], now);

        let ranked = rank_for_task_at(&TaskSignals::default(), &[older, newer], now);
        assert_eq!(ranked[0].entry.id, 2);
    }

    #[test]
    fn ranking_is_replayable_with_explicit_clock() {
        let now = fixed_now();
        let entries = vec![
            entry_at(1, "old auth rule", "constraint", &["auth"], now - Duration::days(90)),
            entry_at(2, "new auth rule", "constraint", &["auth"], now - Duration::days(2)),
        ];
        let signals = TaskSignals::from_text("auth change");

        let first = rank_for_task_at(&signals, &entries, now);
        let second = rank_for_task_at(&signals, &entries, now);

        assert_eq!(
            first.iter().map(|entry| entry.entry.id).collect::<Vec<_>>(),
            second.iter().map(|entry| entry.entry.id).collect::<Vec<_>>()
        );
        assert_eq!(
            first.iter().map(|entry| entry.score).collect::<Vec<_>>(),
            second.iter().map(|entry| entry.score).collect::<Vec<_>>()
        );
    }

    #[test]
    fn render_slice_never_exceeds_hard_budget_even_for_pinned_entries() {
        let mut entries = Vec::new();
        for i in 0..12 {
            let mut pinned = entry(
                i,
                &format!("critical pinned constraint number {i} with enough descriptive text to consume context tokens"),
                "constraint",
                &[],
            );
            pinned.pinned = true;
            entries.push(pinned);
        }

        let scored = rank_for_task_at(&TaskSignals::default(), &entries, fixed_now());
        let render = render_slice(&scored, 80);

        assert!(render.estimated_tokens <= 80);
        assert!(!render.pinned_overflow_ids.is_empty());
        assert!(render.budget_exhausted);
        assert!(render.included_ids.len() < entries.len());
    }

    #[test]
    fn zero_budget_produces_no_memory_payload() {
        let mut pinned = entry(999, "critical pinned constraint", "constraint", &[]);
        pinned.pinned = true;
        let scored = rank_for_task_at(&TaskSignals::default(), &[pinned], fixed_now());
        let render = render_slice(&scored, 0);

        assert!(render.markdown.is_empty());
        assert_eq!(render.estimated_tokens, 0);
        assert_eq!(render.pinned_overflow_ids, vec![999]);
    }

    #[test]
    fn ordinary_budget_drops_low_rank_entries() {
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

        let scored = rank_for_task_at(&TaskSignals::default(), &entries, fixed_now());
        let render = render_slice(&scored, 60);

        assert!(render.included_ids.contains(&999));
        assert!(!render.excluded_ids.is_empty());
        assert!(render.included_ids.len() < entries.len());
        assert!(render.estimated_tokens <= 60);
    }

    #[test]
    fn ignores_non_active_entries() {
        let mut archived = entry(1, "archived note", "general", &[]);
        archived.status = status::ARCHIVED.into();
        let ranked = rank_for_task_at(&TaskSignals::default(), &[archived], fixed_now());
        assert!(ranked.is_empty());
    }

    #[allow(dead_code)]
    fn _input_smoke() -> NewMemoryInput {
        NewMemoryInput::human("demo", "x", "general", &[])
    }
}
