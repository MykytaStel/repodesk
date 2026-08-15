import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../../shared/api/queries";
import { startLocalServer, type ModelHealthSnapshot, type ProviderHealth } from "../../shared/api/models";
import { Toggle } from "../../shared/ui/SharedComponents";
import { useToast } from "../../shared/ui/Toast";
import { CredentialsSection } from "./CredentialsSection";
import { CustomProvidersPanel } from "./CustomProvidersPanel";
import { IdePreferencesPanel } from "./IdePreferencesPanel";
import { useSettings } from "./useSettings";

export function SettingsTab() {
  const queryClient = useQueryClient();
  const toast = useToast();
  const {
    providerPreferences,
    isLoading: isBusy,
    savePreferences,
    isSavingPreferences,
  } = useSettings();

  const modelHealthQuery = useQuery({
    queryKey: queryKeys.models.health,
    queryFn: () => invoke<ModelHealthSnapshot>("model_health_snapshot"),
  });

  const handleLaunch = async (provider: string) => {
    try {
      await startLocalServer(provider);
      toast.success(`Launched ${provider}, waiting for server...`);
      setTimeout(() => {
        queryClient.invalidateQueries({ queryKey: queryKeys.models.health });
      }, 3000);
    } catch (error: any) {
      toast.error(error?.message || `Could not launch ${provider}`);
    }
  };

  const saveProviderPreferences = async () => {
    if (!providerPreferences) return;
    try {
      await savePreferences(providerPreferences);
      toast.success("Provider preferences saved");
    } catch (error: any) {
      toast.error(error?.message || "Could not save preferences");
    }
  };

  if (!providerPreferences) {
    return <div className="content-grid"><section className="panel"><p>Loading settings...</p></section></div>;
  }

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Settings</p>
        <h1>API keys, providers, and preferences.</h1>
        <p className="lead">
          Configure local runtimes and provider routing here. Provider secrets are written only to
          your OS keychain; environment variables remain read-only fallbacks.
        </p>
      </section>

      <CredentialsSection />

      <IdePreferencesPanel />

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Provider settings</p>
            <h2>Runtime configuration</h2>
          </div>
          <button
            className="ghost-button"
            onClick={() => void saveProviderPreferences()}
            disabled={isSavingPreferences || isBusy}
          >
            Save changes
          </button>
        </div>
        <div className="settings-grid">
          <Toggle label="Ollama enabled" checked={providerPreferences.ollama_enabled} onChange={(value) => savePreferences({ ...providerPreferences, ollama_enabled: value })} />
          <Toggle label="LM Studio enabled" checked={providerPreferences.lm_studio_enabled} onChange={(value) => savePreferences({ ...providerPreferences, lm_studio_enabled: value })} />
          <Toggle label="Llamafile enabled" checked={providerPreferences.llamafile_enabled} onChange={(value) => savePreferences({ ...providerPreferences, llamafile_enabled: value })} />
          <Toggle label="LocalAI enabled" checked={providerPreferences.localai_enabled} onChange={(value) => savePreferences({ ...providerPreferences, localai_enabled: value })} />
          <Toggle label="ChatGPT manual enabled" checked={providerPreferences.chatgpt_enabled} onChange={(value) => savePreferences({ ...providerPreferences, chatgpt_enabled: value })} />
          <Toggle label="Codex CLI route enabled" checked={providerPreferences.codex_enabled} onChange={(value) => savePreferences({ ...providerPreferences, codex_enabled: value })} />
          <Toggle label="Gemini manual enabled" checked={providerPreferences.gemini_enabled} onChange={(value) => savePreferences({ ...providerPreferences, gemini_enabled: value })} />
          <Toggle label="Anthropic API enabled" checked={providerPreferences.anthropic_api_enabled} onChange={(value) => savePreferences({ ...providerPreferences, anthropic_api_enabled: value })} />
          <Toggle label="OpenAI API enabled" checked={providerPreferences.openai_api_enabled} onChange={(value) => savePreferences({ ...providerPreferences, openai_api_enabled: value })} />
          <Toggle label="Gemini API enabled" checked={providerPreferences.gemini_api_enabled} onChange={(value) => savePreferences({ ...providerPreferences, gemini_api_enabled: value })} />
          <Toggle label="Allow paid agents" checked={providerPreferences.allow_paid_agents} onChange={(value) => savePreferences({ ...providerPreferences, allow_paid_agents: value })} />
          <label>
            Codex CLI quota proxy
            <select value={providerPreferences.codex_quota_status} onChange={(event) => savePreferences({ ...providerPreferences, codex_quota_status: event.target.value })}>
              <option value="unknown">unknown</option>
              <option value="available">available</option>
              <option value="limited">limited</option>
              <option value="empty">empty</option>
            </select>
          </label>
          <label>Ollama URL<input value={providerPreferences.ollama_url} onChange={(event) => savePreferences({ ...providerPreferences, ollama_url: event.target.value })} /></label>
          <LocalModelSelect
            label="Ollama default model"
            providerId="ollama"
            value={providerPreferences.ollama_model}
            onChange={(value) => savePreferences({ ...providerPreferences, ollama_model: value })}
            health={modelHealthQuery.data?.providers.find((provider) => provider.id === "ollama")}
            onLaunch={() => void handleLaunch("ollama")}
          />
          <label>LM Studio URL<input value={providerPreferences.lm_studio_url} onChange={(event) => savePreferences({ ...providerPreferences, lm_studio_url: event.target.value })} /></label>
          <label>Llamafile URL<input value={providerPreferences.llamafile_url} onChange={(event) => savePreferences({ ...providerPreferences, llamafile_url: event.target.value })} /></label>
          <label>LocalAI URL<input value={providerPreferences.localai_url} onChange={(event) => savePreferences({ ...providerPreferences, localai_url: event.target.value })} /></label>
          <label>OpenAI key env var<input value={providerPreferences.openai_api_key_env_var} onChange={(event) => savePreferences({ ...providerPreferences, openai_api_key_env_var: event.target.value })} /></label>
          <label>Gemini key env var<input value={providerPreferences.gemini_api_key_env_var} onChange={(event) => savePreferences({ ...providerPreferences, gemini_api_key_env_var: event.target.value })} /></label>
          <label>Patch provider<input value={providerPreferences.preferred_patch_provider} onChange={(event) => savePreferences({ ...providerPreferences, preferred_patch_provider: event.target.value })} /></label>
          <label>Compression provider<input value={providerPreferences.preferred_compression_provider} onChange={(event) => savePreferences({ ...providerPreferences, preferred_compression_provider: event.target.value })} /></label>
          <label>Review provider<input value={providerPreferences.preferred_review_provider} onChange={(event) => savePreferences({ ...providerPreferences, preferred_review_provider: event.target.value })} /></label>
          <label className="span-2">Notes<textarea rows={3} value={providerPreferences.notes} onChange={(event) => savePreferences({ ...providerPreferences, notes: event.target.value })} /></label>
        </div>
      </section>

      <CustomProvidersPanel />
    </div>
  );
}

function LocalModelSelect({
  label,
  providerId,
  value,
  onChange,
  health,
  onLaunch,
}: {
  label: string;
  providerId: string;
  value: string;
  onChange: (value: string) => void;
  health?: ProviderHealth;
  onLaunch: () => void;
}) {
  const isReachable = health?.reachability === "working";

  return (
    <label>
      <div className="flex items-center gap-sm" style={{ marginBottom: 4 }}>
        <strong>{label}</strong>
        {health && (
          <span className={`pill ${isReachable ? "ok" : "warn"}`}>
            {isReachable ? "🟢 Running" : "🔴 Stopped"}
          </span>
        )}
      </div>
      <div className="input-with-action">
        {isReachable && health.models.length > 0 ? (
          <select value={value} onChange={(event) => onChange(event.target.value)} style={{ flex: 1 }}>
            <option value="">-- select a model --</option>
            {health.models.map((model) => (
              <option key={model.id} value={model.id}>{model.id}</option>
            ))}
          </select>
        ) : (
          <input
            value={value}
            onChange={(event) => onChange(event.target.value)}
            placeholder={isReachable ? "No models found" : `Enter ${providerId} model name`}
          />
        )}
        {!isReachable && (
          <button type="button" className="ghost-button" onClick={onLaunch}>
            Launch {providerId}
          </button>
        )}
      </div>
    </label>
  );
}
