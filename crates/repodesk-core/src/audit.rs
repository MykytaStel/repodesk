use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub project_name: String,
    pub action_type: String,
    pub details: String,
    pub previous_hash: String,
    pub hash: String,
}

pub struct AuditLogger {
    log_path: PathBuf,
}

impl AuditLogger {
    pub fn new() -> RepoDeskResult<Self> {
        let paths = RepoDeskPaths::resolve()?;
        let audit_dir = paths.home.join("audit");
        if !audit_dir.exists() {
            fs::create_dir_all(&audit_dir)?;
        }
        let log_path = audit_dir.join("audit_trail.jsonl");
        Ok(Self { log_path })
    }

    fn get_last_hash(&self) -> RepoDeskResult<String> {
        if !self.log_path.exists() {
            return Ok(GENESIS_HASH.to_string());
        }

        let content = fs::read_to_string(&self.log_path)?;
        let lines: Vec<(usize, &str)> = content.lines().enumerate().collect();
        for (index, line) in lines.into_iter().rev() {
            if line.trim().is_empty() {
                continue;
            }
            let event = parse_event_line(line, index)?;
            return Ok(event.hash);
        }
        Ok(GENESIS_HASH.to_string())
    }

    pub fn log_action(&self, action_type: &str, details: &str) -> RepoDeskResult<()> {
        let project_name = get_active_project()
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "Unknown".to_string());

        let timestamp = Utc::now();
        let previous_hash = self.get_last_hash()?;

        let hash = compute_hash(
            &timestamp,
            &project_name,
            action_type,
            details,
            &previous_hash,
        );

        let event = AuditEvent {
            timestamp,
            project_name,
            action_type: action_type.to_string(),
            details: details.to_string(),
            previous_hash,
            hash,
        };

        let json_line = serde_json::to_string(&event)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        writeln!(file, "{json_line}")?;

        Ok(())
    }

    /// Recompute the hash an event should have, given its fields and the hash of
    /// the event before it. Must mirror [`AuditLogger::log_action`] exactly.
    fn expected_hash(event: &AuditEvent) -> String {
        compute_hash(
            &event.timestamp,
            &event.project_name,
            &event.action_type,
            &event.details,
            &event.previous_hash,
        )
    }

    /// All events in the trail, oldest first. Empty when nothing is logged yet.
    pub fn list_events(&self) -> RepoDeskResult<Vec<AuditEvent>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.log_path)?;
        let mut events = Vec::new();
        for (index, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            events.push(parse_event_line(line, index)?);
        }
        Ok(events)
    }

    /// Verify the SHA-256 hash chain end to end: each event's `hash` must match a
    /// recomputation of its fields, and its `previous_hash` must equal the prior
    /// event's `hash` (the first links to the genesis hash).
    pub fn verify_chain(&self) -> RepoDeskResult<ChainVerification> {
        if !self.log_path.exists() {
            return Ok(ChainVerification::valid(0));
        }

        let content = fs::read_to_string(&self.log_path)?;
        let lines: Vec<(usize, &str)> = content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .collect();
        let total = lines.len();
        let mut previous = GENESIS_HASH.to_string();
        for (event_index, (line_index, line)) in lines.iter().enumerate() {
            let event = match serde_json::from_str::<AuditEvent>(line) {
                Ok(event) => event,
                Err(_) => {
                    return Ok(ChainVerification::broken(
                        total,
                        event_index,
                        &format!("event row {} is not valid JSON", line_index + 1),
                    ));
                }
            };
            if event.previous_hash != previous {
                return Ok(ChainVerification::broken(
                    total,
                    event_index,
                    "previous-hash link does not match the prior event",
                ));
            }
            if Self::expected_hash(&event) != event.hash {
                return Ok(ChainVerification::broken(
                    total,
                    event_index,
                    "event hash does not match its contents (tampered)",
                ));
            }
            previous = event.hash.clone();
        }
        Ok(ChainVerification::valid(total))
    }
}

fn compute_hash(
    timestamp: &DateTime<Utc>,
    project_name: &str,
    action_type: &str,
    details: &str,
    previous_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.to_rfc3339().as_bytes());
    hasher.update(project_name.as_bytes());
    hasher.update(action_type.as_bytes());
    hasher.update(details.as_bytes());
    hasher.update(previous_hash.as_bytes());
    hex::encode(hasher.finalize())
}

fn parse_event_line(line: &str, zero_based_line: usize) -> RepoDeskResult<AuditEvent> {
    serde_json::from_str::<AuditEvent>(line).map_err(|error| {
        RepoDeskError::Api(format!(
            "invalid audit event at line {}: {error}",
            zero_based_line + 1
        ))
    })
}

/// Result of verifying the audit-trail hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerification {
    pub valid: bool,
    pub total_events: usize,
    /// Index of the first broken link, when `valid` is false.
    pub broken_at: Option<usize>,
    pub message: String,
}

impl ChainVerification {
    fn valid(total: usize) -> Self {
        let message = if total == 0 {
            "No audit events recorded yet.".to_string()
        } else {
            format!("Chain verified across {total} event(s).")
        };
        Self {
            valid: true,
            total_events: total,
            broken_at: None,
            message,
        }
    }

    fn broken(total: usize, index: usize, reason: &str) -> Self {
        Self {
            valid: false,
            total_events: total,
            broken_at: Some(index),
            message: format!("Chain broken at event {index}: {reason}."),
        }
    }
}

pub fn log_audit_event(action_type: &str, details: &str) -> RepoDeskResult<()> {
    let logger = AuditLogger::new()?;
    logger.log_action(action_type, details)
}

/// List the most recent audit events, newest first, capped at `limit`.
pub fn recent_events(limit: usize) -> RepoDeskResult<Vec<AuditEvent>> {
    let mut events = AuditLogger::new()?.list_events()?;
    events.reverse();
    events.truncate(limit);
    Ok(events)
}

/// Verify the audit-trail hash chain.
pub fn verify_audit_chain() -> RepoDeskResult<ChainVerification> {
    AuditLogger::new()?.verify_chain()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn logger_at(log_path: PathBuf) -> AuditLogger {
        AuditLogger { log_path }
    }

    fn sample_event(previous_hash: &str, details: &str) -> AuditEvent {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 6, 20, 1, 2, 3)
            .single()
            .expect("fixed timestamp");
        let mut event = AuditEvent {
            timestamp,
            project_name: "demo".to_string(),
            action_type: "agent_run".to_string(),
            details: details.to_string(),
            previous_hash: previous_hash.to_string(),
            hash: String::new(),
        };
        event.hash = AuditLogger::expected_hash(&event);
        event
    }

    #[test]
    fn verify_chain_accepts_valid_events() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let logger = logger_at(dir.path().join("audit.jsonl"));
        let first = sample_event(GENESIS_HASH, "first");
        let second = sample_event(&first.hash, "second");
        std::fs::write(
            &logger.log_path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first).unwrap(),
                serde_json::to_string(&second).unwrap()
            ),
        )
        .expect("write audit log");

        let verification = logger.verify_chain().expect("verify");
        assert!(verification.valid);
        assert_eq!(verification.total_events, 2);
    }

    #[test]
    fn verify_chain_reports_malformed_json_as_broken() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let logger = logger_at(dir.path().join("audit.jsonl"));
        std::fs::write(&logger.log_path, "{not json}\n").expect("write audit log");

        let verification = logger.verify_chain().expect("verify");
        assert!(!verification.valid);
        assert_eq!(verification.total_events, 1);
        assert_eq!(verification.broken_at, Some(0));
        assert!(verification.message.contains("not valid JSON"));
    }

    #[test]
    fn list_events_rejects_malformed_json() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let logger = logger_at(dir.path().join("audit.jsonl"));
        std::fs::write(&logger.log_path, "{not json}\n").expect("write audit log");

        assert!(logger.list_events().is_err());
    }

    #[test]
    fn last_hash_rejects_corrupt_tail() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let logger = logger_at(dir.path().join("audit.jsonl"));
        let first = sample_event(GENESIS_HASH, "first");
        std::fs::write(
            &logger.log_path,
            format!("{}\n{{not json}}\n", serde_json::to_string(&first).unwrap()),
        )
        .expect("write audit log");

        assert!(logger.get_last_hash().is_err());
    }
}
