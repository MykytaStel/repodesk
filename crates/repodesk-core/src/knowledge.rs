use std::path::PathBuf;

use chrono::Utc;

use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

pub fn append_knowledge(kind: &str, text: &str) -> RepoDeskResult<PathBuf> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;
    let (file_name, heading) = knowledge_file(kind);
    let file = paths.project_dir(&project.name).join(file_name);

    if !file.exists() {
        std::fs::write(&file, format!("# {heading}\n\n"))?;
    }

    let entry = format!(
        "\n## {}\n\n{}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        text.trim()
    );

    let mut existing = std::fs::read_to_string(&file).unwrap_or_default();
    existing.push_str(&entry);
    std::fs::write(&file, existing)?;

    Ok(file)
}

pub fn read_knowledge(kind: &str) -> RepoDeskResult<String> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;
    let (file_name, heading) = knowledge_file(kind);
    let file = paths.project_dir(&project.name).join(file_name);

    if !file.exists() {
        return Ok(format!("# {heading}\n\nNo entries yet.\n"));
    }

    Ok(std::fs::read_to_string(file)?)
}

pub fn format_knowledge(kind: &str, content: &str) -> String {
    format!("Knowledge: {kind}\n\n{content}")
}

fn knowledge_file(kind: &str) -> (&'static str, &'static str) {
    match kind.to_ascii_lowercase().as_str() {
        "decision" | "decisions" => ("decisions.md", "Decisions"),
        "risk" | "risks" => ("risks.md", "Risks"),
        _ => ("memory.md", "Project Memory"),
    }
}
