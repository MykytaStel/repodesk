use rusqlite::{Connection, OptionalExtension, params};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use repodesk_core::credentials;

use super::types::{DbStatus, ProviderPreferences, ProviderSettings};

/// Legacy plaintext settings key ↔ canonical keychain credential key, for the
/// secrets that now live in the OS keychain instead of the SQLite settings table.
const SECRET_KEYS: &[(&str, &str)] = &[
    ("provider.anthropic_api_key", credentials::ANTHROPIC_API_KEY),
    ("provider.openai_api_key", credentials::OPENAI_API_KEY),
    ("provider.gemini_api_key", credentials::GEMINI_API_KEY),
];

/// Resolve a provider secret: keychain first, then the legacy plaintext column.
/// Empty string when neither holds it. Never errors — a keychain miss/failure
/// falls back to plaintext so the app keeps working pre-migration.
fn resolve_provider_key(connection: &Connection, setting_key: &str, cred_key: &str) -> String {
    if let Ok(Some(secret)) = credentials::keychain_secret(cred_key) {
        return secret;
    }
    get_string(connection, setting_key, "").unwrap_or_default()
}

/// Best-effort, idempotent migration of legacy plaintext API keys into the OS
/// keychain: store each non-empty plaintext key (unless the keychain already has
/// one), clear the plaintext column, then VACUUM so the old value can't linger
/// in freed pages. A box without a keychain leaves the plaintext fallback intact.
fn migrate_legacy_keys(connection: &Connection) {
    let mut migrated_any = false;
    for (setting_key, cred_key) in SECRET_KEYS {
        let plaintext = match get_setting(connection, setting_key) {
            Ok(Some(value)) if !value.trim().is_empty() => value,
            _ => continue,
        };
        // Don't clobber an existing keychain value; only store when absent.
        let needs_store = matches!(credentials::keychain_secret(cred_key), Ok(None));
        if needs_store && credentials::store_secret(cred_key, &plaintext).is_err() {
            continue; // Keychain unavailable — keep plaintext, retry next time.
        }
        if set_setting(connection, setting_key, "").is_ok() {
            migrated_any = true;
        }
    }
    if migrated_any {
        let _ = connection.execute_batch("VACUUM;");
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

    let preferred_patch_provider = canonical_provider_setting(&get_string(
        &connection,
        "provider.preferred_patch_provider",
        &defaults.preferred_patch_provider,
    )?);
    let preferred_compression_provider = canonical_provider_setting(&get_string(
        &connection,
        "provider.preferred_compression_provider",
        &defaults.preferred_compression_provider,
    )?);
    let preferred_review_provider = canonical_provider_setting(&get_string(
        &connection,
        "provider.preferred_review_provider",
        &defaults.preferred_review_provider,
    )?);

    // One-time, best-effort move of any legacy plaintext keys into the OS
    // keychain; clears the plaintext column + VACUUMs on success. A box without
    // a keychain (headless CI) silently keeps the plaintext fallback below.
    migrate_legacy_keys(&connection);

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
        anthropic_api_enabled: get_bool(
            &connection,
            "provider.anthropic_api_enabled",
            defaults.anthropic_api_enabled,
        )?,
        // Secrets resolve keychain-first, with the legacy plaintext column only
        // as a fallback until migration moves it into the keychain.
        anthropic_api_key: resolve_provider_key(
            &connection,
            "provider.anthropic_api_key",
            credentials::ANTHROPIC_API_KEY,
        ),
        openai_api_key: resolve_provider_key(
            &connection,
            "provider.openai_api_key",
            credentials::OPENAI_API_KEY,
        ),
        gemini_api_key: resolve_provider_key(
            &connection,
            "provider.gemini_api_key",
            credentials::GEMINI_API_KEY,
        ),
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
        preferred_patch_provider,
        preferred_compression_provider,
        preferred_review_provider,
        notes: get_string(&connection, "provider.notes", &defaults.notes)?,
    })
}

pub fn read_provider_preferences() -> Result<ProviderPreferences, String> {
    read_provider_settings().map(|settings| ProviderPreferences::from(&settings))
}

fn canonical_provider_setting(value: &str) -> String {
    match value.trim() {
        "codex" => "codex_cli".to_string(),
        other => other.to_string(),
    }
}

fn canonicalize_preferences(preferences: &mut ProviderPreferences) {
    preferences.preferred_patch_provider =
        canonical_provider_setting(&preferences.preferred_patch_provider);
    preferences.preferred_compression_provider =
        canonical_provider_setting(&preferences.preferred_compression_provider);
    preferences.preferred_review_provider =
        canonical_provider_setting(&preferences.preferred_review_provider);
}

fn persist_provider_preferences(
    connection: &Connection,
    preferences: &ProviderPreferences,
) -> Result<(), String> {
    set_setting(
        connection,
        "provider.ollama_enabled",
        &preferences.ollama_enabled.to_string(),
    )?;
    set_setting(connection, "provider.ollama_url", &preferences.ollama_url)?;
    set_setting(
        connection,
        "provider.ollama_model",
        &preferences.ollama_model,
    )?;
    set_setting(
        connection,
        "provider.lm_studio_enabled",
        &preferences.lm_studio_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.lm_studio_url",
        &preferences.lm_studio_url,
    )?;
    set_setting(
        connection,
        "provider.llamafile_enabled",
        &preferences.llamafile_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.llamafile_url",
        &preferences.llamafile_url,
    )?;
    set_setting(
        connection,
        "provider.localai_enabled",
        &preferences.localai_enabled.to_string(),
    )?;
    set_setting(connection, "provider.localai_url", &preferences.localai_url)?;
    set_setting(
        connection,
        "provider.chatgpt_enabled",
        &preferences.chatgpt_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.codex_enabled",
        &preferences.codex_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.gemini_enabled",
        &preferences.gemini_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.openai_api_enabled",
        &preferences.openai_api_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.openai_api_key_env_var",
        &preferences.openai_api_key_env_var,
    )?;
    set_setting(
        connection,
        "provider.gemini_api_enabled",
        &preferences.gemini_api_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.gemini_api_key_env_var",
        &preferences.gemini_api_key_env_var,
    )?;
    set_setting(
        connection,
        "provider.anthropic_api_enabled",
        &preferences.anthropic_api_enabled.to_string(),
    )?;
    set_setting(
        connection,
        "provider.allow_paid_agents",
        &preferences.allow_paid_agents.to_string(),
    )?;
    set_setting(
        connection,
        "provider.codex_quota_status",
        &preferences.codex_quota_status,
    )?;
    set_setting(
        connection,
        "provider.preferred_patch_provider",
        &preferences.preferred_patch_provider,
    )?;
    set_setting(
        connection,
        "provider.preferred_compression_provider",
        &preferences.preferred_compression_provider,
    )?;
    set_setting(
        connection,
        "provider.preferred_review_provider",
        &preferences.preferred_review_provider,
    )?;
    set_setting(connection, "provider.notes", &preferences.notes)?;
    Ok(())
}

pub fn save_provider_preferences(
    mut preferences: ProviderPreferences,
) -> Result<ProviderPreferences, String> {
    canonicalize_preferences(&mut preferences);
    validate_provider_preferences(&preferences)?;
    let connection = open_db()?;
    persist_provider_preferences(&connection, &preferences)?;
    Ok(preferences)
}

#[cfg(test)]
pub fn validate_provider_settings(settings: &ProviderSettings) -> Result<(), String> {
    validate_provider_preferences(&ProviderPreferences::from(settings))?;
    validate_api_key("Anthropic API key", &settings.anthropic_api_key)?;
    validate_api_key("OpenAI API key", &settings.openai_api_key)?;
    validate_api_key("Gemini API key", &settings.gemini_api_key)?;
    Ok(())
}

pub fn validate_provider_preferences(settings: &ProviderPreferences) -> Result<(), String> {
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
                || provider.as_str() == "codex_cli"
                || provider.as_str() == "gemini"
                || provider.as_str() == "openai_api"
                || provider.as_str() == "anthropic_api"
                || provider.as_str() == "gemini_api"
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
        "ollama" | "lm_studio" | "chatgpt" | "codex" | "codex_cli" | "gemini" | "manual"
        | "llamafile" | "localai" | "openai_api" | "anthropic_api" | "gemini_api" => Ok(()),
        _ => Err(format!(
            "{label} must be one of: ollama, lm_studio, chatgpt, codex_cli, gemini, manual, llamafile, localai, openai_api, anthropic_api, gemini_api"
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

/// Test-only validation for the legacy/internal credential fields retained for
/// plaintext-to-keychain migration compatibility. Current preference IPC cannot
/// carry these fields.
#[cfg(test)]
fn validate_api_key(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.len() > 400 {
        return Err(format!("{label} is too long"));
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(format!("{label} must not contain control characters"));
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
        assert!(validate_provider_preferences(&ProviderPreferences::default()).is_ok());
    }

    #[test]
    fn canonicalizes_legacy_codex_provider_setting() {
        assert_eq!(canonical_provider_setting("codex"), "codex_cli");
        assert_eq!(canonical_provider_setting("ollama"), "ollama");
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

    #[test]
    fn preference_validation_has_no_secret_input() {
        let preferences = ProviderPreferences {
            notes: "api_key=must-not-be-here".into(),
            ..ProviderPreferences::default()
        };

        assert!(validate_provider_preferences(&preferences).is_err());
    }
}
