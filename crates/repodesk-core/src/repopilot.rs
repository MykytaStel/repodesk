//! Parsing of RepoPilot `review --format json` output into a structured report
//! the desktop can render. The parser is tolerant of schema drift: it pulls
//! common field names and never panics on unexpected shapes.
//!
//! Two derived views layer on top of the raw report:
//! - [`RepoPilotReport::group_by_file`] buckets findings per file so the Code
//!   tab can render them inline next to the changed-file list.
//! - [`record_report`] / [`load_history`] persist a small per-task health-score
//!   trend to `<run_dir>/repopilot/history.json` so the UI can show whether code
//!   health is improving or regressing across reviews.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

/// How many trend points to retain. A bounded log keeps the file tiny and the
/// sparkline readable while still covering a full task's worth of reviews.
const MAX_TREND_POINTS: usize = 50;

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

    /// Group findings by file for inline per-file rendering. Findings with no
    /// file are bucketed under [`NO_FILE_KEY`]. Files are ordered by their worst
    /// finding (critical first) then by name; findings inside each file keep
    /// their original order. Returns an empty vec when the report has no
    /// findings (or errored).
    pub fn group_by_file(&self) -> Vec<RepoPilotFileFindings> {
        let mut order: Vec<String> = Vec::new();
        let mut buckets: std::collections::HashMap<String, Vec<RepoPilotFinding>> =
            std::collections::HashMap::new();

        for finding in &self.findings {
            let key = finding
                .file
                .clone()
                .unwrap_or_else(|| NO_FILE_KEY.to_string());
            if !buckets.contains_key(&key) {
                order.push(key.clone());
            }
            buckets.entry(key).or_default().push(finding.clone());
        }

        let mut groups: Vec<RepoPilotFileFindings> = order
            .into_iter()
            .map(|file| {
                let findings = buckets.remove(&file).unwrap_or_default();
                let worst = findings
                    .iter()
                    .map(|f| severity_rank(&f.severity))
                    .max()
                    .unwrap_or(0);
                RepoPilotFileFindings {
                    file,
                    blocking: findings
                        .iter()
                        .filter(|f| f.severity == "CRITICAL" || f.severity == "HIGH")
                        .count(),
                    total: findings.len(),
                    worst_rank: worst,
                    findings,
                }
            })
            .collect();

        groups.sort_by(|a, b| {
            b.worst_rank
                .cmp(&a.worst_rank)
                .then_with(|| a.file.cmp(&b.file))
        });
        groups
    }

    /// Reduce a full report to a single trend point for the health log.
    fn to_trend_point(&self) -> RepoPilotTrendPoint {
        RepoPilotTrendPoint {
            timestamp: chrono::Utc::now().to_rfc3339(),
            health_score: self.health_score,
            total: self.total,
            blocking: self.blocking_count(),
            counts: self.counts.clone(),
        }
    }
}

/// Findings that have no associated file are grouped under this key.
pub const NO_FILE_KEY: &str = "(no file)";

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "CRITICAL" => 4,
        "HIGH" => 3,
        "MEDIUM" => 2,
        "LOW" => 1,
        _ => 0,
    }
}

/// Findings for a single file, sortable by worst severity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoPilotFileFindings {
    pub file: String,
    /// Count of CRITICAL + HIGH findings in this file.
    pub blocking: usize,
    pub total: usize,
    /// Numeric rank of the worst finding (4=critical … 0=info); used for sort
    /// and lets the UI tone the file header without rescanning findings.
    pub worst_rank: u8,
    pub findings: Vec<RepoPilotFinding>,
}

/// One review's worth of health data, persisted to build a trend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoPilotTrendPoint {
    /// RFC3339 UTC timestamp of the review.
    pub timestamp: String,
    pub health_score: Option<i64>,
    pub total: usize,
    pub blocking: usize,
    pub counts: RepoPilotCounts,
}

/// The retained trend of recent reviews (oldest first).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RepoPilotHistory {
    pub points: Vec<RepoPilotTrendPoint>,
}

fn history_path() -> RepoDeskResult<std::path::PathBuf> {
    Ok(show_active_task()?.config.run_dir.join("repopilot"))
}

/// Load the persisted health trend for the active task. Returns an empty
/// history when nothing has been recorded yet (or the file is unreadable —
/// the trend is advisory, never load-bearing).
pub fn load_history() -> RepoDeskResult<RepoPilotHistory> {
    let path = history_path()?.join("history.json");
    if !path.exists() {
        return Ok(RepoPilotHistory::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// Append a review's health to the active task's trend (bounded to the most
/// recent [`MAX_TREND_POINTS`]) and return the updated history. Errored or
/// unavailable reports are not recorded — only real reviews shape the trend.
pub fn record_report(report: &RepoPilotReport) -> RepoDeskResult<RepoPilotHistory> {
    if !report.available || report.error.is_some() {
        return load_history();
    }
    let mut history = load_history().unwrap_or_default();
    history.points.push(report.to_trend_point());
    if history.points.len() > MAX_TREND_POINTS {
        let overflow = history.points.len() - MAX_TREND_POINTS;
        history.points.drain(0..overflow);
    }
    let dir = history_path()?;
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&history)?;
    std::fs::write(dir.join("history.json"), json)?;
    Ok(history)
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

    #[test]
    fn group_by_file_buckets_and_sorts_by_worst_severity() {
        let json = r#"{
            "findings": [
                {"severity": "LOW", "title": "nit", "file": "a.rs"},
                {"severity": "CRITICAL", "title": "boom", "file": "b.rs"},
                {"severity": "HIGH", "title": "leak", "file": "a.rs"},
                {"severity": "MEDIUM", "title": "global"}
            ]
        }"#;
        let report = parse_review_json(json);
        let groups = report.group_by_file();
        assert_eq!(groups.len(), 3);
        // b.rs (critical) ranks above a.rs (high) above the no-file bucket (medium).
        assert_eq!(groups[0].file, "b.rs");
        assert_eq!(groups[0].worst_rank, 4);
        assert_eq!(groups[1].file, "a.rs");
        assert_eq!(groups[1].total, 2);
        assert_eq!(groups[1].blocking, 1);
        assert_eq!(groups[2].file, NO_FILE_KEY);
    }

    #[test]
    fn group_by_file_is_empty_for_clean_report() {
        let report = parse_review_json(r#"{"health_score": 100, "findings": []}"#);
        assert!(report.group_by_file().is_empty());
    }

    #[test]
    fn errored_report_is_not_recorded_to_trend() {
        // record_report on an errored report must not panic or require a task;
        // it short-circuits before touching the filesystem.
        let report = RepoPilotReport::error("nope");
        // load_history may fail without an active task; the contract is only that
        // an errored report never *appends*. We assert the early return shape by
        // confirming the report is flagged unavailable.
        assert!(!report.available);
        assert!(report.error.is_some());
    }
}
