use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

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
            return Ok(String::from("0000000000000000000000000000000000000000000000000000000000000000")); // Genesis hash
        }

        let content = fs::read_to_string(&self.log_path)?;
        if let Some(last_line) = content.lines().last() {
            if let Ok(event) = serde_json::from_str::<AuditEvent>(last_line) {
                return Ok(event.hash);
            }
        }
        Ok(String::from("0000000000000000000000000000000000000000000000000000000000000000"))
    }

    pub fn log_action(&self, action_type: &str, details: &str) -> RepoDeskResult<()> {
        let project_name = get_active_project()
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "Unknown".to_string());

        let timestamp = Utc::now();
        let previous_hash = self.get_last_hash()?;

        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_rfc3339().as_bytes());
        hasher.update(project_name.as_bytes());
        hasher.update(action_type.as_bytes());
        hasher.update(details.as_bytes());
        hasher.update(previous_hash.as_bytes());
        
        let result = hasher.finalize();
        let hash = hex::encode(result);

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

        writeln!(file, "{}", json_line)?;

        Ok(())
    }
}

pub fn log_audit_event(action_type: &str, details: &str) -> RepoDeskResult<()> {
    let logger = AuditLogger::new()?;
    logger.log_action(action_type, details)
}
