use super::{
    run_cli, run_cli_str, validate_path, validate_short_id, CommandResult, ProjectAddInput,
};

#[tauri::command]
pub fn project_info() -> CommandResult {
    run_cli_str(&["project", "info"])
}

#[tauri::command]
pub fn project_list() -> CommandResult {
    run_cli_str(&["project", "list"])
}

#[tauri::command]
pub fn project_use(name: String) -> Result<CommandResult, String> {
    validate_short_id("Project name", &name)?;
    Ok(run_cli(&[
        "project".into(),
        "use".into(),
        name.trim().into(),
    ]))
}

#[tauri::command]
pub fn project_add(input: ProjectAddInput) -> Result<CommandResult, String> {
    validate_short_id("Project name", &input.name)?;
    validate_path(&input.path)?;
    validate_short_id("Project type", &input.project_type)?;

    if let Some(language) = &input.main_language {
        if !language.trim().is_empty() {
            validate_short_id("Main language", language)?;
        }
    }

    let mut args = vec![
        "project".into(),
        "add".into(),
        input.name.trim().into(),
        input.path.trim().into(),
        "--type".into(),
        input.project_type.trim().into(),
    ];

    if let Some(language) = input.main_language {
        let trimmed = language.trim();
        if !trimmed.is_empty() {
            args.push("--main-language".into());
            args.push(trimmed.into());
        }
    }

    Ok(run_cli(&args))
}

#[tauri::command]
pub fn get_active_project_config() -> Result<repodesk_core::projects::ProjectConfig, String> {
    repodesk_core::projects::get_active_project().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project_ignore_rules(ignore_rules: Vec<String>) -> Result<(), String> {
    let active = repodesk_core::projects::read_active_project().map_err(|e| e.to_string())?;
    repodesk_core::projects::update_project_ignore_rules(&active, ignore_rules)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project_file_token_estimates(
) -> Result<Vec<repodesk_core::project_token_check::FileTokenEstimate>, String> {
    repodesk_core::project_token_check::get_project_file_token_estimates()
        .map_err(|e| e.to_string())
}
