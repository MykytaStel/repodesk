//! Optional Ollama-assisted "smart" layer for the Memory Brain.
//!
//! Every method degrades gracefully: if Ollama is disabled or unreachable, the
//! call returns `None` and the caller falls back to the deterministic engine.
//! All successful calls are logged to the token ledger so brain upkeep shows up
//! in the Tokens tab.

use crate::api_clients::LlmProvider;
use crate::api_clients::ollama::OllamaClient;
use crate::tokens::estimate_text;
use crate::usage::token_ledger::{LogTokenInput, log_token_event};

/// Valid memory categories the model is allowed to assign.
const CATEGORIES: &[&str] = &[
    "decision",
    "constraint",
    "risk",
    "pattern",
    "context",
    "general",
];

/// Thin wrapper over [`OllamaClient`] scoped to brain operations.
pub struct BrainLlm {
    client: OllamaClient,
    model: String,
    enabled: bool,
}

impl BrainLlm {
    pub fn new(enabled: bool, base_url: Option<String>, model: Option<String>) -> Self {
        let model_name = model.clone().unwrap_or_else(|| "llama3.1".to_string());
        Self {
            client: OllamaClient::new(base_url, model),
            model: model_name,
            enabled,
        }
    }

    /// A wrapper that never calls out (used when Ollama is off).
    pub fn disabled() -> Self {
        Self::new(false, None, None)
    }

    /// True only when enabled *and* the endpoint answers.
    pub async fn available(&self) -> bool {
        self.enabled && self.client.is_available().await
    }

    async fn run(&self, category: &str, prompt: &str) -> Option<String> {
        let output = self.client.generate(prompt).await.ok()?;
        // Best-effort cost accounting; ignore ledger failures.
        let _ = log_token_event(LogTokenInput {
            agent: "ollama".to_string(),
            model: Some(self.model.clone()),
            input_tokens: estimate_text(prompt).estimated_tokens,
            output_tokens: estimate_text(&output).estimated_tokens,
            category: category.to_string(),
            notes: Some("memory brain".to_string()),
        });
        Some(output)
    }

    /// Extract `(content, category)` candidates from an AI response. Returns
    /// `None` (caller falls back) on unavailability or parse failure.
    pub async fn extract(&self, text: &str) -> Option<Vec<(String, String)>> {
        if !self.available().await {
            return None;
        }
        let output = self.run("memory-extract", &extract_prompt(text)).await?;
        let parsed = parse_candidates_json(&output);
        if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        }
    }

    /// Produce a single reconciled note from two conflicting entries.
    pub async fn reconcile(&self, a: &str, b: &str) -> Option<String> {
        if !self.available().await {
            return None;
        }
        let output = self.run("memory-merge", &reconcile_prompt(a, b)).await?;
        let cleaned = clean_line(&output);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    /// Tighten a consolidated `memory.md` body into compact prose.
    pub async fn summarize(&self, markdown: &str) -> Option<String> {
        if !self.available().await {
            return None;
        }
        let output = self
            .run("memory-consolidate", &summarize_prompt(markdown))
            .await?;
        let trimmed = output.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

fn extract_prompt(text: &str) -> String {
    format!(
        "You extract durable project memory from an AI assistant's response.\n\
         Return ONLY a JSON array. Each item: {{\"content\": string, \"category\": string}}.\n\
         category must be one of: decision, constraint, risk, pattern, context.\n\
         Keep each content to one concise sentence. Omit chit-chat, code, and restated questions.\n\
         If nothing durable is present, return [].\n\n\
         RESPONSE TO ANALYZE:\n{text}\n\n\
         JSON:"
    )
}

fn reconcile_prompt(a: &str, b: &str) -> String {
    format!(
        "Two project-memory notes appear to conflict. Write ONE reconciled note (a single \
         sentence) that resolves the contradiction or states the most defensible version. \
         Return only the sentence.\n\nNOTE A: {a}\nNOTE B: {b}\n\nRECONCILED:"
    )
}

fn summarize_prompt(markdown: &str) -> String {
    format!(
        "Rewrite the following project memory as a tight, de-duplicated markdown summary, \
         preserving every decision, constraint, and risk. Do not invent content.\n\n{markdown}\n\nSUMMARY:"
    )
}

/// Parse a model response into `(content, category)` pairs. Tolerates code
/// fences and surrounding prose by extracting the outermost JSON array.
pub fn parse_candidates_json(output: &str) -> Vec<(String, String)> {
    let Some(json) = extract_json_array(output) else {
        return Vec::new();
    };

    #[derive(serde::Deserialize)]
    struct Item {
        content: String,
        #[serde(default)]
        category: String,
    }

    let items: Vec<Item> = serde_json::from_str(&json).unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            let content = item.content.trim().to_string();
            if content.is_empty() {
                return None;
            }
            Some((content, normalize_category(&item.category)))
        })
        .collect()
}

fn extract_json_array(output: &str) -> Option<String> {
    let start = output.find('[')?;
    let end = output.rfind(']')?;
    if end > start {
        Some(output[start..=end].to_string())
    } else {
        None
    }
}

fn normalize_category(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    if CATEGORIES.contains(&lower.as_str()) {
        lower
    } else {
        "context".to_string()
    }
}

fn clean_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_json_array() {
        let out = r#"[{"content":"Use SQLite","category":"decision"},
                      {"content":"Never expose secrets","category":"constraint"}]"#;
        let got = parse_candidates_json(out);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("Use SQLite".to_string(), "decision".to_string()));
        assert_eq!(got[1].1, "constraint");
    }

    #[test]
    fn tolerates_code_fence_and_prose() {
        let out = "Here you go:\n```json\n[{\"content\":\"Rotate tokens\",\"category\":\"weird\"}]\n```\nDone.";
        let got = parse_candidates_json(out);
        assert_eq!(got.len(), 1);
        // Unknown category falls back to "context".
        assert_eq!(got[0].1, "context");
    }

    #[test]
    fn empty_or_garbage_yields_nothing() {
        assert!(parse_candidates_json("no json here").is_empty());
        assert!(parse_candidates_json("[]").is_empty());
        assert!(parse_candidates_json("[{\"content\":\"  \"}]").is_empty());
    }

    #[test]
    fn clean_line_strips_quotes_and_blanks() {
        assert_eq!(
            clean_line("\n  \"Tokens rotate every 24h\"  \n"),
            "Tokens rotate every 24h"
        );
    }
}
