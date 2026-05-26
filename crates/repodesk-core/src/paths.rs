use std::env;
use std::path::PathBuf;

use crate::errors::{RepoDeskError, RepoDeskResult};

#[derive(Debug, Clone)]
pub struct RepoDeskPaths {
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub runs_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub active_project_file: PathBuf,
}

impl RepoDeskPaths {
    pub fn resolve() -> RepoDeskResult<Self> {
        let home = match env::var("REPODESK_HOME") {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
            _ => dirs::home_dir()
                .map(|home| home.join(".repodesk"))
                .ok_or(RepoDeskError::HomeDirectoryNotFound)?,
        };

        Ok(Self {
            config_dir: home.join("config"),
            projects_dir: home.join("projects"),
            runs_dir: home.join("runs"),
            logs_dir: home.join("logs"),
            cache_dir: home.join("cache"),
            active_project_file: home.join("config").join("active_project"),
            home,
        })
    }

    pub fn project_dir(&self, name: &str) -> PathBuf {
        self.projects_dir.join(name)
    }

    pub fn project_config_file(&self, name: &str) -> PathBuf {
        self.project_dir(name).join("project.toml")
    }
}
