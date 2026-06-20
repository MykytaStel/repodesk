use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub name: String,
    pub description: String,
    pub steps: Vec<PlaybookStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    pub name: String,
    pub action: String,
    pub payload: String,
}

pub fn get_playbooks_dir() -> RepoDeskResult<PathBuf> {
    let project = get_active_project()?;
    let dir = project.path.join(".repodesk").join("workflows");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

pub fn list_playbooks() -> RepoDeskResult<Vec<Playbook>> {
    let dir = get_playbooks_dir()?;
    let mut playbooks = Vec::new();

    if !dir.exists() {
        return Ok(playbooks);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            if let Ok(playbook) = serde_json::from_str::<Playbook>(&content) {
                playbooks.push(playbook);
            }
        }
    }

    Ok(playbooks)
}

pub fn create_playbook(playbook: &Playbook) -> RepoDeskResult<()> {
    let dir = get_playbooks_dir()?;
    let file_name = format!("{}.json", playbook.name.replace(" ", "_").to_lowercase());
    let path = dir.join(file_name);

    let content = serde_json::to_string_pretty(playbook)?;
    fs::write(path, content)?;

    Ok(())
}
