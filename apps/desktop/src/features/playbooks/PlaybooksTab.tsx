import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { TabId } from "../../shared/types/api";
import * as api from "../../shared/api/playbooks";
import type { Playbook } from "../../shared/api/playbooks";
import "../../shared/ui/manual-import.css";
import "./playbooks-route.css";

const TARGETS: { id: TabId; label: string }[] = [
  { id: "work", label: "Work" },
  { id: "code", label: "Code" },
  { id: "changes", label: "Changes" },
  { id: "history", label: "Runs" },
  { id: "projects", label: "Projects" },
  { id: "settings", label: "Settings" },
];

const LEGACY_TARGETS: Record<string, TabId> = {
  dashboard: "work",
  git: "changes",
  orchestrate: "work",
  outcomes: "history",
  audit: "history",
  memory: "projects",
  playbooks: "projects",
  "models-cost": "settings",
  models: "settings",
  tokens: "settings",
  system: "settings",
};

function canonicalTarget(target: string): TabId {
  if (LEGACY_TARGETS[target]) return LEGACY_TARGETS[target];
  const canonical = TARGETS.find((item) => item.id === target);
  return canonical?.id ?? "work";
}

function targetLabel(target: string): string {
  const canonical = canonicalTarget(target);
  return TARGETS.find((item) => item.id === canonical)?.label ?? "Work";
}

const EMPTY_DRAFT: Playbook = {
  id: "",
  title: "",
  summary: "",
  target: "work",
  destination: "",
  action: "",
  artifact: "",
  starts_agent: false,
};

export function PlaybooksTab({ setActiveTab }: { setActiveTab: (tab: TabId, detail?: string) => void }) {
  const queryClient = useQueryClient();
  const playbooks = useQuery({ queryKey: ["playbooks"], queryFn: api.listPlaybooks });
  const [lastShortcut, setLastShortcut] = useState<Playbook | null>(null);
  const [draft, setDraft] = useState<Playbook>(EMPTY_DRAFT);
  const [showEditor, setShowEditor] = useState(false);
  const [importDoc, setImportDoc] = useState("");

  const list = playbooks.data ?? [];
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["playbooks"] });
  const save = useMutation({
    mutationFn: (pb: Playbook) => api.savePlaybook({ ...pb, target: canonicalTarget(pb.target) }),
    onSuccess: () => {
      setDraft(EMPTY_DRAFT);
      setShowEditor(false);
      void refresh();
    },
  });
  const remove = useMutation({ mutationFn: (id: string) => api.deletePlaybook(id), onSuccess: () => void refresh() });
  const importer = useMutation({
    mutationFn: (doc: string) => api.importPlaybooks(doc),
    onSuccess: () => {
      setImportDoc("");
      void refresh();
    },
  });

  const mutationError =
    (save.error as Error | null)?.message ??
    (remove.error as Error | null)?.message ??
    (importer.error as Error | null)?.message ??
    null;

  function openShortcut(pb: Playbook) {
    setLastShortcut(pb);
    const target = canonicalTarget(pb.target);
    setActiveTab(target, `${pb.title}: opened ${targetLabel(target)}.`);
  }

  function startEdit(pb: Playbook) {
    setDraft({ ...pb, target: canonicalTarget(pb.target) });
    setShowEditor(true);
  }

  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Work templates</p>
        <h1>Reusable workflow entry points</h1>
        <p className="lead">
          Templates capture a repeatable engineering starting point and always open the canonical surface that owns the work. They never create a hidden agent run.
        </p>
        <div className="button-row">
          <button
            className="primary-button"
            onClick={() => {
              setDraft(EMPTY_DRAFT);
              setShowEditor((value) => !value);
            }}
          >
            {showEditor ? "Close editor" : "New work template"}
          </button>
        </div>
      </section>

      {showEditor ? (
        <section className="panel">
          <div className="panel-title-row">
            <div><p className="eyebrow">{draft.id ? "Edit" : "New"}</p><h2>Template details</h2></div>
          </div>
          <div className="playbook-form">
            <label>Title<input value={draft.title} onChange={(event) => setDraft({ ...draft, title: event.target.value })} /></label>
            <label>
              Opens
              <select value={canonicalTarget(draft.target)} onChange={(event) => setDraft({ ...draft, target: event.target.value })}>
                {TARGETS.map((target) => <option key={target.id} value={target.id}>{target.label}</option>)}
              </select>
            </label>
            <label>Summary<input value={draft.summary} onChange={(event) => setDraft({ ...draft, summary: event.target.value })} /></label>
            <label>Destination<input value={draft.destination} placeholder="e.g. Work / Execute" onChange={(event) => setDraft({ ...draft, destination: event.target.value })} /></label>
            <label>Action<input value={draft.action} onChange={(event) => setDraft({ ...draft, action: event.target.value })} /></label>
            <label>Visible result<input value={draft.artifact} onChange={(event) => setDraft({ ...draft, artifact: event.target.value })} /></label>
            <label className="playbook-form-check">
              <input type="checkbox" checked={draft.starts_agent} onChange={(event) => setDraft({ ...draft, starts_agent: event.target.checked })} />
              Following this template can eventually start an executor
            </label>
          </div>
          <div className="phase-actions">
            <button className="primary-button" onClick={() => save.mutate(draft)} disabled={save.isPending || !draft.title.trim()}>
              {draft.id ? "Save changes" : "Create template"}
            </button>
          </div>
        </section>
      ) : null}

      <section className="panel">
        <div className="panel-title-row">
          <div><p className="eyebrow">Available</p><h2>Work templates</h2></div>
          <span className="pill">{list.length}</span>
        </div>

        {mutationError ? <div className="notice danger">{mutationError}</div> : null}
        {lastShortcut ? (
          <div className="notice ok playbook-result" role="status">
            <strong>{lastShortcut.title}</strong>
            <span>Opened {targetLabel(lastShortcut.target)}. Next visible result: {lastShortcut.artifact}</span>
          </div>
        ) : null}

        {playbooks.isLoading ? (
          <p className="muted">Loading work templates…</p>
        ) : list.length === 0 ? (
          <p className="muted">No work templates yet. Create one or import a set below.</p>
        ) : (
          <div className="playbook-list">
            {list.map((pb) => (
              <div className="check-card playbook-card" key={pb.id}>
                <div className="playbook-copy">
                  <strong>{pb.title}</strong>
                  <p className="muted">{pb.summary}</p>
                  <div className="playbook-route">
                    <div><span>Surface</span><strong>{targetLabel(pb.target)}</strong></div>
                    <div><span>Action</span><strong>{pb.action}</strong></div>
                    <div><span>Visible result</span><strong>{pb.artifact}</strong></div>
                  </div>
                </div>
                <div className="playbook-card-actions">
                  <span className={`pill ${pb.starts_agent ? "warn" : "neutral"}`}>{pb.starts_agent ? "Can reach executor" : "No hidden run"}</span>
                  <button className="tiny-button" onClick={() => openShortcut(pb)}>Open {targetLabel(pb.target)}</button>
                  <button className="tiny-button ghost-button" onClick={() => startEdit(pb)}>Edit</button>
                  <button className="tiny-button link-cta" onClick={() => remove.mutate(pb.id)} disabled={remove.isPending}>Delete</button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-title-row"><div><p className="eyebrow">Import</p><h2>Import work templates</h2></div></div>
        <p className="muted">Paste a TOML or JSON playbooks document. Legacy targets are normalized to canonical RepoDesk surfaces when edited or opened.</p>
        <textarea
          className="manual-import-input"
          rows={6}
          placeholder='[{ "title": "Review current change", "target": "changes", "summary": "…", "action": "…", "artifact": "…" }]'
          value={importDoc}
          onChange={(event) => setImportDoc(event.target.value)}
        />
        <div className="phase-actions">
          <button className="secondary-cta" onClick={() => importer.mutate(importDoc)} disabled={importer.isPending || !importDoc.trim()}>Import</button>
        </div>
      </section>
    </div>
  );
}
