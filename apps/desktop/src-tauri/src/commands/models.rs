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

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_secs(3))
        .timeout_write(Duration::from_secs(3))
        .build()
}

fn request_json(url: &str, headers: &[(&str, &str)]) -> Result<serde_json::Value, HttpJsonError> {
    let agent = http_agent();
    let mut request = agent.get(url).set("accept", "application/json");
    for (key, value) in headers {
        request = request.set(key, value);
    }

    match request.call() {
        Ok(response) => response.into_json().map_err(|error| HttpJsonError {
            status: None,
            summary: format!("Invalid JSON response: {error}"),
        }),
        Err(ureq::Error::Status(code, response)) => {
            let body = response
                .into_string()
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect::<String>();
            Err(HttpJsonError {
                status: Some(code),
                summary: if body.trim().is_empty() {
                    format!("HTTP {code}")
                } else {
                    format!("HTTP {code}: {body}")
                },
            })
        }
        Err(error) => Err(HttpJsonError {
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

fn lm_studio_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.lm_studio_enabled {
        return disabled_provider("lm_studio", "LM Studio");
    }

    match request_json(&join_url(&settings.lm_studio_url, "/v1/models"), &[]) {
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
                                    "lm_studio",
                                    id.to_string(),
                                    Some("visible to LM Studio OpenAI-compatible server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("lm_studio", "LM Studio", "not_required", models)
        }
        Err(error) => provider_error(
            "lm_studio",
            "LM Studio",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn llamafile_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.llamafile_enabled {
        return disabled_provider("llamafile", "Llamafile");
    }

    match request_json(&join_url(&settings.llamafile_url, "/v1/models"), &[]) {
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
                                    "llamafile",
                                    id.to_string(),
                                    Some("visible to Llamafile server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("llamafile", "Llamafile", "not_required", models)
        }
        Err(error) => provider_error(
            "llamafile",
            "Llamafile",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn localai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.localai_enabled {
        return disabled_provider("localai", "LocalAI");
    }

    match request_json(&join_url(&settings.localai_url, "/v1/models"), &[]) {
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
                                    "localai",
                                    id.to_string(),
                                    Some("visible to LocalAI server".into()),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            provider_working("localai", "LocalAI", "not_required", models)
        }
        Err(error) => provider_error(
            "localai",
            "LocalAI",
            "not_required",
            "unreachable",
            error.summary,
        ),
    }
}

fn openai_health(settings: &store::ProviderSettings) -> ProviderHealth {
    if !settings.openai_api_enabled {
        return disabled_provider("openai", "OpenAI API");
    }

    let env_name = settings.openai_api_key_env_var.trim();
    let Ok(api_key) = env::var(env_name) else {
        return provider_error(
            "openai",
            "OpenAI API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live OpenAI model discovery."),
        );
    };

    if api_key.trim().is_empty() {
        return provider_error(
            "openai",
            "OpenAI API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live OpenAI model discovery."),
        );
    }

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

    let env_name = settings.gemini_api_key_env_var.trim();
    let Ok(api_key) = env::var(env_name) else {
        return provider_error(
            "gemini",
            "Gemini API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live Gemini model discovery."),
        );
    };

    if api_key.trim().is_empty() {
        return provider_error(
            "gemini",
            "Gemini API",
            "auth_missing",
            "auth_missing",
            format!("Set {env_name} to enable live Gemini model discovery."),
        );
    }

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

pub(crate) fn model_health_from_settings(
    settings: &store::ProviderSettings,
) -> ModelHealthSnapshot {
    let s = settings.clone();

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
pub fn model_health_snapshot() -> ModelHealthSnapshot {
    build_model_health_snapshot()
}

#[tauri::command]
pub async fn refresh_model_health() -> ModelHealthSnapshot {
    build_model_health_snapshot()
}
