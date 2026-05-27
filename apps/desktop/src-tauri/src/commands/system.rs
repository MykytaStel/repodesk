use crate::store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvDiagnostic {
    pub openai_api_key_set: bool,
    pub gemini_api_key_set: bool,
    pub anthropic_api_key_set: bool,
}

#[tauri::command]
pub fn db_status() -> store::DbStatus {
    store::db_status()
}

#[tauri::command]
pub fn get_system_agents() -> Result<repodesk_core::agents::AgentsConfig, String> {
    repodesk_core::agents::ensure_agents_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_capabilities() -> Result<repodesk_core::capabilities::CapabilitiesConfig, String>
{
    repodesk_core::capabilities::ensure_capabilities_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_peripherals() -> Result<repodesk_core::peripherals::PeripheralsConfig, String> {
    repodesk_core::peripherals::ensure_peripherals_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_modules() -> Vec<repodesk_core::module_registry::BrainModule> {
    repodesk_core::module_registry::list_modules()
}
