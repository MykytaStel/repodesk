use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStatus {
    pub path: String,
    pub exists: bool,
    pub ok: bool,
    pub tables: Vec<String>,
    pub error: Option<String>,
}

/// Legacy/internal provider settings model. Credential fields remain here only
/// for runtime compatibility and plaintext-to-keychain migration. It is not a
/// user-facing provider-preference IPC contract.
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
    pub anthropic_api_enabled: bool,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub gemini_api_key: String,
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
            anthropic_api_enabled: false,
            anthropic_api_key: String::new(),
            openai_api_key: String::new(),
            gemini_api_key: String::new(),
            allow_paid_agents: true,
            codex_quota_status: "unknown".to_string(),
            preferred_patch_provider: "codex_cli".to_string(),
            preferred_compression_provider: "ollama".to_string(),
            preferred_review_provider: "chatgpt".to_string(),
            notes: "Local-first by default. Paid agents should receive bounded smart context only."
                .to_string(),
        }
    }
}

/// Non-secret settings contract used by current Settings IPC. This type is the
/// capability boundary: ordinary preference mutations cannot carry provider
/// secrets because there are no credential fields to populate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPreferences {
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
    pub anthropic_api_enabled: bool,
    pub allow_paid_agents: bool,
    pub codex_quota_status: String,
    pub preferred_patch_provider: String,
    pub preferred_compression_provider: String,
    pub preferred_review_provider: String,
    pub notes: String,
}

impl From<&ProviderSettings> for ProviderPreferences {
    fn from(settings: &ProviderSettings) -> Self {
        Self {
            ollama_enabled: settings.ollama_enabled,
            ollama_url: settings.ollama_url.clone(),
            ollama_model: settings.ollama_model.clone(),
            lm_studio_enabled: settings.lm_studio_enabled,
            lm_studio_url: settings.lm_studio_url.clone(),
            llamafile_enabled: settings.llamafile_enabled,
            llamafile_url: settings.llamafile_url.clone(),
            localai_enabled: settings.localai_enabled,
            localai_url: settings.localai_url.clone(),
            chatgpt_enabled: settings.chatgpt_enabled,
            codex_enabled: settings.codex_enabled,
            gemini_enabled: settings.gemini_enabled,
            openai_api_enabled: settings.openai_api_enabled,
            openai_api_key_env_var: settings.openai_api_key_env_var.clone(),
            gemini_api_enabled: settings.gemini_api_enabled,
            gemini_api_key_env_var: settings.gemini_api_key_env_var.clone(),
            anthropic_api_enabled: settings.anthropic_api_enabled,
            allow_paid_agents: settings.allow_paid_agents,
            codex_quota_status: settings.codex_quota_status.clone(),
            preferred_patch_provider: settings.preferred_patch_provider.clone(),
            preferred_compression_provider: settings.preferred_compression_provider.clone(),
            preferred_review_provider: settings.preferred_review_provider.clone(),
            notes: settings.notes.clone(),
        }
    }
}

impl Default for ProviderPreferences {
    fn default() -> Self {
        Self::from(&ProviderSettings::default())
    }
}

impl ProviderPreferences {
    /// Apply only non-secret preferences to the legacy/internal model. Existing
    /// credential values are deliberately untouched.
    pub fn apply_to(&self, settings: &mut ProviderSettings) {
        settings.ollama_enabled = self.ollama_enabled;
        settings.ollama_url.clone_from(&self.ollama_url);
        settings.ollama_model.clone_from(&self.ollama_model);
        settings.lm_studio_enabled = self.lm_studio_enabled;
        settings.lm_studio_url.clone_from(&self.lm_studio_url);
        settings.llamafile_enabled = self.llamafile_enabled;
        settings.llamafile_url.clone_from(&self.llamafile_url);
        settings.localai_enabled = self.localai_enabled;
        settings.localai_url.clone_from(&self.localai_url);
        settings.chatgpt_enabled = self.chatgpt_enabled;
        settings.codex_enabled = self.codex_enabled;
        settings.gemini_enabled = self.gemini_enabled;
        settings.openai_api_enabled = self.openai_api_enabled;
        settings
            .openai_api_key_env_var
            .clone_from(&self.openai_api_key_env_var);
        settings.gemini_api_enabled = self.gemini_api_enabled;
        settings
            .gemini_api_key_env_var
            .clone_from(&self.gemini_api_key_env_var);
        settings.anthropic_api_enabled = self.anthropic_api_enabled;
        settings.allow_paid_agents = self.allow_paid_agents;
        settings
            .codex_quota_status
            .clone_from(&self.codex_quota_status);
        settings
            .preferred_patch_provider
            .clone_from(&self.preferred_patch_provider);
        settings
            .preferred_compression_provider
            .clone_from(&self.preferred_compression_provider);
        settings
            .preferred_review_provider
            .clone_from(&self.preferred_review_provider);
        settings.notes.clone_from(&self.notes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_preferences_do_not_serialize_credentials() {
        let json =
            serde_json::to_string(&ProviderPreferences::default()).expect("serialize preferences");

        assert!(!json.contains("anthropic_api_key\""));
        assert!(!json.contains("openai_api_key\""));
        assert!(!json.contains("gemini_api_key\""));
    }

    #[test]
    fn applying_preferences_preserves_internal_credentials() {
        let mut settings = ProviderSettings {
            anthropic_api_key: "fixture-a".into(),
            openai_api_key: "fixture-b".into(),
            gemini_api_key: "fixture-c".into(),
            ..ProviderSettings::default()
        };
        let preferences = ProviderPreferences {
            ollama_enabled: false,
            notes: "updated preference".into(),
            ..ProviderPreferences::default()
        };

        preferences.apply_to(&mut settings);

        assert!(!settings.ollama_enabled);
        assert_eq!(settings.notes, "updated preference");
        assert_eq!(settings.anthropic_api_key, "fixture-a");
        assert_eq!(settings.openai_api_key, "fixture-b");
        assert_eq!(settings.gemini_api_key, "fixture-c");
    }
}
