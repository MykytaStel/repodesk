import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { TabId } from "../../shared/types/api";
import * as api from "../../shared/api/playbooks";
import type { Playbook } from "../../shared/api/playbooks";
import "../../shared/ui/manual-import.css";
import "./playbooks-route.css";

// Targets a playbook can open. Kept in sync with the primary surfaces; the value
// is a TabId so the shortcut routes through the same nav as everything else.
const TARGETS: { id: TabId; label: string }[] = [
  { id: "work", label: "Work" },
  { id: "changes", label: "Changes" },
  { id: "history", label: "History" },
  { id: "models-cost", label: "Models & Cost" },
  { id: "orchestrate", label: "Orchestrate" },
  { id: "settings", label: "Settings" },
];

function targetLabel(target: string): string {
  return TARGETS.find((t) => t.id === target)?.label ?? target;
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
    mutationFn: (pb: Playbook) => api.savePlaybook(pb),
    onSuccess: () => {
      setDraft(EMPTY_DRAFT);
      setShowEditor(false);
      refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deletePlaybook(id),
    onSuccess: refresh,
  });
  const importer = useMutation({
    mutationFn: (doc: string) => api.importPlaybooks(doc),
    onSuccess: () => {
      setImportDoc("");
      refresh();
    },
  });

  const mutationError =
    (save.error as Error | null)?.message ??
    (remove.error as Error | null)?.message ??
    (importer.error as Error | null)?.message ??
    null;

  function openShortcut(pb: Playbook) {
    setLastShortcut(pb);
    setActiveTab(pb.target as TabId, `${pb.title}: opened ${targetLabel(pb.target)}.`);
  }

  function startEdit(pb: Playbook) {
    setDraft(pb);
    setShowEditor(true);
  }

  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Playbooks</p>
        <h1>Workflow shortcuts</h1>
        <p className="lead">
          Each shortcut opens the surface that owns the work. Nothing starts an agent from here —
          they're saved, editable shortcuts you can author and share.
        </p>
        <div className="button-row">
          <button
            className="primary-button"
            onClick={() => {
              setDraft(EMPTY_DRAFT);
              setShowEditor((v) => !v);
            }}
          >
            {showEditor ? "Close editor" : "New playbook"}
          </button>
        </div>
      </section>

      {showEditor && (
        <section className="panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">{draft.id ? "Edit" : "New"}</p>
              <h2>Playbook details</h2>
            </div>
          </div>
          <div className="playbook-form">
            <label>
              Title
              <input value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} />
            </label>
            <label>
              Opens
              <select value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value })}>
                {TARGETS.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Summary
              <input value={draft.summary} onChange={(e) => setDraft({ ...draft, summary: e.target.value })} />
            </label>
            <label>
              Destination
              <input
                value={draft.destination}
                placeholder="e.g. Work / Execute"
                onChange={(e) => setDraft({ ...draft, destination: e.target.value })}
              />
            </label>
            <label>
              Action
              <input value={draft.action} onChange={(e) => setDraft({ ...draft, action: e.target.value })} />
            </label>
            <label>
              Visible result
              <input value={draft.artifact} onChange={(e) => setDraft({ ...draft, artifact: e.target.value })} />
            </label>
            <label className="playbook-form-check">
              <input
                type="checkbox"
                checked={draft.starts_agent}
                onChange={(e) => setDraft({ ...draft, starts_agent: e.target.checked })}
              />
              Following this eventually starts an agent
            </label>
          </div>
          <div className="phase-actions">
            <button
              className="primary-button"
              onClick={() => save.mutate(draft)}
              disabled={save.isPending || !draft.title.trim()}
            >
              {draft.id ? "Save changes" : "Create playbook"}
            </button>
          </div>
        </section>
      )}

      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Available</p>
            <h2>Playbook shortcuts</h2>
          </div>
          <span className="pill">{list.length}</span>
        </div>

        {mutationError && <div className="notice danger">{mutationError}</div>}

        {lastShortcut && (
          <div className="notice ok playbook-result" role="status">
            <strong>{lastShortcut.title}</strong>
            <span>
              Opened {targetLabel(lastShortcut.target)}. Next visible result: {lastShortcut.artifact}
            </span>
          </div>
        )}

        {playbooks.isLoading ? (
          <p className="muted">Loading playbooks…</p>
        ) : list.length === 0 ? (
          <p className="muted">No playbooks yet. Create one or import a set below.</p>
        ) : (
          <div className="playbook-list">
            {list.map((pb) => (
              <div className="check-card playbook-card" key={pb.id}>
                <div className="playbook-copy">
                  <strong>{pb.title}</strong>
                  <p className="muted">{pb.summary}</p>
                  <div className="playbook-route">
                    <div>
                      <span>Destination</span>
                      <strong>{pb.destination || targetLabel(pb.target)}</strong>
                    </div>
                    <div>
                      <span>Action</span>
                      <strong>{pb.action}</strong>
                    </div>
                    <div>
                      <span>Visible result</span>
                      <strong>{pb.artifact}</strong>
                    </div>
                  </div>
                </div>
                <div className="playbook-card-actions">
                  <span className={`pill ${pb.starts_agent ? "warn" : "neutral"}`}>
                    {pb.starts_agent ? "Starts agent" : "No hidden run"}
                  </span>
                  <button className="tiny-button" onClick={() => openShortcut(pb)}>
                    Open {targetLabel(pb.target)}
                  </button>
                  <button className="tiny-button ghost-button" onClick={() => startEdit(pb)}>
                    Edit
                  </button>
                  <button
                    className="tiny-button link-cta"
                    onClick={() => remove.mutate(pb.id)}
                    disabled={remove.isPending}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Import</p>
            <h2>Import playbooks</h2>
          </div>
        </div>
        <p className="muted">
          Paste a TOML or JSON document (a <code>playbooks</code> list, or a bare array). Entries
          merge by id.
        </p>
        <textarea
          className="manual-import-input"
          rows={6}
          placeholder='[{ "title": "My shortcut", "target": "work", "summary": "…", "action": "…", "artifact": "…" }]'
          value={importDoc}
          onChange={(e) => setImportDoc(e.target.value)}
        />
        <div className="phase-actions">
          <button
            className="secondary-cta"
            onClick={() => importer.mutate(importDoc)}
            disabled={importer.isPending || !importDoc.trim()}
          >
            Import
          </button>
        </div>
      </section>
    </div>
  );
}
