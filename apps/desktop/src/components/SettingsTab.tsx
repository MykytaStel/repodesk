import React from "react";
import { statusTone, getString, Toggle } from "./SharedComponents";

interface SettingsTabProps {
  isBusy: boolean;
  providerSettings: any;
  setProviderSettings: (settings: any) => void;
  setupForm: any;
  setSetupForm: (form: any) => void;
  dbState: any;
  projectMemory: string;
  memoryAppendInput: string;
  setMemoryAppendInput: (val: string) => void;
  apiEnvDiagnostic: any;
  saveSettings: () => void;
  refreshAll: (label: string) => void;
  addProjectFromSetup: () => void;
  createTaskFromSetup: () => void;
  loadProjectMemory: () => void;
  handleAppendMemory: () => void;
}

export function SettingsTab({
  isBusy,
  providerSettings,
  setProviderSettings,
  setupForm,
  setSetupForm,
  dbState,
  projectMemory,
  memoryAppendInput,
  setMemoryAppendInput,
  apiEnvDiagnostic,
  saveSettings,
  refreshAll,
  addProjectFromSetup,
  createTaskFromSetup,
  loadProjectMemory,
  handleAppendMemory,
}: SettingsTabProps) {
  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Settings</p>
        <h1>Project, task, and provider controls.</h1>
        <p className="lead">Provider settings store URLs, toggles, and environment variable names only. Raw API keys stay outside RepoDesk settings.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void saveSettings()} disabled={isBusy}>Save provider settings</button>
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing settings")} disabled={isBusy}>Refresh</button>
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">Connect project</p><h2>Active workspace</h2>
        <div className="form-stack">
          <label>Project name<input value={setupForm.projectName} onChange={(event) => setSetupForm({ ...setupForm, projectName: event.target.value })} /></label>
          <label>Project path<input value={setupForm.projectPath} onChange={(event) => setSetupForm({ ...setupForm, projectPath: event.target.value })} placeholder="/Users/mykyta/Documents/projects/repodesk" /></label>
          <label>Project type<input value={setupForm.projectType} onChange={(event) => setSetupForm({ ...setupForm, projectType: event.target.value })} /></label>
          <label>Main language<input value={setupForm.mainLanguage} onChange={(event) => setSetupForm({ ...setupForm, mainLanguage: event.target.value })} /></label>
          <button className="primary-button full" onClick={() => void addProjectFromSetup()} disabled={isBusy}>Add and activate project</button>
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">Task</p><h2>Create active task</h2>
        <div className="form-stack">
          <label>Task title<input value={setupForm.taskTitle} onChange={(event) => setSetupForm({ ...setupForm, taskTitle: event.target.value })} /></label>
          <button className="primary-button full" onClick={() => void createTaskFromSetup()} disabled={isBusy}>Create task</button>
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Provider settings</p><h2>Runtime configuration</h2></div><span className={`pill ${statusTone(Boolean(dbState))}`}>DB {getString(dbState, "ok", "-")}</span></div>
        <div className="settings-grid">
          <Toggle label="Ollama enabled" checked={providerSettings.ollama_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, ollama_enabled: value })} />
          <Toggle label="LM Studio enabled" checked={providerSettings.lm_studio_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, lm_studio_enabled: value })} />
          <Toggle label="Llamafile enabled" checked={providerSettings.llamafile_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, llamafile_enabled: value })} />
          <Toggle label="LocalAI enabled" checked={providerSettings.localai_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, localai_enabled: value })} />
          <Toggle label="ChatGPT manual enabled" checked={providerSettings.chatgpt_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, chatgpt_enabled: value })} />
          <Toggle label="Codex enabled" checked={providerSettings.codex_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, codex_enabled: value })} />
          <Toggle label="Gemini manual enabled" checked={providerSettings.gemini_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, gemini_enabled: value })} />
          <Toggle label="OpenAI API enabled" checked={providerSettings.openai_api_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, openai_api_enabled: value })} />
          <Toggle label="Gemini API enabled" checked={providerSettings.gemini_api_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, gemini_api_enabled: value })} />
          <Toggle label="Allow paid agents" checked={providerSettings.allow_paid_agents} onChange={(value) => setProviderSettings({ ...providerSettings, allow_paid_agents: value })} />
          <label>Codex quota proxy<select value={providerSettings.codex_quota_status} onChange={(event) => setProviderSettings({ ...providerSettings, codex_quota_status: event.target.value })}>
            <option value="unknown">unknown</option>
            <option value="available">available</option>
            <option value="limited">limited</option>
            <option value="empty">empty</option>
          </select></label>
          <label>Ollama URL<input value={providerSettings.ollama_url} onChange={(event) => setProviderSettings({ ...providerSettings, ollama_url: event.target.value })} /></label>
          <label>Ollama default model<input value={providerSettings.ollama_model} onChange={(event) => setProviderSettings({ ...providerSettings, ollama_model: event.target.value })} /></label>
          <label>LM Studio URL<input value={providerSettings.lm_studio_url} onChange={(event) => setProviderSettings({ ...providerSettings, lm_studio_url: event.target.value })} /></label>
          <label>Llamafile URL<input value={providerSettings.llamafile_url} onChange={(event) => setProviderSettings({ ...providerSettings, llamafile_url: event.target.value })} /></label>
          <label>LocalAI URL<input value={providerSettings.localai_url} onChange={(event) => setProviderSettings({ ...providerSettings, localai_url: event.target.value })} /></label>
          <label>OpenAI key env var<input value={providerSettings.openai_api_key_env_var} onChange={(event) => setProviderSettings({ ...providerSettings, openai_api_key_env_var: event.target.value })} /></label>
          <label>Gemini key env var<input value={providerSettings.gemini_api_key_env_var} onChange={(event) => setProviderSettings({ ...providerSettings, gemini_api_key_env_var: event.target.value })} /></label>
          <label>Patch provider<input value={providerSettings.preferred_patch_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_patch_provider: event.target.value })} /></label>
          <label>Compression provider<input value={providerSettings.preferred_compression_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_compression_provider: event.target.value })} /></label>
          <label>Review provider<input value={providerSettings.preferred_review_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_review_provider: event.target.value })} /></label>
          <label className="span-2">Notes<textarea rows={3} value={providerSettings.notes} onChange={(event) => setProviderSettings({ ...providerSettings, notes: event.target.value })} /></label>
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
        <pre className="code-panel compact" style={{ whiteSpace: "pre-wrap", maxHeight: "250px", marginBottom: "14px", overflowY: "auto" }}>
          {projectMemory || "No guidelines or memory logs saved yet."}
        </pre>
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
          <button className="primary-button" onClick={() => void handleAppendMemory()} disabled={isBusy || !memoryAppendInput.trim()}>
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
