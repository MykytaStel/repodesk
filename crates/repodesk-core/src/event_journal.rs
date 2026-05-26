use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone)]
pub struct LogEventInput {
    pub module_name: String,
    pub level: String,
    pub message: String,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub task_id: String,
    pub module_name: String,
    pub level: String,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

pub fn log_event(input: LogEventInput) -> RepoDeskResult<PathBuf> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let journal_file = paths.logs_dir.join("event-journal.jsonl");

    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let task_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());

    let metadata = input.metadata.into_iter().collect::<BTreeMap<_, _>>();

    let entry = EventEntry {
        timestamp: Utc::now(),
        project,
        task_id,
        module_name: input.module_name,
        level: input.level,
        message: input.message,
        metadata,
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&journal_file)?;

    writeln!(file, "{}", serde_json::to_string(&entry)?)?;

    Ok(journal_file)
}

pub fn read_events(limit: usize) -> RepoDeskResult<Vec<EventEntry>> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let journal_file = paths.logs_dir.join("event-journal.jsonl");

    if !journal_file.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(journal_file)?;
    let mut entries = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        entries.push(serde_json::from_str::<EventEntry>(line)?);
    }

    let keep = limit.min(entries.len());
    Ok(entries.into_iter().rev().take(keep).collect())
}

pub fn format_events(events: &[EventEntry]) -> String {
    if events.is_empty() {
        return "No events recorded yet.\n".to_string();
    }

    let mut output = String::new();
    output.push_str("Event journal:\n\n");

    for event in events {
        output.push_str(&format!(
            "- [{}] {} :: {}\n",
            event.level, event.module_name, event.message
        ));
        output.push_str(&format!("  time: {}\n", event.timestamp));
        output.push_str(&format!("  project: {}\n", event.project));
        output.push_str(&format!("  task: {}\n", event.task_id));

        if !event.metadata.is_empty() {
            output.push_str("  metadata:\n");
            for (key, value) in &event.metadata {
                output.push_str(&format!("    {}: {}\n", key, value));
            }
        }
    }

    output
}
