use super::*;
use crate::store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub id: String,
    pub provider: String,
    pub available: bool,
    pub loaded: Option<bool>,
    pub context_window: Option<usize>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub auth_status: String,
    pub reachability: String,
    pub models: Vec<ModelStatus>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHealthSnapshot {
    pub generated_at_ms: u128,
    pub providers: Vec<ProviderHealth>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ProviderFetchError {
    pub status: Option<u16>,
    pub summary: String,
}

fn http_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .build();
    config.into()
}

fn request_json(
    url: &str,
    headers: &[(&str, &str)],
) -> Result<serde_json::Value, ProviderFetchError> {
    let agent = http_agent();
    let mut request = agent.get(url).header("accept", "application/json");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }

    match request.call() {
        Ok(mut response) => {
            let status = response.status();
            let body_str =
                response
                    .body_mut()
                    .read_to_string()
                    .map_err(|error| ProviderFetchError {
                        status: Some(status.as_u16()),
                        summary: error.to_string(),
                    })?;

            if status.as_u16() >= 400 {
                let summary = if body_str.trim().is_empty() {
                    format!("HTTP {}", status.as_u16())
                } else {
                    format!(
                        "HTTP {}: {}",
                        status.as_u16(),
                        body_str.chars().take(240).collect::<String>()
                    )
                };
                return Err(ProviderFetchError {
                    status: Some(status.as_u16()),
                    summary,
                });
            }

            serde_json::from_str(&body_str).map_err(|error| ProviderFetchError {
                status: Some(status.as_u16()),
                summary: format!("Invalid JSON response: {error}"),
            })
        }
        Err(error) => Err(ProviderFetchError {
            status: None,
            summary: error.to_string(),
        }),
    }
}

fn model_status(provider: &str, id: String, notes: Option<String>) -> ModelStatus {
    ModelStatus {
        id,
        provider: provider.to_string(),
        available: true,
        loaded: None,
        context_window: None,
        notes,
    }
}

fn disabled_provider(id: &str, label: &str) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: false,
        auth_status: "disabled".into(),
        reachability: "disabled".into(),
        models: Vec::new(),
        error_summary: None,
    }
}

fn provider_error(
    id: &str,
    label: &str,
    auth_status: &str,
    reachability: &str,
    error: String,
) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: true,
        auth_status: auth_status.into(),
        reachability: reachability.into(),
        models: Vec::new(),
        error_summary: Some(truncate_text(&error, 500)),
    }
}

fn provider_working(
    id: &str,
    label: &str,
    auth_status: &str,
    models: Vec<ModelStatus>,
) -> ProviderHealth {
    ProviderHealth {
        id: id.into(),
        label: label.into(),
        enabled: true,
        auth_status: auth_status.into(),
        reachability: "working".into(),
        models,
        error_summary: None,
    }
}

/// Resolve a provider key: an in-app stored key wins; otherwise fall back to the
/// named environment variable. Returns the key and a label describing its source
/// for the "missing" hint.
fn resolve_key(stored: &str, env_var: &str) -> (Option<String>, String) {
    let stored = stored.trim();
    if !stored.is_empty() {
        return (Some(stored.to_string()), "in-app key".to_string());
    }
    match env::var(env_var) {
        Ok(value) if !value.trim().is_empty() => (Some(value), format!("${env_var}")),
        _ => (None, format!("${env_var}")),
    }
}

fn join_url(base: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base.trim().trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

fn ollama_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.ollama_enabled {
        return disabled_provider("ollama", "Ollama");
    }

    match request_json(&join_url(&settings.ollama_url, "/api/tags"), &[]) {
        Ok(value) => {
            let models = value
                .get("models")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("model")
                                .or_else(|| item.get("name"))
                                .and_then(|value| value.as_str())
                                .map(|name| {
                                    model_status(
                                        "ollama",
                                        name.to_string(),
                                        item.get("details")
                                            .and_then(|details| details.get("parameter_size"))
                                            .and_then(|value| value.as_str())
                                            .map(|value| format!("parameters: {value}")),
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("ollama", "Ollama", "not_required", models)
        }
        Err(error) => provider_error(
            "ollama",
            "Ollama",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn open_ai_compatible_health(
    id: &str,
    label: &str,
    enabled: bool,
    url: &str,
    suffix: &str,
    notes: &str,
) -> ProviderHealth {
    if !enabled {
        return disabled_provider(id, label);
    }

    match request_json(&join_url(url, suffix), &[]) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id")
                                .and_then(|value| value.as_str())
                                .map(|model_id| {
                                    model_status(id, model_id.to_string(), Some(notes.to_string()))
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working(id, label, "not_required", models)
        }
        Err(error) => provider_error(id, label, "not_required", "unreachable", error.summary),
    }
}

fn lm_studio_health(settings: &store::ProviderSettings) -> ProviderHealth {
    open_ai_compatible_health(
        "lm_studio",
        "LM Studio",
        settings.lm_studio_enabled,
        &settings.lm_studio_url,
        "/v1/models",
        "visible to LM Studio OpenAI-compatible server",
    )
}

fn llamafile_health(settings: &store::ProviderSettings) -> ProviderHealth {
    open_ai_compatible_health(
        "llamafile",
        "Llamafile",
        settings.llamafile_enabled,
        &settings.llamafile_url,
        "/v1/models",
        "visible to Llamafile server",
    )
}

fn localai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    open_ai_compatible_health(
        "localai",
        "LocalAI",
        settings.localai_enabled,
        &settings.localai_url,
        "/v1/models",
        "visible to LocalAI server",
    )
}

fn openai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.openai_api_enabled {
        return disabled_provider("openai", "OpenAI API");
    }

    let (key, source) = resolve_key(
        &settings.openai_api_key,
        settings.openai_api_key_env_var.trim(),
    );
    let Some(api_key) = key else {
        return provider_error(
            "openai",
            "OpenAI API",
            "auth_missing",
            "auth_missing",
            format!(
                "Add an OpenAI key in Settings or set {source} to enable live model discovery."
            ),
        );
    };

    let authorization = format!("Bearer {api_key}");
    match request_json(
        "https://api.openai.com/v1/models",
        &[("authorization", authorization.as_str())],
    ) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "openai",
                                    id.to_string(),
                                    item.get("owned_by")
                                        .and_then(|value| value.as_str())
                                        .map(|owner| format!("owned by {owner}")),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("openai", "OpenAI API", "configured", models)
        }
        Err(error) => {
            let reachability = match error.status {
                Some(401 | 403) => "auth_missing",
                Some(429) => "rate_limited",
                _ => "unreachable",
            };
            let auth_status = if reachability == "auth_missing" {
                "auth_missing"
            } else {
                "configured"
            };
            provider_error(
                "openai",
                "OpenAI API",
                auth_status,
                reachability,
                error.summary,
            )
        }
    }
}

fn gemini_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.gemini_api_enabled {
        return disabled_provider("gemini", "Gemini API");
    }

    let (key, source) = resolve_key(
        &settings.gemini_api_key,
        settings.gemini_api_key_env_var.trim(),
    );
    let Some(api_key) = key else {
        return provider_error(
            "gemini",
            "Gemini API",
            "auth_missing",
            "auth_missing",
            format!("Add a Gemini key in Settings or set {source} to enable live model discovery."),
        );
    };

    match request_json(
        "https://generativelanguage.googleapis.com/v1beta/models",
        &[("x-goog-api-key", api_key.as_str())],
    ) {
        Ok(value) => {
            let models = value
                .get("models")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("name")
                                .and_then(|value| value.as_str())
                                .map(|name| {
                                    let id = name.strip_prefix("models/").unwrap_or(name);
                                    model_status(
                                        "gemini",
                                        id.to_string(),
                                        item.get("displayName")
                                            .and_then(|value| value.as_str())
                                            .map(str::to_string),
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("gemini", "Gemini API", "configured", models)
        }
        Err(error) => {
            let reachability = match error.status {
                Some(401 | 403) => "auth_missing",
                Some(429) => "rate_limited",
                _ => "unreachable",
            };
            let auth_status = if reachability == "auth_missing" {
                "auth_missing"
            } else {
                "configured"
            };
            provider_error(
                "gemini",
                "Gemini API",
                auth_status,
                reachability,
                error.summary,
            )
        }
    }
}

fn anthropic_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.anthropic_api_enabled {
        return disabled_provider("anthropic", "Anthropic API");
    }

    let (key, source) = resolve_key(&settings.anthropic_api_key, "ANTHROPIC_API_KEY");
    let Some(api_key) = key else {
        return provider_error(
            "anthropic",
            "Anthropic API",
            "auth_missing",
            "auth_missing",
            format!(
                "Add an Anthropic key in Settings or set {source} to enable live model discovery."
            ),
        );
    };

    match request_json(
        "https://api.anthropic.com/v1/models",
        &[
            ("x-api-key", api_key.as_str()),
            ("anthropic-version", "2023-06-01"),
        ],
    ) {
        Ok(value) => {
            let models = value
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id").and_then(|value| value.as_str()).map(|id| {
                                model_status(
                                    "anthropic",
                                    id.to_string(),
                                    item.get("display_name")
                                        .and_then(|value| value.as_str())
                                        .map(str::to_string),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("anthropic", "Anthropic API", "configured", models)
        }
        Err(error) => {
            let reachability = match error.status {
                Some(401 | 403) => "auth_missing",
                Some(429) => "rate_limited",
                _ => "unreachable",
            };
            let auth_status = if reachability == "auth_missing" {
                "auth_missing"
            } else {
                "configured"
            };
            provider_error(
                "anthropic",
                "Anthropic API",
                auth_status,
                reachability,
                error.summary,
            )
        }
    }
}

pub(crate) fn model_health_from_settings(
    settings: &store::ProviderSettings,
) -> ModelHealthSnapshot {
    let s = settings.clone();

    // Use crossbeam or standard threads for parallel execution since we are in a synchronous context,
    // but in Tauri commands we typically use async. If this function runs in the blocking pool, it's fine.
    let t_ollama = std::thread::spawn({
        let s = s.clone();
        move || ollama_health(&s)
    });
    let t_lm = std::thread::spawn({
        let s = s.clone();
        move || lm_studio_health(&s)
    });
    let t_llamafile = std::thread::spawn({
        let s = s.clone();
        move || llamafile_health(&s)
    });
    let t_localai = std::thread::spawn({
        let s = s.clone();
        move || localai_health(&s)
    });
    let t_openai = std::thread::spawn({
        let s = s.clone();
        move || openai_health(&s)
    });
    let t_gemini = std::thread::spawn({
        let s = s.clone();
        move || gemini_health(&s)
    });
    let t_anthropic = std::thread::spawn({
        let s = s.clone();
        move || anthropic_health(&s)
    });

    let providers = vec![
        t_ollama
            .join()
            .unwrap_or_else(|_| disabled_provider("ollama", "Ollama")),
        t_lm.join()
            .unwrap_or_else(|_| disabled_provider("lm_studio", "LM Studio")),
        t_llamafile
            .join()
            .unwrap_or_else(|_| disabled_provider("llamafile", "Llamafile")),
        t_localai
            .join()
            .unwrap_or_else(|_| disabled_provider("localai", "LocalAI")),
        t_openai
            .join()
            .unwrap_or_else(|_| disabled_provider("openai", "OpenAI API")),
        t_gemini
            .join()
            .unwrap_or_else(|_| disabled_provider("gemini", "Gemini API")),
        t_anthropic
            .join()
            .unwrap_or_else(|_| disabled_provider("anthropic", "Anthropic API")),
    ];
    let mut warnings = Vec::new();

    if providers
        .iter()
        .any(|provider| provider.reachability == "auth_missing")
    {
        warnings.push(
            "Some API providers are enabled but missing environment-based credentials.".into(),
        );
    }

    if providers
        .iter()
        .filter(|provider| provider.enabled)
        .all(|provider| provider.reachability != "working")
    {
        warnings.push("No enabled model provider is currently reachable.".into());
    }

    ModelHealthSnapshot {
        generated_at_ms: now_ms(),
        providers,
        warnings,
    }
}

pub(crate) fn build_model_health_snapshot() -> ModelHealthSnapshot {
    let settings = store::read_provider_settings().unwrap_or_default();
    model_health_from_settings(&settings)
}

#[tauri::command]
pub async fn model_health_snapshot() -> ModelHealthSnapshot {
    tauri::async_runtime::spawn_blocking(build_model_health_snapshot)
        .await
        .unwrap_or_else(|_| ModelHealthSnapshot {
            generated_at_ms: now_ms(),
            providers: vec![],
            warnings: vec!["Internal async task failure while fetching model health".into()],
        })
}

#[tauri::command]
pub async fn refresh_model_health() -> ModelHealthSnapshot {
    tauri::async_runtime::spawn_blocking(build_model_health_snapshot)
        .await
        .unwrap_or_else(|_| ModelHealthSnapshot {
            generated_at_ms: now_ms(),
            providers: vec![],
            warnings: vec!["Internal async task failure while refreshing models".into()],
        })
}
