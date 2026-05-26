use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub name: String,
    pub project: String,
    pub task_id: String,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Ended,
}

pub fn begin_session(name: &str) -> RepoDeskResult<SessionRecord> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let sessions_dir = paths.home.join("sessions");
    fs::create_dir_all(&sessions_dir)?;

    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let task_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());

    let now = Utc::now();
    let id = format!("{}-{}", now.format("%Y-%m-%d-%H%M%S"), slugify(name));

    let record = SessionRecord {
        id: id.clone(),
        name: name.trim().to_string(),
        project,
        task_id,
        status: SessionStatus::Active,
        started_at: now,
        ended_at: None,
    };

    write_session(&sessions_dir.join(&id).join("session.toml"), &record)?;
    fs::write(paths.config_dir.join("active_session"), format!("{id}\n"))?;

    Ok(record)
}

pub fn show_active_session() -> RepoDeskResult<SessionRecord> {
    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("active_session");

    if !file.exists() {
        return Err(RepoDeskError::ActiveProjectNotSet);
    }

    let id = fs::read_to_string(file)?.trim().to_string();
    let session_file = paths.home.join("sessions").join(&id).join("session.toml");
    read_session(&session_file)
}

pub fn end_session() -> RepoDeskResult<SessionRecord> {
    let paths = RepoDeskPaths::resolve()?;
    let file = paths.config_dir.join("active_session");

    if !file.exists() {
        return Err(RepoDeskError::ActiveProjectNotSet);
    }

    let id = fs::read_to_string(&file)?.trim().to_string();
    let session_file = paths.home.join("sessions").join(&id).join("session.toml");
    let mut record = read_session(&session_file)?;

    record.status = SessionStatus::Ended;
    record.ended_at = Some(Utc::now());

    write_session(&session_file, &record)?;
    fs::remove_file(file).ok();

    Ok(record)
}

pub fn format_session_record(record: &SessionRecord) -> String {
    format!(
        r#"Session:
  id: {}
  name: {}
  project: {}
  task: {}
  status: {:?}
  started: {}
  ended: {}
"#,
        record.id,
        record.name,
        record.project,
        record.task_id,
        record.status,
        record.started_at,
        record
            .ended_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not ended".to_string())
    )
}

fn write_session(path: &PathBuf, record: &SessionRecord) -> RepoDeskResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, toml::to_string_pretty(record)?)?;
    Ok(())
}

fn read_session(path: &PathBuf) -> RepoDeskResult<SessionRecord> {
    let content = fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }

    let slug = slug.trim_matches('-').chars().take(40).collect::<String>();

    if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    }
}
