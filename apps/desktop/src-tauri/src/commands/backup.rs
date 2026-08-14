use std::path::PathBuf;

/// Back up the local SQLite database (memory, events, token ledger, action
/// history) to a timestamped file under `<REPODESK_HOME>/backups`. Returns the
/// backup path.
#[tauri::command]
pub fn backup_state() -> Result<String, String> {
    let paths =
        repodesk_core::paths::RepoDeskPaths::resolve().map_err(|error| error.to_string())?;
    let dest = paths
        .home
        .join("backups")
        .join(format!("repodesk-{}.sqlite", super::now_ms()));
    repodesk_core::persistence::backup_to(&dest).map_err(|error| error.to_string())?;
    Ok(dest.display().to_string())
}

/// Restore the local SQLite database from a validated RepoDesk backup.
///
/// A verified snapshot of the current database is mandatory before the restore
/// is attempted. The core restore protocol then validates and migrates a staged
/// candidate before replacing the live database, with an internal rollback if
/// promotion fails.
#[tauri::command]
pub fn restore_state(path: String) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Backup path is required".into());
    }
    let src = PathBuf::from(trimmed);

    let paths =
        repodesk_core::paths::RepoDeskPaths::resolve().map_err(|error| error.to_string())?;
    let safety = paths
        .home
        .join("backups")
        .join(format!("pre-restore-{}.sqlite", super::now_ms()));

    repodesk_core::persistence::backup_to(&safety).map_err(|error| {
        format!(
            "Restore cancelled because the current RepoDesk state could not be backed up safely: {error}"
        )
    })?;

    repodesk_core::persistence::restore_from(&src).map_err(|error| error.to_string())?;
    Ok(format!(
        "Restored from {}. Previous database saved to {}.",
        src.display(),
        safety.display()
    ))
}
