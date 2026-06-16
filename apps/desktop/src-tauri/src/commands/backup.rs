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

/// Restore the local SQLite database from a backup file. The current database is
/// snapshotted to a `pre-restore-*.sqlite` first so a mistaken restore is
/// recoverable.
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
    let _ = repodesk_core::persistence::backup_to(&safety);

    repodesk_core::persistence::restore_from(&src).map_err(|error| error.to_string())?;
    Ok(format!(
        "Restored from {}. Previous database saved to {}.",
        src.display(),
        safety.display()
    ))
}
