use std::sync::LazyLock;

use repodesk_core::language_intelligence::{
    LanguageIntelligenceSnapshot, active_language_intelligence_snapshot,
};
use serde::Deserialize;
use serde_json::{Value, to_value};
use tauri::AppHandle;

#[path = "../language_server.rs"]
mod language_server;

use language_server::LanguageServerManager;

static LANGUAGE_SERVER_MANAGER: LazyLock<LanguageServerManager> =
    LazyLock::new(LanguageServerManager::default);

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LanguageIntelligenceAction {
    Status,
    SyncDocument {
        path: String,
        language: String,
        text: String,
    },
    CloseDocument {
        path: String,
    },
    Hover {
        path: String,
        text: String,
        line: u32,
        column: u32,
    },
    Definition {
        path: String,
        text: String,
        line: u32,
        column: u32,
    },
    References {
        path: String,
        text: String,
        line: u32,
        column: u32,
    },
    DocumentSymbols {
        path: String,
        text: String,
    },
    Stop,
}

/// One compatibility transport owns both the side-effect-free RD2-15 discovery
/// snapshot and RD2-15b live language actions. Existing callers that omit
/// `action` receive the exact discovery shape introduced in #82.
#[tauri::command]
pub async fn language_intelligence_snapshot(
    app: AppHandle,
    action: Option<LanguageIntelligenceAction>,
) -> Result<Value, String> {
    let Some(action) = action else {
        let snapshot: LanguageIntelligenceSnapshot =
            active_language_intelligence_snapshot().map_err(|error| error.to_string())?;
        return to_value(snapshot).map_err(|error| error.to_string());
    };

    match action {
        LanguageIntelligenceAction::Status => {
            to_value(LANGUAGE_SERVER_MANAGER.status()).map_err(|error| error.to_string())
        }
        LanguageIntelligenceAction::CloseDocument { path } => {
            LANGUAGE_SERVER_MANAGER.close_document(&path)?;
            Ok(Value::Null)
        }
        LanguageIntelligenceAction::Stop => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            tauri::async_runtime::spawn_blocking(move || manager.stop())
                .await
                .map_err(|error| format!("Language server shutdown worker failed: {error}"))?;
            Ok(Value::Null)
        }
        LanguageIntelligenceAction::SyncDocument {
            path,
            language,
            text,
        } => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                manager.sync_document(&app, &path, &language, &text)
            })
            .await
            .map_err(|error| format!("Language server worker failed: {error}"))??;
            to_value(result).map_err(|error| error.to_string())
        }
        LanguageIntelligenceAction::Hover {
            path,
            text,
            line,
            column,
        } => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                manager.hover(&app, &path, &text, line, column)
            })
            .await
            .map_err(|error| format!("Language hover worker failed: {error}"))??;
            to_value(result).map_err(|error| error.to_string())
        }
        LanguageIntelligenceAction::Definition {
            path,
            text,
            line,
            column,
        } => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                manager.definition(&app, &path, &text, line, column)
            })
            .await
            .map_err(|error| format!("Language definition worker failed: {error}"))??;
            to_value(result).map_err(|error| error.to_string())
        }
        LanguageIntelligenceAction::References {
            path,
            text,
            line,
            column,
        } => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                manager.references(&app, &path, &text, line, column)
            })
            .await
            .map_err(|error| format!("Language references worker failed: {error}"))??;
            to_value(result).map_err(|error| error.to_string())
        }
        LanguageIntelligenceAction::DocumentSymbols { path, text } => {
            let manager = (*LANGUAGE_SERVER_MANAGER).clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                manager.document_symbols(&app, &path, &text)
            })
            .await
            .map_err(|error| format!("Language symbols worker failed: {error}"))??;
            to_value(result).map_err(|error| error.to_string())
        }
    }
}
