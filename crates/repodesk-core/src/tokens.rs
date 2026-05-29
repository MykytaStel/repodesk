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
    pub language: String,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageCostModel {
    pub language: String,
    pub chars_per_token: f64,
    pub symbol_divisor: f64,
    pub description: String,
}

pub fn get_language_model(lang: &str) -> LanguageCostModel {
    match lang.to_lowercase().as_str() {
        "rust" => LanguageCostModel {
            language: "Rust".to_string(),
            chars_per_token: 2.4,
            symbol_divisor: 12.0,
            description: "Rust tokenization is symbol-heavy (generics, lifetimes, scopes), making it token-dense.".to_string(),
        },
        "python" => LanguageCostModel {
            language: "Python".to_string(),
            chars_per_token: 3.3,
            symbol_divisor: 24.0,
            description: "Python is block-structured with fewer symbols, tokenizing very efficiently.".to_string(),
        },
        "javascript" | "typescript" | "js" | "ts" => LanguageCostModel {
            language: "JavaScript/TypeScript".to_string(),
            chars_per_token: 2.8,
            symbol_divisor: 16.0,
            description: "JS/TS has moderate symbols and keyword-heavy patterns, tokenizing moderately.".to_string(),
        },
        "go" => LanguageCostModel {
            language: "Go".to_string(),
            chars_per_token: 3.0,
            symbol_divisor: 18.0,
            description: "Go is syntax-light with keywords and moderate symbols, tokenizing efficiently.".to_string(),
        },
        _ => LanguageCostModel {
            language: "Text/Markdown".to_string(),
            chars_per_token: 3.5,
            symbol_divisor: 20.0,
            description: "Standard text or Markdown tokenization with average English word densities.".to_string(),
        },
    }
}

pub fn detect_language(text: &str) -> String {
    let mut rust_score = 0;
    let mut python_score = 0;
    let mut js_score = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") || trimmed.starts_with("impl ") || trimmed.starts_with("use crate::") || trimmed.contains("match ") || trimmed.contains("mut ") {
            rust_score += 3;
        }
        if trimmed.starts_with("def ") || trimmed.starts_with("import ") || trimmed.contains("elif ") || trimmed.ends_with(":") {
            python_score += 2;
        }
        if trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("function ") || trimmed.contains("console.log") || trimmed.contains("=>") {
            js_score += 2;
        }
    }

    if rust_score > python_score && rust_score > js_score {
        "rust".to_string()
    } else if python_score > rust_score && python_score > js_score {
        "python".to_string()
    } else if js_score > rust_score && js_score > python_score {
        "javascript".to_string()
    } else {
        "text".to_string()
    }
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
    estimate_text_for_language(text, None)
}

pub fn estimate_text_for_language(text: &str, lang: Option<&str>) -> TokenEstimate {
    let characters = text.chars().count();
    let lines = text.lines().count();
    let breakdown = analyze_breakdown(text);

    let detected_lang = match lang {
        Some(l) => l.to_string(),
        None => detect_language(text),
    };

    let model = get_language_model(&detected_lang);

    let base_tokens = (breakdown.code_like_chars + breakdown.prose_chars + breakdown.markdown_chars) as f64 / model.chars_per_token;
    let symbol_penalty = breakdown.symbol_chars as f64 / model.symbol_divisor;
    let cyrillic_penalty = breakdown.cyrillic_chars as f64 / 12.0;

    let weighted_estimate = (base_tokens + symbol_penalty + cyrillic_penalty).ceil() as usize;
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

    let mut explanation = Vec::new();
    explanation.push(format!(
        "Detected/Selected language model: {} (average ~{:.1} chars/token).",
        model.language, model.chars_per_token
    ));
    explanation.push(model.description.clone());
    
    explanation.push(format!(
        "Base characters ({}) estimated at {:.1} tokens.",
        characters, base_tokens.ceil()
    ));

    if breakdown.symbol_chars > 0 {
        explanation.push(format!(
            "Symbol chars ({}) added {:.1} tokens penalty (divisor = {:.1}). Code features fragment text.",
            breakdown.symbol_chars, symbol_penalty.ceil(), model.symbol_divisor
        ));
    }

    if breakdown.cyrillic_chars > 0 {
        explanation.push(format!(
            "Cyrillic chars ({}) added {:.1} tokens penalty. Cyrillic takes more UTF-8 space.",
            breakdown.cyrillic_chars, cyrillic_penalty.ceil()
        ));
    }

    explanation.push(format!(
        "Final estimation: {} tokens. Status: {}.",
        estimated_tokens, status.as_label()
    ));

    TokenEstimate {
        characters,
        lines,
        estimated_tokens,
        status,
        breakdown,
        language: model.language,
        explanation,
    }
}


pub fn estimate_file(path: &Path) -> RepoDeskResult<TokenEstimate> {
    let content = fs::read_to_string(path)?;
    Ok(estimate_text(&content))
}

pub fn format_estimate(estimate: &TokenEstimate) -> String {
    let mut output = format!(
        r#"Characters: {}
Lines: {}
Estimated tokens: {}
Status: {}
Recommendation: {}
Language: {}

Explanation:
"#,
        estimate.characters,
        estimate.lines,
        estimate.estimated_tokens,
        estimate.status.as_label(),
        estimate.status.recommendation(),
        estimate.language
    );

    for step in &estimate.explanation {
        output.push_str(&format!("  {}\n", step));
    }

    output.push_str(&format!(
        r#"
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
        estimate.breakdown.code_like_chars,
        estimate.breakdown.markdown_chars,
        estimate.breakdown.prose_chars,
        estimate.breakdown.whitespace_chars,
        estimate.breakdown.symbol_chars,
        estimate.breakdown.cyrillic_chars,
        estimate.breakdown.ascii_letters,
        estimate.breakdown.digits,
        explain_breakdown(estimate)
    ));

    output
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxComparison {
    pub snippet: String,
    pub characters: usize,
    pub lines: usize,
    pub estimated_tokens: usize,
    pub code_like_chars: usize,
    pub symbol_chars: usize,
    pub cost_percentage_difference: f64,
    pub language: String,
    pub explanation: Vec<String>,
}

pub fn compare_syntax(snippets: &[String]) -> Vec<SyntaxComparison> {
    if snippets.is_empty() {
        return Vec::new();
    }

    let estimates: Vec<(String, TokenEstimate)> = snippets
        .iter()
        .map(|s| (s.clone(), estimate_text(s)))
        .collect();

    let baseline_tokens = estimates[0].1.estimated_tokens as f64;

    estimates
        .into_iter()
        .map(|(snippet, est)| {
            let diff = if baseline_tokens > 0.0 {
                ((est.estimated_tokens as f64 - baseline_tokens) / baseline_tokens) * 100.0
            } else {
                0.0
            };
            SyntaxComparison {
                snippet,
                characters: est.characters,
                lines: est.lines,
                estimated_tokens: est.estimated_tokens,
                code_like_chars: est.breakdown.code_like_chars,
                symbol_chars: est.breakdown.symbol_chars,
                cost_percentage_difference: diff,
                language: est.language,
                explanation: est.explanation,
            }
        })
        .collect()
}

pub fn format_syntax_comparison(comparisons: &[SyntaxComparison]) -> String {
    let mut output = String::new();
    output.push_str("Syntax / Language Token Comparison:\n\n");
    for (i, comp) in comparisons.iter().enumerate() {
        output.push_str(&format!("Snippet #{}:\n", i + 1));
        output.push_str(&format!("  Content:    {:?}\n", comp.snippet));
        output.push_str(&format!("  Language:   {}\n", comp.language));
        output.push_str(&format!("  Characters: {}\n", comp.characters));
        output.push_str(&format!("  Lines:      {}\n", comp.lines));
        output.push_str(&format!("  Tokens:     {}\n", comp.estimated_tokens));
        output.push_str(&format!("  Code-like:  {}\n", comp.code_like_chars));
        output.push_str(&format!("  Symbols:    {}\n", comp.symbol_chars));
        if i == 0 {
            output.push_str("  Cost Diff:  Baseline (0.0%)\n");
        } else {
            output.push_str(&format!("  Cost Diff:  {:+2.1}%\n", comp.cost_percentage_difference));
        }
        output.push_str("  Why this snippet cost:\n");
        for step in &comp.explanation {
            output.push_str(&format!("    - {}\n", step));
        }
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_syntax() {
        let snippets = vec![
            "fn foo() {}".to_string(),
            "function foo() {}".to_string(),
            "def foo(): pass".to_string(),
        ];
        let comps = compare_syntax(&snippets);
        assert_eq!(comps.len(), 3);
        assert_eq!(comps[0].cost_percentage_difference, 0.0);
        assert_eq!(comps[0].snippet, "fn foo() {}");
        assert_eq!(comps[1].snippet, "function foo() {}");
    }

    #[test]
    fn test_language_detection() {
        let rust_code = "pub fn main() { let mut x = 5; match x { _ => () } }";
        let python_code = "def main():\n    import sys\n    if True:\n        print('hello')";
        let js_code = "const main = () => { console.log('hello'); };";
        let text_code = "This is a simple sentence in plain English text.";

        assert_eq!(detect_language(rust_code), "rust");
        assert_eq!(detect_language(python_code), "python");
        assert_eq!(detect_language(js_code), "javascript");
        assert_eq!(detect_language(text_code), "text");
    }

    #[test]
    fn test_grammar_aware_token_density() {
        // Compare same logical declaration in Rust vs Python
        let rust_est = estimate_text_for_language("pub fn greet(name: &str) -> String { format!(\"Hello, {}!\", name) }", Some("rust"));
        let python_est = estimate_text_for_language("def greet(name: str) -> str:\n    return f\"Hello, {name}!\"", Some("python"));

        assert_eq!(rust_est.language, "Rust");
        assert_eq!(python_est.language, "Python");

        // Verify that explanation has elements
        assert!(!rust_est.explanation.is_empty());
        assert!(!python_est.explanation.is_empty());
    }
}


