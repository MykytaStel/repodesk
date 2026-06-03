//! Capture memory candidates from an AI response.
//!
//! "Merging between AI": paste back what Codex/ChatGPT/Gemini/Ollama produced,
//! and this extracts durable notes (decisions, constraints, risks, patterns)
//! attributed to that agent. It writes **proposals**, never entries directly —
//! a human accepts them via the review queue.
//!
//! Extraction is deterministic (headings + prefixes + keyworded bullets), with
//! an optional Ollama extractor ([`capture_from_text_smart`]) that falls back to
//! the heuristics when Ollama is unavailable.

use std::collections::HashSet;

use crate::errors::RepoDeskResult;

use super::llm::BrainLlm;
use super::model::{MemoryProposal, NewProposal, ProposedEntry, compute_content_hash, source};
use super::store;

const MAX_CAPTURE_CANDIDATES: usize = 25;
const MIN_CONTENT_LEN: usize = 12;

/// Deterministic capture: extract candidates with heuristics and persist them
/// as pending capture proposals.
pub fn capture_from_text(
    project: &str,
    task_id: &str,
    agent: &str,
    text: &str,
) -> RepoDeskResult<Vec<MemoryProposal>> {
    let candidates = extract_candidates(text)
        .into_iter()
        .map(|c| (c.content, c.category))
        .collect();
    persist_candidates(project, task_id, agent, candidates)
}

/// Hybrid capture: use the Ollama extractor when available, otherwise fall back
/// to the deterministic heuristics.
pub async fn capture_from_text_smart(
    project: &str,
    task_id: &str,
    agent: &str,
    text: &str,
    llm: &BrainLlm,
) -> RepoDeskResult<Vec<MemoryProposal>> {
    match llm.extract(text).await {
        Some(candidates) => persist_candidates(project, task_id, agent, candidates),
        None => capture_from_text(project, task_id, agent, text),
    }
}

/// Persist `(content, category)` candidates as pending capture proposals,
/// skipping anything that duplicates an existing active entry (by content hash)
/// or repeats within this batch.
fn persist_candidates(
    project: &str,
    task_id: &str,
    agent: &str,
    candidates: Vec<(String, String)>,
) -> RepoDeskResult<Vec<MemoryProposal>> {
    // Existing active content hashes, so we don't propose obvious duplicates.
    let mut seen: HashSet<String> = store::list_active(project)?
        .iter()
        .map(|e| e.content_hash.clone())
        .collect();

    let mut created = Vec::new();
    for (content, category) in candidates.into_iter().take(MAX_CAPTURE_CANDIDATES) {
        let content = content.trim().to_string();
        if content.chars().count() < MIN_CONTENT_LEN {
            continue;
        }
        let hash = compute_content_hash(&content);
        if !seen.insert(hash) {
            continue; // duplicate of an existing entry or an earlier candidate
        }
        let proposed = ProposedEntry {
            content,
            category,
            tags: Vec::new(),
            source: source::AI.to_string(),
            agent: agent.to_string(),
        };
        let proposal =
            store::add_proposal(NewProposal::capture(project, task_id, agent, proposed))?;
        created.push(proposal);
    }

    Ok(created)
}

#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    content: String,
    category: String,
}

/// Pure extraction — no IO. Exposed for tests.
fn extract_candidates(text: &str) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut current_heading_category: Option<&'static str> = None;
    let mut in_code_fence = false;

    for raw in text.lines() {
        let line = raw.trim();

        if line.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence || line.is_empty() {
            continue;
        }

        // Headings set the category context for following bullets.
        if let Some(rest) = line.strip_prefix('#') {
            current_heading_category = category_from_keywords(rest);
            continue;
        }

        // `Decision: ...`, `Risk - ...` style lines.
        if let Some((category, content)) = split_labeled_line(line) {
            push_candidate(&mut out, content, category);
            continue;
        }

        // Bullets: capture when under a recognized heading or carrying a keyword.
        if let Some(content) = strip_bullet(line) {
            let category = current_heading_category.or_else(|| category_from_keywords(content));
            if let Some(category) = category {
                push_candidate(&mut out, content, category);
            }
        }
    }

    out
}

fn push_candidate(out: &mut Vec<Candidate>, content: &str, category: &str) {
    let content = content.trim().trim_end_matches(['.', ';']).trim();
    if content.chars().count() < MIN_CONTENT_LEN {
        return;
    }
    let candidate = Candidate {
        content: content.to_string(),
        category: category.to_string(),
    };
    if !out.contains(&candidate) {
        out.push(candidate);
    }
}

fn strip_bullet(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some(rest.trim());
        }
    }
    // Numbered bullets: "1. ", "2) "
    let mut chars = line.char_indices();
    if let Some((_, first)) = chars.next()
        && first.is_ascii_digit()
        && let Some(rest) = line
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .strip_prefix(". ")
            .or_else(|| {
                line.trim_start_matches(|c: char| c.is_ascii_digit())
                    .strip_prefix(") ")
            })
    {
        return Some(rest.trim());
    }
    None
}

/// `Decision: use X` → ("decision", "use X"). Only matches a known label.
fn split_labeled_line(line: &str) -> Option<(&'static str, &str)> {
    let stripped = strip_bullet(line).unwrap_or(line);
    let (label, rest) = stripped
        .split_once(':')
        .or_else(|| stripped.split_once(" - "))?;
    let category = category_from_label(label.trim())?;
    Some((category, rest.trim()))
}

fn category_from_label(label: &str) -> Option<&'static str> {
    let l = label.to_lowercase();
    if l.len() > 24 {
        return None; // not a short label, probably prose with a colon
    }
    category_from_keywords(&l)
}

/// Map free text (a heading or label) to a memory category.
fn category_from_keywords(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    if t.contains("decision") || t.contains("decide") || t.contains("chose") || t.contains("choose")
    {
        Some("decision")
    } else if t.contains("constraint")
        || t.contains("guardrail")
        || t.contains("must not")
        || t.contains("never")
        || t.contains("do not")
        || t.contains("don't")
        || t.contains("requirement")
    {
        Some("constraint")
    } else if t.contains("risk")
        || t.contains("danger")
        || t.contains("warning")
        || t.contains("caveat")
    {
        Some("risk")
    } else if t.contains("pattern") || t.contains("convention") || t.contains("approach") {
        Some("pattern")
    } else if t.contains("assumption") || t.contains("context") || t.contains("note") {
        Some("context")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_labeled_and_headed_candidates() {
        let text = r#"
Here is my analysis.

## Risks
- The auth token refresh can race under load
- logging is fine

## Decisions
- Use SQLite for local state

Constraint: Never send full repository to paid agents

```rust
// this should be ignored
let x = "Risk: ignored inside code fence";
```
"#;
        let got = extract_candidates(text);
        let contents: Vec<&str> = got.iter().map(|c| c.content.as_str()).collect();

        assert!(contents.iter().any(|c| c.contains("auth token refresh")));
        assert!(
            contents
                .iter()
                .any(|c| c.contains("Use SQLite for local state"))
        );
        assert!(
            contents
                .iter()
                .any(|c| c.contains("Never send full repository"))
        );
        // Code-fence content is ignored.
        assert!(
            !contents
                .iter()
                .any(|c| c.contains("ignored inside code fence"))
        );
        // A neutral bullet under "Risks" is still captured (heading context),
        // but a too-short/neutral one outside any heading would not be.
        let risk = got
            .iter()
            .find(|c| c.content.contains("auth token refresh"))
            .unwrap();
        assert_eq!(risk.category, "risk");
        let decision = got.iter().find(|c| c.content.contains("SQLite")).unwrap();
        assert_eq!(decision.category, "decision");
        let constraint = got
            .iter()
            .find(|c| c.content.contains("Never send"))
            .unwrap();
        assert_eq!(constraint.category, "constraint");
    }

    #[test]
    fn ignores_plain_prose_without_signal() {
        let text = "This is just a paragraph of prose with no bullets or labels.";
        assert!(extract_candidates(text).is_empty());
    }
}
