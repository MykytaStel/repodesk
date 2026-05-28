use serde::{Deserialize, Serialize};

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
    pub lm_studio_enabled: bool,
    pub lm_studio_url: String,
    pub llamafile_enabled: bool,
    pub llamafile_url: String,
    pub localai_enabled: bool,
    pub localai_url: String,
    pub chatgpt_enabled: bool,
    pub codex_enabled: bool,
    pub gemini_enabled: bool,
    pub openai_api_enabled: bool,
    pub openai_api_key_env_var: String,
    pub gemini_api_enabled: bool,
    pub gemini_api_key_env_var: String,
    pub allow_paid_agents: bool,
    pub codex_quota_status: String,
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
            lm_studio_enabled: true,
            lm_studio_url: "http://127.0.0.1:1234".to_string(),
            llamafile_enabled: false,
            llamafile_url: "http://127.0.0.1:8080".to_string(),
            localai_enabled: false,
            localai_url: "http://127.0.0.1:8080".to_string(),
            chatgpt_enabled: true,
            codex_enabled: true,
            gemini_enabled: false,
            openai_api_enabled: true,
            openai_api_key_env_var: "OPENAI_API_KEY".to_string(),
            gemini_api_enabled: false,
            gemini_api_key_env_var: "GEMINI_API_KEY".to_string(),
            allow_paid_agents: true,
            codex_quota_status: "unknown".to_string(),
            preferred_patch_provider: "codex".to_string(),
            preferred_compression_provider: "ollama".to_string(),
            preferred_review_provider: "chatgpt".to_string(),
            notes: "Local-first by default. Paid agents should receive bounded smart context only."
                .to_string(),
        }
    }
}
