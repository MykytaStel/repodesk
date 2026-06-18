use super::{CommandResult, ErrorPayload, ProjectAddInput, validate_path, validate_short_id};

#[tauri::command]
pub fn project_info() -> CommandResult {
    match repodesk_core::projects::get_active_project() {
        Ok(config) => {
            let mut stdout = format!(
                "Active project:\n  name: {}\n  path: {}\n  type: {}",
                config.name,
                config.path.display(),
                config.project_type
            );
            if let Some(language) = config.main_language {
                stdout.push_str(&format!("\n  main language: {language}"));
            }
            if config.checks.is_empty() {
                stdout.push_str("\n  checks: none configured");
            } else {
                stdout.push_str("\n  checks:");
                for check in config.checks {
                    stdout.push_str(&format!("\n    - {check}"));
                }
            }
            if config.context_ignore.is_empty() {
                stdout.push_str("\n  context ignore: none configured");
            } else {
                stdout.push_str("\n  context ignore:");
                for item in config.context_ignore {
                    stdout.push_str(&format!("\n    - {item}"));
                }
            }
            CommandResult {
                ok: true,
                command: "repodesk project info".to_string(),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk project info".to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    }
}

#[tauri::command]
pub fn project_list() -> CommandResult {
    match repodesk_core::projects::list_projects() {
        Ok(projects) => {
            let mut stdout = String::new();
            if projects.is_empty() {
                stdout.push_str("No projects registered yet.\nAdd one with:\n  repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli");
            } else {
                stdout.push_str("Registered projects:\n");
                for project in projects {
                    let marker = if project.is_active { "*" } else { " " };
                    stdout.push_str(&format!(
                        "{} {} ({})\n    path: {}\n",
                        marker,
                        project.config.name,
                        project.config.project_type,
                        project.config.path.display()
                    ));
                }
            }
            CommandResult {
                ok: true,
                command: "repodesk project list".to_string(),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            }
        }
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk project list".to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    }
}

#[tauri::command]
pub fn project_use(name: String) -> Result<CommandResult, ErrorPayload> {
    validate_short_id("Project name", &name)?;
    match repodesk_core::projects::use_project(name.trim()) {
        Ok(config) => {
            let stdout = format!(
                "Active project set to '{}'\nPath: {}",
                config.name,
                config.path.display()
            );
            Ok(CommandResult {
                ok: true,
                command: format!("repodesk project use {}", name),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
        Err(e) => Ok(CommandResult {
            ok: false,
            command: format!("repodesk project use {}", name),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        }),
    }
}

#[tauri::command]
pub fn project_add(input: ProjectAddInput) -> Result<CommandResult, ErrorPayload> {
    validate_short_id("Project name", &input.name)?;
    validate_path(&input.path)?;
    validate_short_id("Project type", &input.project_type)?;

    if let Some(language) = &input.main_language
        && !language.trim().is_empty()
    {
        validate_short_id("Main language", language)?;
    }

    let core_input = repodesk_core::projects::AddProjectInput {
        name: input.name.trim().to_string(),
        path: std::path::PathBuf::from(input.path.trim()),
        project_type: input.project_type.trim().to_string(),
        main_language: input
            .main_language
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    };

    match repodesk_core::projects::add_project(core_input) {
        Ok(config) => {
            let mut stdout = format!(
                "Project added:\n  name: {}\n  path: {}\n  type: {}",
                config.name,
                config.path.display(),
                config.project_type
            );
            if let Some(language) = config.main_language {
                stdout.push_str(&format!("\n  main language: {language}"));
            }
            if config.checks.is_empty() {
                stdout.push_str("\n  checks: none configured yet");
            } else {
                stdout.push_str("\n  checks:");
                for check in config.checks {
                    stdout.push_str(&format!("\n    - {check}"));
                }
            }
            Ok(CommandResult {
                ok: true,
                command: format!("repodesk project add {}", config.name),
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
        Err(e) => Ok(CommandResult {
            ok: false,
            command: format!("repodesk project add {}", input.name),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        }),
    }
}

#[tauri::command]
pub fn get_active_project_config() -> Result<repodesk_core::projects::ProjectConfig, ErrorPayload> {
    repodesk_core::projects::get_active_project().map_err(ErrorPayload::from)
}

/// Structured list of connected projects (for the header project switcher).
#[tauri::command]
pub fn project_list_configs() -> Result<Vec<repodesk_core::projects::ProjectConfig>, ErrorPayload> {
    repodesk_core::projects::list_projects()
        .map(|projects| projects.into_iter().map(|p| p.config).collect())
        .map_err(ErrorPayload::from)
}

#[tauri::command]
pub async fn save_project_ignore_rules(ignore_rules: Vec<String>) -> Result<(), ErrorPayload> {
    let active = repodesk_core::projects::read_active_project().map_err(ErrorPayload::from)?;
    repodesk_core::projects::update_project_ignore_rules(&active, ignore_rules)
        .map(|_| ())
        .map_err(ErrorPayload::from)
}

#[tauri::command]
pub async fn get_project_file_token_estimates()
-> Result<Vec<repodesk_core::project_token_check::FileTokenEstimate>, ErrorPayload> {
    repodesk_core::project_token_check::get_project_file_token_estimates()
        .map_err(ErrorPayload::from)
}
