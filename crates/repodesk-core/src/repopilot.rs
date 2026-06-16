//! Parsing of RepoPilot `review --format json` output into a structured report
//! the desktop can render. The parser is tolerant of schema drift: it pulls
//! common field names and never panics on unexpected shapes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RepoPilotCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoPilotFinding {
    /// Normalized to one of CRITICAL/HIGH/MEDIUM/LOW/INFO.
    pub severity: String,
    pub title: String,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoPilotReport {
    pub available: bool,
    pub health_score: Option<i64>,
    pub total: usize,
    pub counts: RepoPilotCounts,
    pub findings: Vec<RepoPilotFinding>,
    pub error: Option<String>,
}

impl RepoPilotReport {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            ..Default::default()
        }
    }

    /// High/Critical findings are the ones that should block a confident commit.
    pub fn blocking_count(&self) -> usize {
        self.counts.critical + self.counts.high
    }
}

fn normalize_severity(raw: Option<&str>) -> String {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "critical" | "p0" | "blocker" => "CRITICAL",
        "high" | "p1" | "error" => "HIGH",
        "medium" | "p2" | "warning" | "warn" => "MEDIUM",
        "low" | "p3" | "minor" => "LOW",
        _ => "INFO",
    }
    .to_string()
}

fn first_str(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(found) = value.get(key).and_then(|item| item.as_str())
            && !found.trim().is_empty()
        {
            return Some(found.to_string());
        }
    }
    None
}

/// Parse a RepoPilot `review --format json` document into a structured report.
pub fn parse_review_json(raw: &str) -> RepoPilotReport {
    let value: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(error) => {
            return RepoPilotReport::error(format!("Failed to parse RepoPilot JSON: {error}"));
        }
    };

    let health_score = value.get("health_score").and_then(Value::as_i64);
    let raw_findings = value
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut counts = RepoPilotCounts::default();
    let mut findings = Vec::new();

    for item in &raw_findings {
        let severity =
            normalize_severity(first_str(item, &["severity", "priority", "level"]).as_deref());
        match severity.as_str() {
            "CRITICAL" => counts.critical += 1,
            "HIGH" => counts.high += 1,
            "MEDIUM" => counts.medium += 1,
            "LOW" => counts.low += 1,
            _ => counts.info += 1,
        }
        findings.push(RepoPilotFinding {
            severity,
            title: first_str(item, &["title", "message", "rule", "intent"])
                .unwrap_or_else(|| "Untitled finding".to_string()),
            file: first_str(item, &["file", "path", "location"]),
            line: item.get("line").and_then(Value::as_u64),
            rule: first_str(item, &["rule", "rule_id", "category"]),
        });
    }

    RepoPilotReport {
        available: true,
        health_score,
        total: findings.len(),
        counts,
        findings,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_findings_and_counts_by_severity() {
        let json = r#"{
            "health_score": 87,
            "findings": [
                {"severity": "HIGH", "title": "SQL injection", "file": "src/db.rs", "line": 42},
                {"priority": "p2", "message": "Large function", "path": "src/big.rs"},
                {"level": "critical", "rule": "secret.hardcoded"}
            ]
        }"#;
        let report = parse_review_json(json);
        assert!(report.available);
        assert_eq!(report.health_score, Some(87));
        assert_eq!(report.total, 3);
        assert_eq!(report.counts.high, 1);
        assert_eq!(report.counts.medium, 1);
        assert_eq!(report.counts.critical, 1);
        assert_eq!(report.blocking_count(), 2);

        let first = &report.findings[0];
        assert_eq!(first.severity, "HIGH");
        assert_eq!(first.file.as_deref(), Some("src/db.rs"));
        assert_eq!(first.line, Some(42));
    }

    #[test]
    fn empty_findings_is_a_clean_report() {
        let report = parse_review_json(r#"{"health_score": 100, "findings": []}"#);
        assert!(report.available);
        assert_eq!(report.total, 0);
        assert_eq!(report.blocking_count(), 0);
        assert!(report.error.is_none());
    }

    #[test]
    fn missing_findings_key_is_tolerated() {
        let report = parse_review_json(r#"{"health_score": 50}"#);
        assert!(report.available);
        assert_eq!(report.total, 0);
    }

    #[test]
    fn invalid_json_returns_error_not_panic() {
        let report = parse_review_json("not json at all");
        assert!(!report.available);
        assert!(report.error.is_some());
    }

    #[test]
    fn untitled_finding_gets_placeholder() {
        let report = parse_review_json(r#"{"findings": [{"severity": "low"}]}"#);
        assert_eq!(report.findings[0].title, "Untitled finding");
        assert_eq!(report.counts.low, 1);
    }
}
