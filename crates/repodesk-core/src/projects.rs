use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::init;
use crate::paths::RepoDeskPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub path: PathBuf,
    pub project_type: String,
    pub main_language: Option<String>,
    pub checks: Vec<String>,
    pub context_ignore: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AddProjectInput {
    pub name: String,
    pub path: PathBuf,
    pub project_type: String,
    pub main_language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub config: ProjectConfig,
    pub is_active: bool,
}

pub fn add_project(input: AddProjectInput) -> RepoDeskResult<ProjectConfig> {
    init::init_home()?;

    validate_project_name(&input.name)?;
    validate_project_path(&input.path)?;

    let paths = RepoDeskPaths::resolve()?;
    let project_dir = paths.project_dir(&input.name);
    let project_file = paths.project_config_file(&input.name);

    if project_file.exists() {
        return Err(RepoDeskError::ProjectAlreadyExists(input.name));
    }

    fs::create_dir_all(project_dir.join("prompts"))?;

    let now = Utc::now();

    let config = ProjectConfig {
        name: input.name,
        path: input.path,
        project_type: input.project_type.clone(),
        main_language: input
            .main_language
            .or_else(|| infer_main_language(&input.project_type)),
        checks: default_checks_for_project_type(&input.project_type),
        context_ignore: default_context_ignore(),
        created_at: now,
        updated_at: now,
    };

    write_project_config(&project_file, &config)?;

    let memory_file = paths.project_dir(&config.name).join("memory.md");
    let decisions_file = paths.project_dir(&config.name).join("decisions.md");
    let risks_file = paths.project_dir(&config.name).join("risks.md");

    write_if_missing(
        &memory_file,
        &format!(
            "# {} Memory\n\nProject-specific notes, constraints, and workflow memory.\n",
            config.name
        ),
    )?;
    write_if_missing(
        &decisions_file,
        "# Decisions\n\nImportant architecture and process decisions.\n",
    )?;
    write_if_missing(&risks_file, "# Risks\n\nOpen project and workflow risks.\n")?;

    Ok(config)
}

pub fn list_projects() -> RepoDeskResult<Vec<ProjectInfo>> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let active = read_active_project().ok();

    let mut projects = Vec::new();

    for entry in fs::read_dir(paths.projects_dir)? {
        let entry = entry?;
        let project_file = entry.path().join("project.toml");

        if project_file.exists() {
            let config = read_project_config(&project_file)?;
            let is_active = active.as_deref() == Some(config.name.as_str());
            projects.push(ProjectInfo { config, is_active });
        }
    }

    projects.sort_by(|a, b| a.config.name.cmp(&b.config.name));
    Ok(projects)
}

pub fn use_project(name: &str) -> RepoDeskResult<ProjectConfig> {
    validate_project_name(name)?;

    let config = get_project(name)?;
    let paths = RepoDeskPaths::resolve()?;
    fs::write(&paths.active_project_file, format!("{name}\n"))?;

    Ok(config)
}

pub fn get_project(name: &str) -> RepoDeskResult<ProjectConfig> {
    validate_project_name(name)?;

    let paths = RepoDeskPaths::resolve()?;
    let project_file = paths.project_config_file(name);

    if !project_file.exists() {
        return Err(RepoDeskError::ProjectNotFound(name.to_string()));
    }

    read_project_config(&project_file)
}

pub fn get_active_project() -> RepoDeskResult<ProjectConfig> {
    let name = read_active_project()?;
    get_project(&name)
}

pub fn read_active_project() -> RepoDeskResult<String> {
    let paths = RepoDeskPaths::resolve()?;

    if !paths.active_project_file.exists() {
        return Err(RepoDeskError::ActiveProjectNotSet);
    }

    let name = fs::read_to_string(paths.active_project_file)?;
    let name = name.trim().to_string();

    if name.is_empty() {
        return Err(RepoDeskError::ActiveProjectNotSet);
    }

    Ok(name)
}

pub fn update_project_ignore_rules(
    name: &str,
    ignore_rules: Vec<String>,
) -> RepoDeskResult<ProjectConfig> {
    let mut config = get_project(name)?;
    config.context_ignore = ignore_rules;
    config.updated_at = Utc::now();
    let paths = RepoDeskPaths::resolve()?;
    let project_file = paths.project_config_file(name);
    write_project_config(&project_file, &config)?;
    Ok(config)
}

/// Register a check command on a project. The command is validated against the
/// same allowlist `run_checks` enforces, so an unrunnable check can never be
/// stored. Idempotent: re-adding an existing check is a no-op.
pub fn add_project_check(name: &str, command: &str) -> RepoDeskResult<ProjectConfig> {
    let command = command.trim().to_string();
    crate::checks::is_allowed_check_command(&command)
        .map_err(RepoDeskError::InvalidCheckCommand)?;

    let mut config = get_project(name)?;
    if config.checks.iter().any(|existing| existing == &command) {
        return Ok(config);
    }

    config.checks.push(command);
    config.updated_at = Utc::now();

    let paths = RepoDeskPaths::resolve()?;
    let project_file = paths.project_config_file(name);
    write_project_config(&project_file, &config)?;

    Ok(config)
}

fn write_project_config(path: &Path, config: &ProjectConfig) -> RepoDeskResult<()> {
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

fn read_project_config(path: &Path) -> RepoDeskResult<ProjectConfig> {
    let content = fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}

fn write_if_missing(path: &Path, content: &str) -> RepoDeskResult<()> {
    if !path.exists() {
        fs::write(path, content)?;
    }

    Ok(())
}

fn validate_project_path(path: &Path) -> RepoDeskResult<()> {
    if !path.exists() {
        return Err(RepoDeskError::ProjectPathDoesNotExist(
            path.display().to_string(),
        ));
    }

    Ok(())
}

fn validate_project_name(name: &str) -> RepoDeskResult<()> {
    let valid = !name.trim().is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');

    if valid {
        Ok(())
    } else {
        Err(RepoDeskError::InvalidProjectName(name.to_string()))
    }
}

fn infer_main_language(project_type: &str) -> Option<String> {
    match project_type {
        "rust" | "rust-cli" | "rust-desktop" => Some("rust".to_string()),
        "node" | "react" | "react-native" => Some("typescript".to_string()),
        "python" => Some("python".to_string()),
        "monorepo" => None,
        _ => None,
    }
}

fn default_checks_for_project_type(project_type: &str) -> Vec<String> {
    match project_type {
        "rust" | "rust-cli" | "rust-desktop" => vec![
            "cargo fmt --all -- --check".to_string(),
            "cargo clippy --all-targets --all-features -- -D warnings".to_string(),
            "cargo test --all".to_string(),
        ],
        "node" | "react" | "react-native" => {
            vec!["pnpm typecheck".to_string(), "pnpm test".to_string()]
        }
        "python" => vec!["python -m pytest".to_string()],
        _ => Vec::new(),
    }
}

fn default_context_ignore() -> Vec<String> {
    vec![
        ".git".to_string(),
        "target".to_string(),
        "node_modules".to_string(),
        "dist".to_string(),
        "build".to_string(),
        ".next".to_string(),
        ".turbo".to_string(),
        "coverage".to_string(),
        "*.pdf".to_string(),
        "*.html".to_string(),
        "*.log".to_string(),
    ]
}
