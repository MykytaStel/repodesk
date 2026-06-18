import { statusTone, getString, Toggle } from "../../shared/ui/SharedComponents";

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
    taskNotice,
    memoryAppendInput,
    setMemoryAppendInput,
    saveSettings,
    isSavingSettings,
    handleAppendMemory,
    isAppendingMemory,
    addProjectFromSetup,
    isAddingProject,
    createTaskFromSetup,
    isCreatingTask,
  } = useSettings();

  const loadProjectMemory = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
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
  
  const refreshAll = (label: string) => {
    queryClient.invalidateQueries();
  };

  if (!providerSettings) {
    return <div className="content-grid"><section className="panel"><p>Loading settings...</p></section></div>;
  }
  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Settings</p>
        <h1>Project, task, and provider controls.</h1>
        <p className="lead">Provider settings store URLs, toggles, and environment variable names only. Raw API keys stay outside RepoDesk settings.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void saveProviderSettings()} disabled={isSavingSettings || isBusy}>Save provider settings</button>
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing settings")} disabled={isBusy}>Refresh</button>
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">Connect project</p><h2>Active workspace</h2>
        <div className="form-stack">
          <div className="notice" style={{ padding: "10px 12px" }}>
            Current active project: <strong>{projectName}</strong>
          </div>
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
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">Task</p><h2>Create active task</h2>
        <div className="form-stack">
          <label>Task title<input value={setupForm.taskTitle} onChange={(event) => setSetupForm({ ...setupForm, taskTitle: event.target.value })} /></label>
          <button className="primary-button full" onClick={() => void createTaskFromSetup().catch(() => undefined)} disabled={isCreatingTask || isBusy}>
            {isCreatingTask ? "Creating task..." : "Create task"}
          </button>
          {taskNotice && <div className={`notice ${taskNotice.tone}`}>{taskNotice.message}</div>}
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Provider settings</p><h2>Runtime configuration</h2></div><span className={`pill ${statusTone(Boolean(dbState))}`}>DB {getString(dbState, "ok", "-")}</span></div>
        <div className="settings-grid">
          <Toggle label="Ollama enabled" checked={providerSettings.ollama_enabled} onChange={(value) => saveSettings({ ...providerSettings, ollama_enabled: value })} />
          <Toggle label="LM Studio enabled" checked={providerSettings.lm_studio_enabled} onChange={(value) => saveSettings({ ...providerSettings, lm_studio_enabled: value })} />
          <Toggle label="Llamafile enabled" checked={providerSettings.llamafile_enabled} onChange={(value) => saveSettings({ ...providerSettings, llamafile_enabled: value })} />
          <Toggle label="LocalAI enabled" checked={providerSettings.localai_enabled} onChange={(value) => saveSettings({ ...providerSettings, localai_enabled: value })} />
          <Toggle label="ChatGPT manual enabled" checked={providerSettings.chatgpt_enabled} onChange={(value) => saveSettings({ ...providerSettings, chatgpt_enabled: value })} />
          <Toggle label="Codex enabled" checked={providerSettings.codex_enabled} onChange={(value) => saveSettings({ ...providerSettings, codex_enabled: value })} />
          <Toggle label="Gemini manual enabled" checked={providerSettings.gemini_enabled} onChange={(value) => saveSettings({ ...providerSettings, gemini_enabled: value })} />
          <Toggle label="OpenAI API enabled" checked={providerSettings.openai_api_enabled} onChange={(value) => saveSettings({ ...providerSettings, openai_api_enabled: value })} />
          <Toggle label="Gemini API enabled" checked={providerSettings.gemini_api_enabled} onChange={(value) => saveSettings({ ...providerSettings, gemini_api_enabled: value })} />
          <Toggle label="Allow paid agents" checked={providerSettings.allow_paid_agents} onChange={(value) => saveSettings({ ...providerSettings, allow_paid_agents: value })} />
          <label>Codex quota proxy<select value={providerSettings.codex_quota_status} onChange={(event) => saveSettings({ ...providerSettings, codex_quota_status: event.target.value })}>
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

      <section className="panel wide-panel flex-col gap-lg">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow m-0">Security & API Credentials</p>
            <h2 className="mt-xs">Secure API Environment Diagnostic</h2>
          </div>
        </div>
        <p className="muted">
          RepoDesk detects system environment variables to securely sign API requests without storing plaintext credentials in local files or databases.
        </p>

        <div style={{ display: "grid", gap: "12px", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
          <div className="card flex justify-between items-center p-md">
            <div className="flex-col">
              <strong className="text-base mb-xs" style={{ display: "block" }}>OPENAI_API_KEY</strong>
              <span className="text-sm text-muted">For OpenAI GPT models and tools</span>
            </div>
            <span className={`pill flex items-center gap-xs ${apiEnvDiagnostic?.openai_api_key_set ? "ok" : "warn"}`}>
              {apiEnvDiagnostic?.openai_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
            </span>
          </div>

          <div className="card flex justify-between items-center p-md">
            <div className="flex-col">
              <strong className="text-base mb-xs" style={{ display: "block" }}>GEMINI_API_KEY</strong>
              <span className="text-sm text-muted">For Gemini reasoning and chat models</span>
            </div>
            <span className={`pill flex items-center gap-xs ${apiEnvDiagnostic?.gemini_api_key_set ? "ok" : "warn"}`}>
              {apiEnvDiagnostic?.gemini_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
            </span>
          </div>

          <div className="card flex justify-between items-center p-md">
            <div className="flex-col">
              <strong className="text-base mb-xs" style={{ display: "block" }}>ANTHROPIC_API_KEY</strong>
              <span className="text-sm text-muted">For Anthropic Claude models and agents</span>
            </div>
            <span className={`pill flex items-center gap-xs ${apiEnvDiagnostic?.anthropic_api_key_set ? "ok" : "warn"}`}>
              {apiEnvDiagnostic?.anthropic_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
            </span>
          </div>
        </div>

        <div className="p-md mt-sm" style={{
          borderRadius: "8px",
          backgroundColor: "rgba(255, 255, 255, 0.04)",
          borderLeft: "4px solid var(--border)"
        }}>
          <div className="text-base font-bold mb-xs">💡 How to configure environment variables permanently on macOS:</div>
          <p className="text-sm text-muted m-0">
            To ensure RepoDesk and your terminal sessions can securely load credentials, add them to your shell config file (typically <code>~/.zshrc</code>). Run the following commands in your terminal:
          </p>
          <pre style={{
            backgroundColor: "rgba(0, 0, 0, 0.3)",
            padding: "10px",
            borderRadius: "4px",
            fontSize: "12px",
            margin: "10px 0 0 0",
            fontFamily: "monospace",
            overflowX: "auto"
          }}>
            {`echo 'export OPENAI_API_KEY="your-openai-key"' >> ~/.zshrc
echo 'export GEMINI_API_KEY="your-gemini-key"' >> ~/.zshrc
echo 'export ANTHROPIC_API_KEY="your-anthropic-key"' >> ~/.zshrc
source ~/.zshrc`}
          </pre>
        </div>
      </section>
    </div>
  );
}
