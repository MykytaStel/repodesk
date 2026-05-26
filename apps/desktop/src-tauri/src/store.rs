use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStatus {
    pub path: String,
    pub exists: bool,
    pub ok: bool,
    pub tables: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub ollama_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: String,
    pub chatgpt_enabled: bool,
    pub codex_enabled: bool,
    pub gemini_enabled: bool,
    pub allow_paid_agents: bool,
    pub preferred_patch_provider: String,
    pub preferred_compression_provider: String,
    pub preferred_review_provider: String,
    pub notes: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            ollama_enabled: true,
            ollama_url: "http://127.0.0.1:11434".to_string(),
            ollama_model: "llama3.1".to_string(),
            chatgpt_enabled: true,
            codex_enabled: true,
            gemini_enabled: false,
            allow_paid_agents: true,
            preferred_patch_provider: "codex".to_string(),
            preferred_compression_provider: "ollama".to_string(),
            preferred_review_provider: "chatgpt".to_string(),
            notes: "Local-first by default. Paid agents should receive bounded smart context only."
                .to_string(),
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn db_path() -> PathBuf {
    home_dir().join(".repodesk").join("repodesk.db")
}

fn open_db() -> Result<Connection, String> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );
            "#,
        )
        .map_err(|error| error.to_string())?;

    Ok(connection)
}

fn get_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn set_setting(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO settings(key, value, updated_at_ms) VALUES(?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            params![key, value, now_ms() as i64],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn get_bool(connection: &Connection, key: &str, fallback: bool) -> Result<bool, String> {
    match get_setting(connection, key)? {
        Some(value) => Ok(value == "true"),
        None => Ok(fallback),
    }
}

fn get_string(connection: &Connection, key: &str, fallback: &str) -> Result<String, String> {
    Ok(get_setting(connection, key)?.unwrap_or_else(|| fallback.to_string()))
}

pub fn db_status() -> DbStatus {
    let path = db_path();
    let exists = path.exists();

    match open_db() {
        Ok(connection) => {
            let tables = connection
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .and_then(|mut statement| {
                    statement
                        .query_map([], |row| row.get::<_, String>(0))?
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or_default();

            DbStatus {
                path: path.display().to_string(),
                exists,
                ok: true,
                tables,
                error: None,
            }
        }
        Err(error) => DbStatus {
            path: path.display().to_string(),
            exists,
            ok: false,
            tables: Vec::new(),
            error: Some(error),
        },
    }
}

pub fn read_provider_settings() -> Result<ProviderSettings, String> {
    let connection = open_db()?;
    let defaults = ProviderSettings::default();

    Ok(ProviderSettings {
        ollama_enabled: get_bool(
            &connection,
            "provider.ollama_enabled",
            defaults.ollama_enabled,
        )?,
        ollama_url: get_string(&connection, "provider.ollama_url", &defaults.ollama_url)?,
        ollama_model: get_string(&connection, "provider.ollama_model", &defaults.ollama_model)?,
        chatgpt_enabled: get_bool(
            &connection,
            "provider.chatgpt_enabled",
            defaults.chatgpt_enabled,
        )?,
        codex_enabled: get_bool(
            &connection,
            "provider.codex_enabled",
            defaults.codex_enabled,
        )?,
        gemini_enabled: get_bool(
            &connection,
            "provider.gemini_enabled",
            defaults.gemini_enabled,
        )?,
        allow_paid_agents: get_bool(
            &connection,
            "provider.allow_paid_agents",
            defaults.allow_paid_agents,
        )?,
        preferred_patch_provider: get_string(
            &connection,
            "provider.preferred_patch_provider",
            &defaults.preferred_patch_provider,
        )?,
        preferred_compression_provider: get_string(
            &connection,
            "provider.preferred_compression_provider",
            &defaults.preferred_compression_provider,
        )?,
        preferred_review_provider: get_string(
            &connection,
            "provider.preferred_review_provider",
            &defaults.preferred_review_provider,
        )?,
        notes: get_string(&connection, "provider.notes", &defaults.notes)?,
    })
}

pub fn save_provider_settings(settings: ProviderSettings) -> Result<ProviderSettings, String> {
    validate_provider_settings(&settings)?;
    let connection = open_db()?;

    set_setting(
        &connection,
        "provider.ollama_enabled",
        &settings.ollama_enabled.to_string(),
    )?;
    set_setting(&connection, "provider.ollama_url", &settings.ollama_url)?;
    set_setting(&connection, "provider.ollama_model", &settings.ollama_model)?;
    set_setting(
        &connection,
        "provider.chatgpt_enabled",
        &settings.chatgpt_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.codex_enabled",
        &settings.codex_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.gemini_enabled",
        &settings.gemini_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.allow_paid_agents",
        &settings.allow_paid_agents.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.preferred_patch_provider",
        &settings.preferred_patch_provider,
    )?;
    set_setting(
        &connection,
        "provider.preferred_compression_provider",
        &settings.preferred_compression_provider,
    )?;
    set_setting(
        &connection,
        "provider.preferred_review_provider",
        &settings.preferred_review_provider,
    )?;
    set_setting(&connection, "provider.notes", &settings.notes)?;

    Ok(settings)
}

pub fn validate_provider_settings(settings: &ProviderSettings) -> Result<(), String> {
    validate_local_url("Ollama URL", &settings.ollama_url)?;
    validate_safe_text("Ollama model", &settings.ollama_model, 80)?;
    validate_safe_text("Notes", &settings.notes, 1_000)?;

    validate_provider(
        "Preferred patch provider",
        &settings.preferred_patch_provider,
    )?;
    validate_provider(
        "Preferred compression provider",
        &settings.preferred_compression_provider,
    )?;
    validate_provider(
        "Preferred review provider",
        &settings.preferred_review_provider,
    )?;

    if !settings.allow_paid_agents
        && [
            &settings.preferred_patch_provider,
            &settings.preferred_compression_provider,
            &settings.preferred_review_provider,
        ]
        .iter()
        .any(|provider| {
            provider.as_str() == "chatgpt"
                || provider.as_str() == "codex"
                || provider.as_str() == "gemini"
        })
    {
        return Err(
            "Paid agents are disabled, but one of the preferred providers is paid. Use ollama or manual."
                .to_string(),
        );
    }

    Ok(())
}

fn validate_provider(label: &str, value: &str) -> Result<(), String> {
    match value {
        "ollama" | "chatgpt" | "codex" | "gemini" | "manual" => Ok(()),
        _ => Err(format!(
            "{label} must be one of: ollama, chatgpt, codex, gemini, manual"
        )),
    }
}

fn validate_local_url(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.len() > 160 || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(format!("{label} is not safe"));
    }

    if trimmed.starts_with("http://127.0.0.1") || trimmed.starts_with("http://localhost") {
        return Ok(());
    }

    Err(format!(
        "{label} must be local-only: http://127.0.0.1... or http://localhost..."
    ))
}

fn validate_safe_text(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.len() > max_len || value.contains('\0') {
        return Err(format!("{label} is not safe"));
    }

    if value.contains("-----BEGIN") || value.to_lowercase().contains("api_key") {
        return Err(format!("{label} must not contain secrets"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_provider_settings, ProviderSettings};

    #[test]
    fn rejects_remote_ollama_url() {
        let settings = ProviderSettings {
            ollama_url: "https://example.com".to_string(),
            ..ProviderSettings::default()
        };

        assert!(validate_provider_settings(&settings).is_err());
    }

    #[test]
    fn rejects_paid_provider_when_paid_agents_disabled() {
        let settings = ProviderSettings {
            allow_paid_agents: false,
            preferred_review_provider: "chatgpt".to_string(),
            ..ProviderSettings::default()
        };

        assert!(validate_provider_settings(&settings).is_err());
    }

    #[test]
    fn accepts_local_first_defaults() {
        assert!(validate_provider_settings(&ProviderSettings::default()).is_ok());
    }
}
