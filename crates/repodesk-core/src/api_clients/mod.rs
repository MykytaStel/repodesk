//! LLM provider clients and the provider-agnostic abstraction the orchestrator
//! builds on.
//!
//! [`LlmProvider`] keeps the original `generate`/`is_available` methods (used by
//! the Memory Brain) untouched, and adds [`LlmProvider::complete`] — a richer
//! call with per-request model selection, an optional system prompt, an output
//! token cap, an extended-thinking hint, and token usage returned for the
//! ledger. Use [`provider_for`] to build the right client for a provider name.

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

pub use anthropic::AnthropicClient;
pub use gemini::GeminiClient;
pub use ollama::OllamaClient;
pub use openai::OpenAiClient;

/// Boxed, `Send` future returned by provider methods (object-safe, `'static`).
pub type ProviderFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;

/// Extended-thinking / reasoning-effort hint for a completion. Providers that
/// don't support it ignore the hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    /// Token budget for providers with an explicit thinking budget (Anthropic).
    /// `None` means extended thinking is disabled.
    pub fn thinking_budget_tokens(&self) -> Option<u32> {
        match self {
            Self::None => None,
            Self::Low => Some(1_024),
            Self::Medium => Some(4_096),
            Self::High => Some(12_000),
        }
    }
}

/// A single, provider-agnostic completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Provider-specific model id (e.g. `claude-...`, `gpt-...`, `llama3.1`).
    pub model: String,
    /// Optional system prompt / instructions.
    #[serde(default)]
    pub system: Option<String>,
    /// The user prompt (typically the per-sub-agent context pack).
    pub prompt: String,
    /// Hard cap on output tokens.
    pub max_tokens: u32,
    /// Extended-thinking hint (honored by providers that support it).
    #[serde(default)]
    pub thinking: ThinkingLevel,
}

impl LlmRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system: None,
            prompt: prompt.into(),
            max_tokens: 1_024,
            thinking: ThinkingLevel::None,
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_thinking(mut self, thinking: ThinkingLevel) -> Self {
        self.thinking = thinking;
        self
    }
}

/// The result of a completion, including token accounting for the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

/// `Send + Sync` so a boxed provider can be held across `.await` points (e.g. in
/// the orchestrator runner, and in Tauri commands which require `Send` futures).
pub trait LlmProvider: Send + Sync {
    /// Legacy one-shot generate (used by the Memory Brain). Kept for back-compat.
    fn generate(
        &self,
        prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>>;

    /// Health check.
    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>;

    /// Rich completion with model selection, an optional system prompt, an output
    /// token cap, a thinking hint, and token usage returned. Used by the
    /// orchestrator's runner.
    fn complete(&self, request: LlmRequest) -> ProviderFuture<RepoDeskResult<LlmResponse>>;
}

/// Credentials + endpoint overrides for one provider.
#[derive(Debug, Clone, Default)]
pub struct ProviderCredential {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

impl ProviderCredential {
    fn has_key(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.trim().is_empty())
    }
}

/// All provider credentials, resolved env-first. A settings-file fallback can be
/// layered on later without changing call sites.
#[derive(Debug, Clone, Default)]
pub struct ProviderSettings {
    pub ollama: ProviderCredential,
    /// LM Studio's OpenAI-compatible local server (no key; runs at localhost).
    pub lm_studio: ProviderCredential,
    pub anthropic: ProviderCredential,
    pub openai: ProviderCredential,
    pub gemini: ProviderCredential,
}

impl ProviderSettings {
    /// Resolve credentials from environment variables.
    pub fn from_env() -> Self {
        let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Self {
            ollama: ProviderCredential {
                api_key: None,
                base_url: env("OLLAMA_BASE_URL"),
                default_model: env("OLLAMA_MODEL"),
            },
            lm_studio: ProviderCredential {
                api_key: None,
                base_url: env("LM_STUDIO_BASE_URL"),
                default_model: env("LM_STUDIO_MODEL"),
            },
            anthropic: ProviderCredential {
                api_key: env("ANTHROPIC_API_KEY"),
                base_url: env("ANTHROPIC_BASE_URL"),
                default_model: env("ANTHROPIC_MODEL"),
            },
            openai: ProviderCredential {
                api_key: env("OPENAI_API_KEY"),
                base_url: env("OPENAI_BASE_URL"),
                default_model: env("OPENAI_MODEL"),
            },
            gemini: ProviderCredential {
                api_key: env("GEMINI_API_KEY").or_else(|| env("GOOGLE_API_KEY")),
                base_url: env("GEMINI_BASE_URL"),
                default_model: env("GEMINI_MODEL"),
            },
        }
    }

    /// Which configured providers currently have a usable client (key present,
    /// or local Ollama which needs none). Used for plan/route gating.
    pub fn available_providers(&self) -> Vec<&'static str> {
        // Both local providers need no key; they're always offered for routing.
        let mut out = vec!["ollama", "lm_studio"];
        if self.anthropic.has_key() {
            out.push("anthropic");
        }
        if self.openai.has_key() {
            out.push("openai");
        }
        if self.gemini.has_key() {
            out.push("gemini");
        }
        out
    }
}

/// Build a boxed [`LlmProvider`] for a provider name, using `settings` for
/// credentials/endpoints. Provider names are normalized: `chatgpt`/`codex`/`gpt`
/// map to the OpenAI client; `claude` to Anthropic; local engines to Ollama.
pub fn provider_for(
    name: &str,
    settings: &ProviderSettings,
) -> RepoDeskResult<Box<dyn LlmProvider>> {
    match name.trim().to_ascii_lowercase().as_str() {
        "ollama" | "local" | "llamafile" | "localai" => Ok(Box::new(OllamaClient::new(
            settings.ollama.base_url.clone(),
            settings.ollama.default_model.clone(),
        ))),
        // LM Studio serves an OpenAI-compatible API (not Ollama's), so it uses the
        // OpenAI client pointed at the local endpoint. It ignores auth, but the
        // client treats a non-empty key as "available", so a placeholder is used
        // when none is configured.
        "lm_studio" | "lmstudio" => {
            let key = settings
                .lm_studio
                .api_key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .unwrap_or_else(|| "lm-studio".to_string());
            let base_url = Some(
                settings
                    .lm_studio
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:1234".to_string()),
            );
            Ok(Box::new(OpenAiClient::new(key, base_url)))
        }
        "anthropic" | "claude" => Ok(Box::new(AnthropicClient::new(
            require_key("anthropic", "ANTHROPIC_API_KEY", &settings.anthropic)?,
            settings.anthropic.base_url.clone(),
        ))),
        "openai" | "chatgpt" | "codex" | "gpt" => Ok(Box::new(OpenAiClient::new(
            require_key("openai", "OPENAI_API_KEY", &settings.openai)?,
            settings.openai.base_url.clone(),
        ))),
        "gemini" | "google" => Ok(Box::new(GeminiClient::new(
            require_key("gemini", "GEMINI_API_KEY", &settings.gemini)?,
            settings.gemini.base_url.clone(),
        ))),
        other => Err(RepoDeskError::RoutingFailed {
            detail: format!("no LLM client is implemented for provider '{other}'"),
        }),
    }
}

fn require_key(provider: &str, env_var: &str, cred: &ProviderCredential) -> RepoDeskResult<String> {
    cred.api_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| RepoDeskError::ProviderUnavailable {
            provider: provider.to_string(),
            detail: format!("missing API key (set the {env_var} environment variable)"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_budget_scales_with_level() {
        assert_eq!(ThinkingLevel::None.thinking_budget_tokens(), None);
        assert_eq!(ThinkingLevel::Low.thinking_budget_tokens(), Some(1_024));
        assert!(
            ThinkingLevel::High.thinking_budget_tokens().unwrap()
                > ThinkingLevel::Medium.thinking_budget_tokens().unwrap()
        );
    }

    #[test]
    fn request_builder_sets_fields() {
        let req = LlmRequest::new("claude-x", "hello")
            .with_system("be terse")
            .with_max_tokens(2_048)
            .with_thinking(ThinkingLevel::Medium);
        assert_eq!(req.model, "claude-x");
        assert_eq!(req.system.as_deref(), Some("be terse"));
        assert_eq!(req.max_tokens, 2_048);
        assert_eq!(req.thinking, ThinkingLevel::Medium);
    }

    #[test]
    fn provider_for_ollama_needs_no_key() {
        let settings = ProviderSettings::default();
        assert!(provider_for("ollama", &settings).is_ok());
        assert!(provider_for("local", &settings).is_ok());
    }

    #[test]
    fn provider_for_paid_requires_key() {
        let settings = ProviderSettings::default();
        // `Box<dyn LlmProvider>` isn't `Debug`, so match on the Result directly.
        assert!(matches!(
            provider_for("anthropic", &settings),
            Err(RepoDeskError::ProviderUnavailable { .. })
        ));

        let mut with_key = ProviderSettings::default();
        with_key.anthropic.api_key = Some("sk-test".to_string());
        assert!(provider_for("claude", &with_key).is_ok());
    }

    #[test]
    fn provider_for_aliases_map_to_clients() {
        let mut settings = ProviderSettings::default();
        settings.openai.api_key = Some("sk-test".to_string());
        for alias in ["openai", "chatgpt", "codex", "gpt"] {
            assert!(provider_for(alias, &settings).is_ok(), "alias {alias}");
        }
    }

    #[test]
    fn provider_for_unknown_is_routing_error() {
        assert!(matches!(
            provider_for("mystery", &ProviderSettings::default()),
            Err(RepoDeskError::RoutingFailed { .. })
        ));
    }

    #[test]
    fn available_providers_lists_keyed_only() {
        let mut settings = ProviderSettings::default();
        // Both local providers need no key and are always offered.
        assert_eq!(settings.available_providers(), vec!["ollama", "lm_studio"]);
        settings.openai.api_key = Some("sk".to_string());
        settings.gemini.api_key = Some("g".to_string());
        let avail = settings.available_providers();
        assert!(avail.contains(&"lm_studio"));
        assert!(avail.contains(&"openai"));
        assert!(avail.contains(&"gemini"));
        assert!(!avail.contains(&"anthropic"));
    }

    #[test]
    fn provider_for_lm_studio_uses_local_openai_endpoint_without_a_key() {
        // LM Studio needs no real key (the OpenAI-compatible local server ignores
        // auth), so building its client must succeed with default settings.
        let settings = ProviderSettings::default();
        assert!(provider_for("lm_studio", &settings).is_ok());
        assert!(provider_for("lmstudio", &settings).is_ok());
    }
}
