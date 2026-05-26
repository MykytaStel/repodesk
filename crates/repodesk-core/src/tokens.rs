use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEstimate {
    pub characters: usize,
    pub lines: usize,
    pub estimated_tokens: usize,
    pub status: TokenStatus,
    pub breakdown: TokenBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub code_like_chars: usize,
    pub markdown_chars: usize,
    pub prose_chars: usize,
    pub whitespace_chars: usize,
    pub symbol_chars: usize,
    pub cyrillic_chars: usize,
    pub ascii_letters: usize,
    pub digits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    Ok,
    Medium,
    Large,
    TooLarge,
}

impl TokenStatus {
    pub fn as_label(&self) -> &'static str {
        match self {
            TokenStatus::Ok => "OK",
            TokenStatus::Medium => "MEDIUM",
            TokenStatus::Large => "LARGE",
            TokenStatus::TooLarge => "TOO LARGE",
        }
    }

    pub fn recommendation(&self) -> &'static str {
        match self {
            TokenStatus::Ok => "Safe to send as focused context.",
            TokenStatus::Medium => "Acceptable, but keep task scope tight.",
            TokenStatus::Large => "Compress before sending to paid agents.",
            TokenStatus::TooLarge => "Do not send directly. Split or compress first.",
        }
    }
}

pub fn estimate_text(text: &str) -> TokenEstimate {
    let characters = text.chars().count();
    let lines = text.lines().count();

    let breakdown = analyze_breakdown(text);

    let weighted_estimate = estimate_weighted_tokens(&breakdown);
    let conservative_estimate = if characters == 0 {
        0
    } else {
        (characters / 3).max(1)
    };

    let estimated_tokens = weighted_estimate.max(conservative_estimate);

    let status = match estimated_tokens {
        0..=8_000 => TokenStatus::Ok,
        8_001..=12_000 => TokenStatus::Medium,
        12_001..=30_000 => TokenStatus::Large,
        _ => TokenStatus::TooLarge,
    };

    TokenEstimate {
        characters,
        lines,
        estimated_tokens,
        status,
        breakdown,
    }
}

pub fn estimate_file(path: &Path) -> RepoDeskResult<TokenEstimate> {
    let content = fs::read_to_string(path)?;
    Ok(estimate_text(&content))
}

pub fn format_estimate(estimate: &TokenEstimate) -> String {
    format!(
        r#"Characters: {}
Lines: {}
Estimated tokens: {}
Status: {}
Recommendation: {}

Breakdown:
  code-like chars:  {}
  markdown chars:   {}
  prose chars:      {}
  whitespace chars: {}
  symbol chars:     {}
  cyrillic chars:   {}
  ascii letters:    {}
  digits:           {}

Interpretation:
{}
"#,
        estimate.characters,
        estimate.lines,
        estimate.estimated_tokens,
        estimate.status.as_label(),
        estimate.status.recommendation(),
        estimate.breakdown.code_like_chars,
        estimate.breakdown.markdown_chars,
        estimate.breakdown.prose_chars,
        estimate.breakdown.whitespace_chars,
        estimate.breakdown.symbol_chars,
        estimate.breakdown.cyrillic_chars,
        estimate.breakdown.ascii_letters,
        estimate.breakdown.digits,
        explain_breakdown(estimate)
    )
}

fn analyze_breakdown(text: &str) -> TokenBreakdown {
    let mut breakdown = TokenBreakdown {
        code_like_chars: 0,
        markdown_chars: 0,
        prose_chars: 0,
        whitespace_chars: 0,
        symbol_chars: 0,
        cyrillic_chars: 0,
        ascii_letters: 0,
        digits: 0,
    };

    for line in text.lines() {
        let line_len = line.chars().count();

        if is_markdown_line(line) {
            breakdown.markdown_chars += line_len;
        } else if is_code_like_line(line) {
            breakdown.code_like_chars += line_len;
        } else {
            breakdown.prose_chars += line_len;
        }

        for ch in line.chars() {
            if ch.is_whitespace() {
                breakdown.whitespace_chars += 1;
            }

            if ch.is_ascii_alphabetic() {
                breakdown.ascii_letters += 1;
            }

            if ch.is_ascii_digit() {
                breakdown.digits += 1;
            }

            if is_cyrillic(ch) {
                breakdown.cyrillic_chars += 1;
            }

            if is_symbol_heavy(ch) {
                breakdown.symbol_chars += 1;
            }
        }
    }

    breakdown
}

fn estimate_weighted_tokens(breakdown: &TokenBreakdown) -> usize {
    let code_tokens = breakdown.code_like_chars as f64 / 2.7;
    let markdown_tokens = breakdown.markdown_chars as f64 / 3.1;
    let prose_tokens = breakdown.prose_chars as f64 / 3.5;
    let symbol_penalty = breakdown.symbol_chars as f64 / 18.0;
    let cyrillic_penalty = breakdown.cyrillic_chars as f64 / 12.0;

    (code_tokens + markdown_tokens + prose_tokens + symbol_penalty + cyrillic_penalty).ceil()
        as usize
}

fn is_markdown_line(line: &str) -> bool {
    let trimmed = line.trim_start();

    trimmed.starts_with('#')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("> ")
        || trimmed.starts_with("```")
        || trimmed.starts_with("|")
}

fn is_code_like_line(line: &str) -> bool {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return false;
    }

    let code_markers = [
        "fn ",
        "let ",
        "pub ",
        "use ",
        "impl ",
        "struct ",
        "enum ",
        "match ",
        "return ",
        "const ",
        "import ",
        "export ",
        "function ",
        "class ",
        "interface ",
        "type ",
        "async ",
        "await ",
    ];

    if code_markers
        .iter()
        .any(|marker| trimmed.starts_with(marker))
    {
        return true;
    }

    let symbol_count = trimmed
        .chars()
        .filter(|ch| "{}[]();=<>|&$:`".contains(*ch))
        .count();

    symbol_count >= 3
}

fn is_cyrillic(ch: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&ch)
}

fn is_symbol_heavy(ch: char) -> bool {
    "{}[]();=<>|&$:`\\/".contains(ch)
}

fn explain_breakdown(estimate: &TokenEstimate) -> String {
    let mut notes = Vec::new();

    if estimate.breakdown.code_like_chars > estimate.breakdown.prose_chars {
        notes.push("- Code-like content is the largest part. Code/diff usually costs more tokens than plain prose.");
    }

    if estimate.breakdown.markdown_chars > 4_000 {
        notes.push(
            "- Markdown/docs are significant. Long plans and documentation can become expensive.",
        );
    }

    if estimate.breakdown.cyrillic_chars > 2_000 {
        notes.push(
            "- Ukrainian/Russian prose is significant. Keep instructions compact and structured.",
        );
    }

    if estimate.breakdown.symbol_chars > 3_000 {
        notes.push(
            "- Symbol-heavy content is high. This often means code, diffs, stack traces, or logs.",
        );
    }

    if notes.is_empty() {
        notes.push("- No obvious token hotspot detected.");
    }

    notes.join("\n")
}
