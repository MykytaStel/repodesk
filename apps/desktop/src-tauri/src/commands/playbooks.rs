//! Desktop bridge for user-authored playbooks (workflow shortcuts): list, save
//! (create/update), delete, and import. Backed by `playbooks.toml` in core; this
//! is read + the human mutations.

use repodesk_core::playbooks::{self, Playbook};

use super::ErrorPayload;

/// All configured playbooks (seeds defaults on first run).
#[tauri::command]
pub async fn playbooks_list() -> Result<Vec<Playbook>, ErrorPayload> {
    Ok(playbooks::list_playbooks()?)
}

/// Create or update a playbook; returns the full list after the change.
#[tauri::command]
pub async fn playbooks_save(playbook: Playbook) -> Result<Vec<Playbook>, ErrorPayload> {
    Ok(playbooks::save_playbook(playbook)?)
}

/// Delete a playbook by id; returns the full list after the change.
#[tauri::command]
pub async fn playbooks_delete(id: String) -> Result<Vec<Playbook>, ErrorPayload> {
    Ok(playbooks::delete_playbook(&id)?)
}

/// Import playbooks from a pasted TOML/JSON document; returns the merged list.
#[tauri::command]
pub async fn playbooks_import(document: String) -> Result<Vec<Playbook>, ErrorPayload> {
    Ok(playbooks::import_playbooks(&document)?)
}
