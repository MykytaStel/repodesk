import React, { useMemo, useState } from "react";
import { useMemory } from "./useMemory";
import { formatNumber } from "../../shared/utils/helpers";
import { ErrorState } from "../../shared/ui/ErrorState";
import type { MemoryEntry, MemoryProposal } from "../../shared/api/api";

const CATEGORIES = ["general", "decision", "constraint", "context", "risk", "pattern"] as const;
type Category = (typeof CATEGORIES)[number];

const CATEGORY_TONE: Record<string, string> = {
  general: "neutral",
  decision: "ok",
  constraint: "warn",
  context: "neutral",
  risk: "danger",
  pattern: "ok",
};

const AGENTS = ["chatgpt", "codex", "gemini", "ollama", "claude", "human"] as const;

const KIND_TONE: Record<string, string> = {
  capture: "ok",
  dedup: "neutral",
  merge: "warn",
  conflict: "danger",
};

export function MemoryTab() {
  const m = useMemory();
  const {
    projectName,
    hasProject,
    entries,
    entriesLoading,
    proposals,
    preview,
  } = m;

  // Add-entry form
  const [content, setContent] = useState("");
  const [category, setCategory] = useState<Category>("decision");
  const [tagsRaw, setTagsRaw] = useState("");

  // Capture form
  const [captureAgent, setCaptureAgent] = useState<string>("chatgpt");
  const [captureText, setCaptureText] = useState("");

  // Entry list filters
  const [search, setSearch] = useState("");
  const [filterCategory, setFilterCategory] = useState<Category | "all">("all");
  const [filterStatus, setFilterStatus] = useState<"active" | "archived" | "all">("active");

  const entriesById = useMemo(() => {
    const map = new Map<number, MemoryEntry>();
    for (const e of entries) map.set(e.id, e);
    return map;
  }, [entries]);

  const pendingCount = proposals.length;
  const conflictCount = proposals.filter((p) => p.kind === "conflict").length;
  const activeCount = entries.filter((e) => e.status === "active").length;
  const pinnedCount = entries.filter((e) => e.pinned).length;

  const visibleEntries = entries.filter((e) => {
    if (filterStatus !== "all" && e.status !== filterStatus) return false;
    if (filterCategory !== "all" && e.category !== filterCategory) return false;
    if (search.trim()) {
      const q = search.toLowerCase();
      const hay = `${e.content} ${e.category} ${e.tags.join(" ")} ${e.agent}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });

  async function handleAdd() {
    const trimmed = content.trim();
    if (!trimmed) return;
    const tags = tagsRaw.split(",").map((t) => t.trim()).filter(Boolean);
    await m.addEntry.mutateAsync({ content: trimmed, category, tags });
    setContent("");
    setTagsRaw("");
  }

  async function handleCapture() {
    const text = captureText.trim();
    if (!text) return;
    await m.capture.mutateAsync({ agent: captureAgent, text });
    setCaptureText("");
  }

  if (!hasProject) {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Memory Brain</p>
          <h1>No project selected</h1>
          <p className="lead">Open Settings and connect a project to start building shared memory.</p>
        </section>
      </div>
    );
  }

  return (
    <div className="content-grid">
      {/* Hero */}
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Memory Brain</p>
        <h1>
          One shared brain for{" "}
          <em style={{ fontStyle: "normal", color: "var(--accent)" }}>{projectName}</em>
        </h1>
        <p className="lead">
          Capture what every AI produces, review proposals, resolve conflicts, and inject one
          curated memory into every agent prompt. Nothing changes the brain until you approve it.
        </p>
        <div className="button-row compact-buttons" style={{ marginTop: 12 }}>
          <button
            className="tiny-button"
            onClick={() => m.scan.mutate()}
            disabled={m.scan.isPending}
          >
            {m.scan.isPending ? "Scanning…" : "Scan for duplicates / conflicts"}
          </button>
          <button
            className="tiny-button"
            onClick={() => m.consolidate.mutate()}
            disabled={m.consolidate.isPending}
          >
            {m.consolidate.isPending ? "Writing…" : "Rebuild memory.md"}
          </button>
        </div>
      </section>

      {/* Overview strip */}
      <BrainMetric label="Active entries" value={formatNumber(activeCount)} detail="in the brain" tone="ok" />
      <BrainMetric label="Pinned" value={formatNumber(pinnedCount)} detail="always in context" tone="neutral" />
      <BrainMetric
        label="Pending proposals"
        value={formatNumber(pendingCount)}
        detail="awaiting review"
        tone={pendingCount > 0 ? "warn" : "neutral"}
      />
      <BrainMetric
        label="Open conflicts"
        value={formatNumber(conflictCount)}
        detail="need resolution"
        tone={conflictCount > 0 ? "danger" : "neutral"}
      />

      {/* What the AI sees */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">What the AI sees</p>
            <h2>Injected memory slice</h2>
          </div>
          {preview && (
            <div className="row-meta">
              <span className="pill">{formatNumber(preview.estimated_tokens)} tokens</span>
              <span className="muted" style={{ fontSize: 12 }}>
                {preview.included}/{preview.total_active} included
                {preview.excluded > 0 ? ` · ${preview.excluded} dropped (budget)` : ""}
              </span>
            </div>
          )}
        </div>
        <pre className="scroll-area" style={{ maxHeight: 220, whiteSpace: "pre-wrap", fontSize: 13 }}>
          {preview?.markdown ?? (m.previewLoading ? "Loading…" : "No active memory yet.")}
        </pre>
        <p className="muted" style={{ fontSize: 12 }}>
          This exact slice is ranked (pinned → task relevance → recency) and injected into
          context.md and the smart pack for every agent.
        </p>
      </section>

      {/* Capture from AI */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Merge between AI</p>
            <h2>Capture from a response</h2>
          </div>
        </div>
        <div className="form-stack">
          <label>
            Paste an AI response
            <textarea
              rows={4}
              placeholder="Paste what ChatGPT / Codex / Gemini / Ollama returned. RepoDesk extracts decisions, constraints, risks…"
              value={captureText}
              onChange={(e) => setCaptureText(e.target.value)}
              style={{ resize: "vertical", minHeight: 90 }}
            />
          </label>
          <div className="settings-grid">
            <label>
              Produced by
              <select value={captureAgent} onChange={(e) => setCaptureAgent(e.target.value)}>
                {AGENTS.map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {m.capture.error && <ErrorState compact scope="memory:capture" error={m.capture.error} />}
          {m.capture.data && (
            <div className="notice">
              Captured {m.capture.data.length} candidate(s) — see the review queue below.
            </div>
          )}
          <div className="button-row compact-buttons">
            <button
              className="primary-button"
              onClick={() => void handleCapture()}
              disabled={m.capture.isPending || !captureText.trim()}
            >
              {m.capture.isPending ? "Extracting…" : "Extract memory"}
            </button>
          </div>
        </div>
      </section>

      {/* Review queue */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Review queue</p>
            <h2>Pending proposals ({pendingCount})</h2>
          </div>
        </div>
        {pendingCount === 0 ? (
          <p className="muted">Nothing to review. Capture a response or run a scan.</p>
        ) : (
          <div className="table-list">
            {proposals.map((p) => (
              <ProposalRow
                key={p.id}
                proposal={p}
                entriesById={entriesById}
                onAccept={(keepId) => m.accept.mutate({ id: p.id, keepId })}
                onReject={() => m.reject.mutate(p.id)}
                onReconcile={() => m.reconcile.mutate(p.id)}
                busy={m.accept.isPending || m.reject.isPending || m.reconcile.isPending}
              />
            ))}
          </div>
        )}
        {m.reconcile.error && (
          <div style={{ marginTop: 8 }}>
            <ErrorState compact scope="memory:reconcile" error={m.reconcile.error} />
          </div>
        )}
      </section>

      {/* Add entry */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">New entry</p>
            <h2>Add memory by hand</h2>
          </div>
        </div>
        <div className="form-stack">
          <label>
            Content
            <textarea
              rows={2}
              placeholder="A decision, constraint, risk, or pattern every agent should know…"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              style={{ resize: "vertical", minHeight: 60 }}
            />
          </label>
          <div className="settings-grid">
            <label>
              Category
              <select value={category} onChange={(e) => setCategory(e.target.value as Category)}>
                {CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Tags <span style={{ fontWeight: 400 }}>(comma-separated)</span>
              <input
                type="text"
                placeholder="auth, payments, api"
                value={tagsRaw}
                onChange={(e) => setTagsRaw(e.target.value)}
              />
            </label>
          </div>
          <div className="button-row compact-buttons">
            <button
              className="primary-button"
              onClick={() => void handleAdd()}
              disabled={m.addEntry.isPending || !content.trim()}
            >
              {m.addEntry.isPending ? "Saving…" : "Save entry"}
            </button>
          </div>
        </div>
      </section>

      {/* Entry list */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Brain log</p>
            <h2>Stored entries</h2>
          </div>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            {(["active", "archived", "all"] as const).map((s) => (
              <button
                key={s}
                className={`tiny-button ${filterStatus === s ? "active" : ""}`}
                onClick={() => setFilterStatus(s)}
              >
                {s}
              </button>
            ))}
          </div>
        </div>

        <div className="settings-grid" style={{ marginBottom: 10 }}>
          <label>
            Search
            <input
              type="text"
              placeholder="Filter by content, tag, or agent…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </label>
          <label>
            Category
            <select
              value={filterCategory}
              onChange={(e) => setFilterCategory(e.target.value as Category | "all")}
            >
              <option value="all">all</option>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </label>
        </div>

        {entriesLoading ? (
          <p className="muted">Loading…</p>
        ) : visibleEntries.length === 0 ? (
          <p className="muted">No entries match.</p>
        ) : (
          <div className="table-list">
            {visibleEntries.map((entry) => (
              <MemoryRow
                key={entry.id}
                entry={entry}
                onPin={(pinned) => m.setPinned.mutate({ id: entry.id, pinned })}
                onArchive={() =>
                  m.setStatus.mutate({
                    id: entry.id,
                    status: entry.status === "archived" ? "active" : "archived",
                  })
                }
                onDelete={() => m.deleteEntry.mutate(entry.id)}
                onSaveEdit={(content, category, tags) =>
                  m.updateEntry.mutate({ id: entry.id, content, category, tags })
                }
              />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function BrainMetric({
  label,
  value,
  detail,
  tone,
}: {
  label: string;
  value: string;
  detail: string;
  tone: "ok" | "warn" | "danger" | "neutral";
}) {
  return (
    <section className={`panel metric ${tone}`}>
      <p className="eyebrow">{label}</p>
      <h2>{value}</h2>
      <p className="muted">{detail}</p>
    </section>
  );
}

function ProposalRow({
  proposal,
  entriesById,
  onAccept,
  onReject,
  onReconcile,
  busy,
}: {
  proposal: MemoryProposal;
  entriesById: Map<number, MemoryEntry>;
  onAccept: (keepId?: number | null) => void;
  onReject: () => void;
  onReconcile: () => void;
  busy: boolean;
}) {
  const tone = KIND_TONE[proposal.kind] ?? "neutral";
  const sources = proposal.payload.source_ids
    .map((id) => entriesById.get(id))
    .filter((e): e is MemoryEntry => Boolean(e));

  return (
    <div className="table-row flex-col items-start gap-sm" style={{ paddingBottom: 14 }}>
      <div className="w-full flex justify-between items-center">
        <span className={`pill ${tone}`}>{proposal.kind}</span>
        {proposal.payload.agent && <span className="muted" style={{ fontSize: 12 }}>{proposal.payload.agent}</span>}
      </div>
      <p style={{ margin: "4px 0" }}>{proposal.payload.rationale}</p>

      {proposal.payload.proposed && (
        <div className="route-list ok" style={{ width: "100%" }}>
          <strong>Proposed [{proposal.payload.proposed.category}]</strong>
          <span>{proposal.payload.proposed.content}</span>
        </div>
      )}

      {sources.length > 0 && (
        <div style={{ width: "100%", display: "flex", flexDirection: "column", gap: 4 }}>
          {sources.map((s) => (
            <div key={s.id} className="flex justify-between items-center" style={{ gap: 8 }}>
              <code style={{ flex: 1 }}>
                #{s.id} [{s.category}] {s.content}
              </code>
              {proposal.kind === "conflict" && (
                <button className="tiny-button" disabled={busy} onClick={() => onAccept(s.id)}>
                  Keep #{s.id}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      <div className="button-row compact-buttons" style={{ marginTop: 6 }}>
        {proposal.kind === "conflict" ? (
          <>
            <button
              className="tiny-button"
              disabled={busy}
              onClick={onReconcile}
              title="Use Ollama to write a single reconciled note"
            >
              Reconcile (Ollama)
            </button>
            {proposal.payload.proposed && (
              <button className="primary-button" disabled={busy} onClick={() => onAccept(null)}>
                Accept merged
              </button>
            )}
          </>
        ) : (
          <button className="primary-button" disabled={busy} onClick={() => onAccept(null)}>
            Accept
          </button>
        )}
        <button className="tiny-button" disabled={busy} onClick={onReject}>
          Reject
        </button>
      </div>
    </div>
  );
}

function MemoryRow({
  entry,
  onPin,
  onArchive,
  onDelete,
  onSaveEdit,
}: {
  entry: MemoryEntry;
  onPin: (pinned: boolean) => void;
  onArchive: () => void;
  onDelete: () => void;
  onSaveEdit: (content: string, category: string, tags: string[]) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(entry.content);
  const [draftCategory, setDraftCategory] = useState(entry.category);
  const [draftTags, setDraftTags] = useState(entry.tags.join(", "));

  const tone = CATEGORY_TONE[entry.category] ?? "neutral";
  const provenanceTone = entry.source === "human" ? "neutral" : "ok";
  const date = new Date(entry.timestamp).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  function save() {
    const tags = draftTags.split(",").map((t) => t.trim()).filter(Boolean);
    onSaveEdit(draft.trim(), draftCategory, tags);
    setEditing(false);
  }

  if (editing) {
    return (
      <div className="table-row flex-col items-start gap-sm" style={{ paddingBottom: 12 }}>
        <textarea
          rows={2}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          style={{ width: "100%", resize: "vertical" }}
        />
        <div className="settings-grid" style={{ width: "100%" }}>
          <label>
            Category
            <select value={draftCategory} onChange={(e) => setDraftCategory(e.target.value)}>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </label>
          <label>
            Tags
            <input type="text" value={draftTags} onChange={(e) => setDraftTags(e.target.value)} />
          </label>
        </div>
        <div className="button-row compact-buttons">
          <button className="primary-button" onClick={save} disabled={!draft.trim()}>
            Save
          </button>
          <button className="tiny-button" onClick={() => setEditing(false)}>
            Cancel
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="table-row" style={{ alignItems: "flex-start", gridTemplateColumns: "1fr auto" }}>
      <div>
        <strong style={{ display: "block", marginBottom: 4, opacity: entry.status === "active" ? 1 : 0.55 }}>
          {entry.pinned ? "📌 " : ""}
          {entry.content}
        </strong>
        <div style={{ display: "flex", gap: 5, flexWrap: "wrap", marginTop: 6, alignItems: "center" }}>
          <span className={`pill ${provenanceTone}`}>
            {entry.source === "ai" ? entry.agent || "ai" : entry.source}
          </span>
          {entry.tags.map((tag) => (
            <code key={tag}>{tag}</code>
          ))}
        </div>
        <div className="button-row compact-buttons" style={{ marginTop: 8 }}>
          <button className="tiny-button" onClick={() => onPin(!entry.pinned)}>
            {entry.pinned ? "Unpin" : "Pin"}
          </button>
          <button className="tiny-button" onClick={() => setEditing(true)}>
            Edit
          </button>
          <button className="tiny-button" onClick={onArchive}>
            {entry.status === "archived" ? "Restore" : "Archive"}
          </button>
          <button className="tiny-button danger" onClick={onDelete}>
            Delete
          </button>
        </div>
      </div>
      <div className="row-meta">
        <span className={`pill ${tone}`}>{entry.category}</span>
        <span className="muted" style={{ fontSize: 12 }}>{date}</span>
        {entry.status !== "active" && (
          <span className="muted" style={{ fontSize: 11 }}>{entry.status}</span>
        )}
      </div>
    </div>
  );
}
