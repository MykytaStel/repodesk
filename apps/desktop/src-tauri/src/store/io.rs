use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{DbStatus, ProviderSettings};

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
        lm_studio_enabled: get_bool(
            &connection,
            "provider.lm_studio_enabled",
            defaults.lm_studio_enabled,
        )?,
        lm_studio_url: get_string(
            &connection,
            "provider.lm_studio_url",
            &defaults.lm_studio_url,
        )?,
        llamafile_enabled: get_bool(
            &connection,
            "provider.llamafile_enabled",
            defaults.llamafile_enabled,
        )?,
        llamafile_url: get_string(
            &connection,
            "provider.llamafile_url",
            &defaults.llamafile_url,
        )?,
        localai_enabled: get_bool(
            &connection,
            "provider.localai_enabled",
            defaults.localai_enabled,
        )?,
        localai_url: get_string(&connection, "provider.localai_url", &defaults.localai_url)?,
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
        openai_api_enabled: get_bool(
            &connection,
            "provider.openai_api_enabled",
            defaults.openai_api_enabled,
        )?,
        openai_api_key_env_var: get_string(
            &connection,
            "provider.openai_api_key_env_var",
            &defaults.openai_api_key_env_var,
        )?,
        gemini_api_enabled: get_bool(
            &connection,
            "provider.gemini_api_enabled",
            defaults.gemini_api_enabled,
        )?,
        gemini_api_key_env_var: get_string(
            &connection,
            "provider.gemini_api_key_env_var",
            &defaults.gemini_api_key_env_var,
        )?,
        allow_paid_agents: get_bool(
            &connection,
            "provider.allow_paid_agents",
            defaults.allow_paid_agents,
        )?,
        codex_quota_status: get_string(
            &connection,
            "provider.codex_quota_status",
            &defaults.codex_quota_status,
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
        "provider.lm_studio_enabled",
        &settings.lm_studio_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.lm_studio_url",
        &settings.lm_studio_url,
    )?;
    set_setting(
        &connection,
        "provider.llamafile_enabled",
        &settings.llamafile_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.llamafile_url",
        &settings.llamafile_url,
    )?;
    set_setting(
        &connection,
        "provider.localai_enabled",
        &settings.localai_enabled.to_string(),
    )?;
    set_setting(&connection, "provider.localai_url", &settings.localai_url)?;
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
        "provider.openai_api_enabled",
        &settings.openai_api_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.openai_api_key_env_var",
        &settings.openai_api_key_env_var,
    )?;
    set_setting(
        &connection,
        "provider.gemini_api_enabled",
        &settings.gemini_api_enabled.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.gemini_api_key_env_var",
        &settings.gemini_api_key_env_var,
    )?;
    set_setting(
        &connection,
        "provider.allow_paid_agents",
        &settings.allow_paid_agents.to_string(),
    )?;
    set_setting(
        &connection,
        "provider.codex_quota_status",
        &settings.codex_quota_status,
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
    validate_local_url("LM Studio URL", &settings.lm_studio_url)?;
    validate_local_url("Llamafile URL", &settings.llamafile_url)?;
    validate_local_url("LocalAI URL", &settings.localai_url)?;
    validate_safe_text("Ollama model", &settings.ollama_model, 80)?;
    validate_env_var("OpenAI API key env var", &settings.openai_api_key_env_var)?;
    validate_env_var("Gemini API key env var", &settings.gemini_api_key_env_var)?;
    validate_codex_quota_status(&settings.codex_quota_status)?;
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

fn validate_codex_quota_status(value: &str) -> Result<(), String> {
    match value {
        "unknown" | "available" | "limited" | "empty" => Ok(()),
        _ => Err("Codex quota status must be one of: unknown, available, limited, empty".into()),
    }
}

fn validate_provider(label: &str, value: &str) -> Result<(), String> {
    match value {
        "ollama" | "chatgpt" | "codex" | "gemini" | "manual" | "llamafile" | "localai" => Ok(()),
        _ => Err(format!(
            "{label} must be one of: ollama, chatgpt, codex, gemini, manual, llamafile, localai"
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

fn validate_env_var(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return Err(format!("{label} is required"));
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(format!(
            "{label} may only contain ASCII letters, numbers, and underscore"
        ));
    }

    if trimmed.contains("KEY=") || trimmed.contains("sk-") || trimmed.contains('\0') {
        return Err(format!(
            "{label} must be an environment variable name, not a secret"
        ));
    }

    Ok(())
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
    use super::*;

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

    #[test]
    fn rejects_invalid_codex_quota_status() {
        let settings = ProviderSettings {
            codex_quota_status: "scrape-browser".to_string(),
            ..ProviderSettings::default()
        };

        assert!(validate_provider_settings(&settings).is_err());
    }

    #[test]
    fn rejects_api_key_value_in_env_var_field() {
        let settings = ProviderSettings {
            openai_api_key_env_var: "sk-example-secret".to_string(),
            ..ProviderSettings::default()
        };

        assert!(validate_provider_settings(&settings).is_err());
    }
}
