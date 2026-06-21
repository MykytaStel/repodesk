import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../../shared/api/customProviders";
import type { CustomProvider, ProviderPreset } from "../../shared/api/customProviders";

// Settings panel to add/edit/remove OpenAI-compatible providers so RepoDesk
// isn't limited to the built-in vendors — DeepSeek, Groq, OpenRouter, Together,
// Mistral, xAI, or any self-hosted server that speaks the Chat Completions API.
const BLANK: CustomProvider = {
  id: "",
  label: "",
  base_url: "",
  api_key: "",
  default_model: "",
  enabled: true,
};

export function CustomProvidersPanel() {
  const queryClient = useQueryClient();
  const providers = useQuery({ queryKey: ["custom-providers"], queryFn: api.listCustomProviders });
  const presets = useQuery({ queryKey: ["custom-provider-presets"], queryFn: api.customProviderPresets });
  const [draft, setDraft] = useState<CustomProvider>(BLANK);
  const [open, setOpen] = useState(false);

  const list = providers.data ?? [];
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["custom-providers"] });

  const save = useMutation({
    mutationFn: (p: CustomProvider) => api.saveCustomProvider(p),
    onSuccess: () => {
      setDraft(BLANK);
      setOpen(false);
      refresh();
      // New provider can change routing/health, so refresh those too.
      queryClient.invalidateQueries();
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteCustomProvider(id),
    onSuccess: refresh,
  });

  const error = (save.error as Error | null)?.message ?? (remove.error as Error | null)?.message ?? null;

  const applyPreset = (preset: ProviderPreset) =>
    setDraft({
      ...draft,
      id: preset.id,
      label: preset.label,
      base_url: preset.base_url,
      default_model: preset.default_model,
    });

  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Models</p>
          <h2>Custom / OpenAI-compatible providers</h2>
        </div>
        <button className="primary-button" onClick={() => { setDraft(BLANK); setOpen((v) => !v); }}>
          {open ? "Close" : "Add provider"}
        </button>
      </div>
      <p className="muted">
        Add any OpenAI-compatible endpoint (DeepSeek, Groq, OpenRouter, Mistral, xAI, or a
        self-hosted server). Once enabled, it's routable like any other model and its models are
        discovered in Models &amp; Cost.
      </p>

      {error && <div className="notice danger">{error}</div>}

      {open && (
        <div className="custom-provider-form">
          <div className="preset-row">
            <span className="muted">Preset:</span>
            {(presets.data ?? []).map((preset) => (
              <button key={preset.id} className="tiny-button ghost-button" onClick={() => applyPreset(preset)}>
                {preset.label}
              </button>
            ))}
          </div>
          <div className="playbook-form">
            <label>
              Name
              <input value={draft.label} onChange={(e) => setDraft({ ...draft, label: e.target.value })} placeholder="DeepSeek" />
            </label>
            <label>
              Id
              <input value={draft.id} onChange={(e) => setDraft({ ...draft, id: e.target.value })} placeholder="deepseek" />
            </label>
            <label>
              Base URL
              <input value={draft.base_url} onChange={(e) => setDraft({ ...draft, base_url: e.target.value })} placeholder="https://api.deepseek.com" />
            </label>
            <label>
              Default model
              <input value={draft.default_model} onChange={(e) => setDraft({ ...draft, default_model: e.target.value })} placeholder="deepseek-chat" />
            </label>
            <label>
              API key
              <input type="password" value={draft.api_key} onChange={(e) => setDraft({ ...draft, api_key: e.target.value })} placeholder="sk-…" />
            </label>
            <label className="playbook-form-check">
              <input type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} />
              Enabled
            </label>
          </div>
          <div className="phase-actions">
            <button className="primary-button" onClick={() => save.mutate(draft)} disabled={save.isPending || !draft.label.trim() || !draft.base_url.trim()}>
              {save.isPending ? "Saving…" : "Save provider"}
            </button>
          </div>
        </div>
      )}

      {providers.isLoading ? (
        <p className="muted">Loading…</p>
      ) : list.length === 0 ? (
        <p className="muted">No custom providers yet.</p>
      ) : (
        <div className="table-list">
          {list.map((p) => (
            <div className="table-row" key={p.id}>
              <div>
                <strong>{p.label} <span className="muted" style={{ fontWeight: "normal" }}>({p.id})</span></strong>
                <span>{p.base_url} · {p.default_model || "model on request"}{p.api_key ? " · key set" : " · no key"}</span>
              </div>
              <div className="row-meta">
                <span className={`pill ${p.enabled ? "ok" : "neutral"}`}>{p.enabled ? "enabled" : "disabled"}</span>
                <button className="tiny-button ghost-button" onClick={() => { setDraft(p); setOpen(true); }}>Edit</button>
                <button className="tiny-button link-cta" onClick={() => remove.mutate(p.id)} disabled={remove.isPending}>Delete</button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
