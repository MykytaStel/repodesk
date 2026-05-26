use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Serialize)]
struct LocalStateStatus {
    repodesk_home: String,
    database_path: String,
    database_exists: bool,
    mode: String,
}

#[tauri::command]
fn dashboard_snapshot() -> Result<serde_json::Value, String> {
    let snapshot = repodesk_core::dashboard::build_dashboard_snapshot()
        .map_err(|err| format!("failed to build dashboard snapshot: {err}"))?;

    serde_json::to_value(snapshot).map_err(|err| format!("failed to serialize dashboard snapshot: {err}"))
}

#[tauri::command]
fn security_audit_text() -> Result<String, String> {
    let audit = repodesk_core::security::audit_security_policy()
        .map_err(|err| format!("failed to audit security policy: {err}"))?;

    Ok(repodesk_core::security::format_security_audit(&audit))
}

#[tauri::command]
fn local_state_status() -> Result<LocalStateStatus, String> {
    let home = dirs::home_dir().ok_or_else(|| "home directory not found".to_string())?;
    let repodesk_home = home.join(".repodesk");
    let database_path = find_database_path(&repodesk_home);

    Ok(LocalStateStatus {
        repodesk_home: repodesk_home.display().to_string(),
        database_exists: database_path.exists(),
        database_path: database_path.display().to_string(),
        mode: "desktop-local-only".to_string(),
    })
}

fn find_database_path(repodesk_home: &PathBuf) -> PathBuf {
    let candidates = [
        repodesk_home.join("repodesk.sqlite"),
        repodesk_home.join("repodesk.db"),
        repodesk_home.join("db/repodesk.sqlite"),
    ];

    candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .unwrap_or_else(|| repodesk_home.join("repodesk.sqlite"))
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dashboard_snapshot,
            security_audit_text,
            local_state_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoDesk desktop app");
}

#[cfg(test)]
mod tests {
    use super::find_database_path;
    use std::path::PathBuf;

    #[test]
    fn default_database_path_is_inside_repodesk_home() {
        let home = PathBuf::from("/tmp/repodesk-test-home");
        let path = find_database_path(&home);
        assert!(path.ends_with("repodesk.sqlite"));
    }
}
