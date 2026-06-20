import { useEffect, useState } from "react";
import { statusTone, getString, Toggle } from "../../shared/ui/SharedComponents";
import {
  credentialDelete,
  credentialSet,
  credentialStatus,
  CREDENTIAL_KEYS,
  type CredentialMetadata,
} from "../../shared/api/credentials";

import { useQueryClient } from "@tanstack/react-query";
import { pickDirectory, basename } from "../../shared/api/dialog";
import { useToast } from "../../shared/ui/Toast";
import { useSettings } from "./useSettings";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { queryKeys } from "../../shared/api/queries";

export function SettingsTab() {
  const queryClient = useQueryClient();
  const toast = useToast();
  const { dbState, projectName } = useWorkspace();
  const {
    providerSettings,
    apiEnvDiagnostic,
    projectMemory,
    isLoading: isBusy,
    setupForm,
    setSetupForm,
    setupNotice,
    memoryAppendInput,
    setMemoryAppendInput,
    saveSettings,
    isSavingSettings,
    handleAppendMemory,
    isAppendingMemory,
    addProjectFromSetup,
    isAddingProject,
  } = useSettings();

  const [showConnect, setShowConnect] = useState(false);
  const [keyDraft, setKeyDraft] = useState({ anthropic: "", openai: "", gemini: "" });

  const loadProjectMemory = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
  };

  const saveApiKeys = async () => {
    if (!providerSettings) return;
    try {
      await saveSettings({
        ...providerSettings,
        // A blank field keeps the existing key (backend preserves the masked value).
        anthropic_api_key: keyDraft.anthropic.trim() || providerSettings.anthropic_api_key,
        openai_api_key: keyDraft.openai.trim() || providerSettings.openai_api_key,
        gemini_api_key: keyDraft.gemini.trim() || providerSettings.gemini_api_key,
        // Pasting a key enables the matching API provider for routing + discovery.
        anthropic_api_enabled: keyDraft.anthropic.trim() ? true : providerSettings.anthropic_api_enabled,
        openai_api_enabled: keyDraft.openai.trim() ? true : providerSettings.openai_api_enabled,
        gemini_api_enabled: keyDraft.gemini.trim() ? true : providerSettings.gemini_api_enabled,
      });
      setKeyDraft({ anthropic: "", openai: "", gemini: "" });
      queryClient.invalidateQueries({ queryKey: queryKeys.routing.apiEnv });
      queryClient.invalidateQueries({ queryKey: queryKeys.models.health });
      toast.success("API keys saved locally");
    } catch (error: any) {
      toast.error(error?.message || "Could not save API keys");
    }
  };

  const removeKey = async (field: "anthropic_api_key" | "openai_api_key" | "gemini_api_key") => {
    if (!providerSettings) return;
    try {
      await saveSettings({ ...providerSettings, [field]: "" });
      queryClient.invalidateQueries({ queryKey: queryKeys.routing.apiEnv });
      queryClient.invalidateQueries({ queryKey: queryKeys.models.health });
      toast.success("Key removed");
    } catch (error: any) {
      toast.error(error?.message || "Could not remove key");
    }
  };

  const saveProviderSettings = async () => {
    if (!providerSettings) return;
    try {
      await saveSettings(providerSettings);
      toast.success("Provider settings saved");
    } catch (error: any) {
      toast.error(error?.message || "Could not save settings");
    }
  };

  const browseForProjectPath = async () => {
    const path = await pickDirectory();
    if (!path) return;
    setSetupForm((prev) => ({
      ...prev,
      projectPath: path,
      // Default the name from the folder when the user hasn't typed one.
      projectName: prev.projectName.trim() ? prev.projectName : basename(path),
    }));
  };

  if (!providerSettings) {
    return <div className="content-grid"><section className="panel"><p>Loading settings...</p></section></div>;
  }
  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Settings</p>
        <h1>API keys, providers, and workspace.</h1>
        <p className="lead">Paste your own Anthropic, OpenAI, and Gemini API keys below — they're stored locally in <code>~/.repodesk</code>, never committed, never sent to repos or context packs. Local engines (Ollama, LM Studio) need no key.</p>
      </section>

      {/* API keys — the thing most people come here for, so it's first. */}
      <section className="panel wide-panel flex-col gap-lg">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">API keys</p>
            <h2>Bring your own keys</h2>
          </div>
          <span className={`pill ${statusTone(Boolean(dbState))}`}>DB {getString(dbState, "ok", "-")}</span>
        </div>
        <div className="form-stack">
          <KeyField
            label="Anthropic API"
            hint="Powers Anthropic completion routing. Starts with sk-ant-…"
            placeholder="sk-ant-…"
            stored={Boolean(providerSettings.anthropic_api_key)}
            fromEnv={Boolean(apiEnvDiagnostic?.anthropic_api_key_set) && !providerSettings.anthropic_api_key}
            value={keyDraft.anthropic}
            onChange={(v) => setKeyDraft((p) => ({ ...p, anthropic: v }))}
            onRemove={() => void removeKey("anthropic_api_key")}
          />
          <KeyField
            label="OpenAI API"
            hint="Powers OpenAI completion routing. Starts with sk-…"
            placeholder="sk-…"
            stored={Boolean(providerSettings.openai_api_key)}
            fromEnv={Boolean(apiEnvDiagnostic?.openai_api_key_set) && !providerSettings.openai_api_key}
            value={keyDraft.openai}
            onChange={(v) => setKeyDraft((p) => ({ ...p, openai: v }))}
            onRemove={() => void removeKey("openai_api_key")}
          />
          <KeyField
            label="Google (Gemini)"
            hint="Optional. Powers Gemini routing."
            placeholder="AIza…"
            stored={Boolean(providerSettings.gemini_api_key)}
            fromEnv={Boolean(apiEnvDiagnostic?.gemini_api_key_set) && !providerSettings.gemini_api_key}
            value={keyDraft.gemini}
            onChange={(v) => setKeyDraft((p) => ({ ...p, gemini: v }))}
            onRemove={() => void removeKey("gemini_api_key")}
          />
          <div className="button-row">
            <button className="primary-button" onClick={() => void saveApiKeys()} disabled={isSavingSettings || isBusy}>
              {isSavingSettings ? "Saving…" : "Save API keys"}
            </button>
            <span className="muted text-sm">Leave a field blank to keep its saved key. Keys are masked after saving.</span>
          </div>
        </div>
        <details>
          <summary className="muted text-sm" style={{ cursor: "pointer" }}>Prefer environment variables instead?</summary>
          <p className="text-sm text-muted mt-sm">
            You can still export <code>ANTHROPIC_API_KEY</code>, <code>OPENAI_API_KEY</code>, or{" "}
            <code>GEMINI_API_KEY</code> in your shell before launching RepoDesk. An in-app key always
            takes precedence over the environment.
          </p>
        </details>
      </section>

      <SecureKeychainSection />

      {/* Connect project — collapsed by default; usually a one-time action. */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Workspace</p>
            <h2>Active project: {projectName}</h2>
          </div>
          <button className="ghost-button" onClick={() => setShowConnect((v) => !v)}>
            {showConnect ? "Close" : "+ Connect a project"}
          </button>
        </div>
        {showConnect && (
          <div className="form-stack" style={{ marginTop: "var(--space-4)" }}>
            <label>Project name<input value={setupForm.projectName} onChange={(event) => setSetupForm({ ...setupForm, projectName: event.target.value })} /></label>
            <label>Project path
              <div className="input-with-action">
                <input value={setupForm.projectPath} onChange={(event) => setSetupForm({ ...setupForm, projectPath: event.target.value })} placeholder="/Users/you/code/my-app" />
                <button type="button" className="ghost-button" onClick={() => void browseForProjectPath()}>Browse…</button>
              </div>
            </label>
            <label>Project type<input value={setupForm.projectType} onChange={(event) => setSetupForm({ ...setupForm, projectType: event.target.value })} /></label>
            <label>Main language<input value={setupForm.mainLanguage} onChange={(event) => setSetupForm({ ...setupForm, mainLanguage: event.target.value })} /></label>
            <button className="primary-button full" onClick={() => void addProjectFromSetup().catch(() => undefined)} disabled={isAddingProject || isBusy}>
              {isAddingProject ? "Adding and activating..." : "Add and activate project"}
            </button>
            {setupNotice && <div className={`notice ${setupNotice.tone}`}>{setupNotice.message}</div>}
            <p className="muted text-sm">Create and switch tasks from the <strong>Workflow</strong> tab.</p>
          </div>
        )}
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Provider settings</p><h2>Runtime configuration</h2></div><button className="ghost-button" onClick={() => void saveProviderSettings()} disabled={isSavingSettings || isBusy}>Save changes</button></div>
        <div className="settings-grid">
          <Toggle label="Ollama enabled" checked={providerSettings.ollama_enabled} onChange={(value) => saveSettings({ ...providerSettings, ollama_enabled: value })} />
          <Toggle label="LM Studio enabled" checked={providerSettings.lm_studio_enabled} onChange={(value) => saveSettings({ ...providerSettings, lm_studio_enabled: value })} />
          <Toggle label="Llamafile enabled" checked={providerSettings.llamafile_enabled} onChange={(value) => saveSettings({ ...providerSettings, llamafile_enabled: value })} />
          <Toggle label="LocalAI enabled" checked={providerSettings.localai_enabled} onChange={(value) => saveSettings({ ...providerSettings, localai_enabled: value })} />
          <Toggle label="ChatGPT manual enabled" checked={providerSettings.chatgpt_enabled} onChange={(value) => saveSettings({ ...providerSettings, chatgpt_enabled: value })} />
          <Toggle label="Codex CLI route enabled" checked={providerSettings.codex_enabled} onChange={(value) => saveSettings({ ...providerSettings, codex_enabled: value })} />
          <Toggle label="Gemini manual enabled" checked={providerSettings.gemini_enabled} onChange={(value) => saveSettings({ ...providerSettings, gemini_enabled: value })} />
          <Toggle label="Anthropic API enabled" checked={providerSettings.anthropic_api_enabled} onChange={(value) => saveSettings({ ...providerSettings, anthropic_api_enabled: value })} />
          <Toggle label="OpenAI API enabled" checked={providerSettings.openai_api_enabled} onChange={(value) => saveSettings({ ...providerSettings, openai_api_enabled: value })} />
          <Toggle label="Gemini API enabled" checked={providerSettings.gemini_api_enabled} onChange={(value) => saveSettings({ ...providerSettings, gemini_api_enabled: value })} />
          <Toggle label="Allow paid agents" checked={providerSettings.allow_paid_agents} onChange={(value) => saveSettings({ ...providerSettings, allow_paid_agents: value })} />
          <label>Codex CLI quota proxy<select value={providerSettings.codex_quota_status} onChange={(event) => saveSettings({ ...providerSettings, codex_quota_status: event.target.value })}>
            <option value="unknown">unknown</option>
            <option value="available">available</option>
            <option value="limited">limited</option>
            <option value="empty">empty</option>
          </select></label>
          <label>Ollama URL<input value={providerSettings.ollama_url} onChange={(event) => saveSettings({ ...providerSettings, ollama_url: event.target.value })} /></label>
          <label>Ollama default model<input value={providerSettings.ollama_model} onChange={(event) => saveSettings({ ...providerSettings, ollama_model: event.target.value })} /></label>
          <label>LM Studio URL<input value={providerSettings.lm_studio_url} onChange={(event) => saveSettings({ ...providerSettings, lm_studio_url: event.target.value })} /></label>
          <label>Llamafile URL<input value={providerSettings.llamafile_url} onChange={(event) => saveSettings({ ...providerSettings, llamafile_url: event.target.value })} /></label>
          <label>LocalAI URL<input value={providerSettings.localai_url} onChange={(event) => saveSettings({ ...providerSettings, localai_url: event.target.value })} /></label>
          <label>OpenAI key env var<input value={providerSettings.openai_api_key_env_var} onChange={(event) => saveSettings({ ...providerSettings, openai_api_key_env_var: event.target.value })} /></label>
          <label>Gemini key env var<input value={providerSettings.gemini_api_key_env_var} onChange={(event) => saveSettings({ ...providerSettings, gemini_api_key_env_var: event.target.value })} /></label>
          <label>Patch provider<input value={providerSettings.preferred_patch_provider} onChange={(event) => saveSettings({ ...providerSettings, preferred_patch_provider: event.target.value })} /></label>
          <label>Compression provider<input value={providerSettings.preferred_compression_provider} onChange={(event) => saveSettings({ ...providerSettings, preferred_compression_provider: event.target.value })} /></label>
          <label>Review provider<input value={providerSettings.preferred_review_provider} onChange={(event) => saveSettings({ ...providerSettings, preferred_review_provider: event.target.value })} /></label>
          <label className="span-2">Notes<textarea rows={3} value={providerSettings.notes} onChange={(event) => saveSettings({ ...providerSettings, notes: event.target.value })} /></label>
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Project Memory & Guidelines</p>
            <h2>Active workspace instructions</h2>
          </div>
          <button className="tiny-button" onClick={() => void loadProjectMemory()}>Reload memory</button>
        </div>
        <p className="muted" style={{ marginBottom: "12px" }}>
          This memory is included in all context packs to guide external agents and avoid unwanted token usage on unnecessary directories or patterns.
        </p>
        <div className="code-panel compact" style={{ maxHeight: "250px", marginBottom: "14px", overflowY: "auto", display: "flex", flexDirection: "column", gap: "8px" }}>
          {(!projectMemory || projectMemory.length === 0) ? (
            <div className="muted">No guidelines or memory logs saved yet.</div>
          ) : (
            projectMemory.map((entry: any) => (
              <div key={entry.id} style={{ borderBottom: "1px solid rgba(255,255,255,0.1)", paddingBottom: "8px" }}>
                <div style={{ fontSize: "0.8em", color: "var(--muted)", marginBottom: "4px" }}>
                  {new Date(entry.timestamp).toLocaleString()} <span className="pill neutral" style={{ marginLeft: "8px" }}>{entry.category}</span>
                </div>
                <div style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
              </div>
            ))
          )}
        </div>
        <div className="form-stack">
          <label>
            Add memory log / rule (e.g. "Do not change public API flags", "Always keep code modifications inside src-tauri/")
            <textarea
              rows={3}
              value={memoryAppendInput}
              onChange={(event) => setMemoryAppendInput(event.target.value)}
              placeholder="Guidelines, constraints, or architecture notes for agents to remember..."
            />
          </label>
          <button className="primary-button" onClick={() => void handleAppendMemory()} disabled={isAppendingMemory || isBusy || !memoryAppendInput.trim()}>
            Add guidelines to memory.md
          </button>
        </div>
      </section>

    </div>
  );
}

/** Manage provider API keys in the OS keychain (macOS Keychain / Windows
 * Credential Manager / Linux Secret Service). Secrets are write-only from the UI:
 * we send a value to store, but only ever read back a masked hint. */
function SecureKeychainSection() {
  const toast = useToast();
  const [status, setStatus] = useState<CredentialMetadata[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const labels: Record<string, string> = {
    [CREDENTIAL_KEYS.openai]: "OpenAI API key",
    [CREDENTIAL_KEYS.anthropic]: "Anthropic API key",
    [CREDENTIAL_KEYS.gemini]: "Gemini API key",
  };

  const refresh = async () => {
    try {
      setStatus(await credentialStatus());
    } catch (error) {
      toast.error((error as { message?: string })?.message || "Could not read keychain status");
    }
  };

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const save = async (key: string) => {
    const value = drafts[key] ?? "";
    if (!value.trim()) return;
    setBusy(true);
    try {
      await credentialSet(key, value);
      setDrafts((d) => ({ ...d, [key]: "" }));
      await refresh();
      toast.success("Stored securely in the OS keychain");
    } catch (error) {
      toast.error((error as { message?: string })?.message || "Could not store key");
    } finally {
      setBusy(false);
    }
  };

  const remove = async (key: string) => {
    setBusy(true);
    try {
      await credentialDelete(key);
      await refresh();
      toast.success("Removed from the OS keychain");
    } catch (error) {
      toast.error((error as { message?: string })?.message || "Could not remove key");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="panel wide-panel flex-col gap-lg">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Secure keys</p>
          <h2>OS keychain (recommended)</h2>
          <p className="lead">Stored in your operating system's secure store — never in app files, logs, or repos. The app only ever reads back a masked hint, never the full key.</p>
        </div>
      </div>
      <div className="flex-col gap-md">
        {Object.values(CREDENTIAL_KEYS).map((key) => {
          const meta = status.find((m) => m.key === key);
          return (
            <label key={key}>
              <span className="flex items-center gap-sm" style={{ marginBottom: 4 }}>
                <strong>{labels[key]}</strong>
                <span className={`pill ${meta?.configured ? "ok" : "warn"}`}>
                  {meta?.configured ? `Stored ${meta.hint}` : "Not configured"}
                </span>
              </span>
              <div className="input-with-action">
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={drafts[key] ?? ""}
                  placeholder={meta?.configured ? "•••••••• stored — enter to replace" : "Paste key to store securely"}
                  onChange={(event) => setDrafts((d) => ({ ...d, [key]: event.target.value }))}
                />
                <button type="button" className="ghost-button" disabled={busy || !(drafts[key] ?? "").trim()} onClick={() => void save(key)}>
                  Store
                </button>
                {meta?.configured && (
                  <button type="button" className="ghost-button" disabled={busy} onClick={() => void remove(key)}>Remove</button>
                )}
              </div>
            </label>
          );
        })}
      </div>
    </section>
  );
}

/** One labelled, masked API-key input with a status pill and a remove action. */
function KeyField({
  label,
  hint,
  placeholder,
  stored,
  fromEnv,
  value,
  onChange,
  onRemove,
}: {
  label: string;
  hint: string;
  placeholder: string;
  stored: boolean;
  fromEnv: boolean;
  value: string;
  onChange: (value: string) => void;
  onRemove: () => void;
}) {
  const status = stored ? { tone: "ok", text: "Saved in app" } : fromEnv ? { tone: "neutral", text: "From environment" } : { tone: "warn", text: "Not configured" };
  return (
    <label>
      <span className="flex items-center gap-sm" style={{ marginBottom: 4 }}>
        <strong>{label}</strong>
        <span className={`pill ${status.tone}`}>{status.text}</span>
      </span>
      <div className="input-with-action">
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={value}
          placeholder={stored ? "•••••••• saved — leave blank to keep" : placeholder}
          onChange={(event) => onChange(event.target.value)}
        />
        {stored && (
          <button type="button" className="ghost-button" onClick={onRemove}>Remove</button>
        )}
      </div>
      <span className="text-sm text-muted">{hint}</span>
    </label>
  );
}
