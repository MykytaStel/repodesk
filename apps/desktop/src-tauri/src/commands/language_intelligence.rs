use repodesk_core::language_intelligence::{
    LanguageIntelligenceSnapshot, active_language_intelligence_snapshot,
};

#[tauri::command]
pub fn language_intelligence_snapshot() -> Result<LanguageIntelligenceSnapshot, String> {
    active_language_intelligence_snapshot().map_err(|error| error.to_string())
}
