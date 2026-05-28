import React, { useState } from "react";
import { useMemory } from "./useMemory";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { formatNumber } from "../../shared/utils/helpers";
import type { MemoryEntry } from "../../shared/api/api";

const CATEGORIES = ["general", "decision", "constraint", "context", "risk", "pattern"] as const;
type Category = (typeof CATEGORIES)[number];

const CATEGORY_TONE: Record<Category, string> = {
  general: "neutral",
  decision: "ok",
  constraint: "warn",
  context: "neutral",
  risk: "danger",
  pattern: "ok",
};

export function MemoryTab() {
  const { hasProject, projectName } = useWorkspace();
  const { entries, isLoading, addEntry, isAdding, addError } = useMemory();

  const [content, setContent] = useState("");
  const [category, setCategory] = useState<Category>("general");
  const [tagsRaw, setTagsRaw] = useState("");
  const [filterCategory, setFilterCategory] = useState<Category | "all">("all");

  async function handleAdd() {
    const trimmed = content.trim();
    if (!trimmed) return;
    const tags = tagsRaw
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);
    await addEntry({ content: trimmed, category, tags });
    setContent("");
    setTagsRaw("");
  }

  const filtered: MemoryEntry[] =
    filterCategory === "all" ? entries : entries.filter((e) => e.category === filterCategory);

  if (!hasProject) {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Memory</p>
          <h1>No project selected</h1>
          <p className="lead">Open Settings and connect a project to start recording memory entries.</p>
        </section>
      </div>
    );
  }

  return (
    <div className="content-grid">
      {/* Hero */}
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Project Memory</p>
        <h1>Persistent context for <em style={{ fontStyle: "normal", color: "var(--accent)" }}>{projectName}</em></h1>
        <p className="lead">
          Record decisions, constraints, patterns and risks that AI agents should always know about.
          Entries are stored locally in the RepoDesk DB.
        </p>
      </section>

      {/* Stats strip */}
      <section className="panel">
        <p className="eyebrow">Total entries</p>
        <h2>{formatNumber(entries.length)}</h2>
        <p className="muted">across all categories</p>
      </section>
      <section className="panel">
        <p className="eyebrow">Decisions</p>
        <h2>{formatNumber(entries.filter((e) => e.category === "decision").length)}</h2>
        <p className="muted">architectural decisions</p>
      </section>
      <section className="panel">
        <p className="eyebrow">Constraints</p>
        <h2>{formatNumber(entries.filter((e) => e.category === "constraint").length)}</h2>
        <p className="muted">hard limits &amp; rules</p>
      </section>
      <section className="panel">
        <p className="eyebrow">Risks</p>
        <h2>{formatNumber(entries.filter((e) => e.category === "risk").length)}</h2>
        <p className="muted">known risk items</p>
      </section>

      {/* Add new entry */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">New entry</p>
            <h2>Add memory</h2>
          </div>
        </div>
        <div className="form-stack">
          <label>
            Content
            <textarea
              id="memory-content"
              rows={3}
              placeholder="Describe a decision, constraint, context item or risk..."
              value={content}
              onChange={(e) => setContent(e.target.value)}
              style={{ resize: "vertical", minHeight: 80 }}
            />
          </label>
          <div className="settings-grid">
            <label>
              Category
              <select
                id="memory-category"
                value={category}
                onChange={(e) => setCategory(e.target.value as Category)}
              >
                {CATEGORIES.map((cat) => (
                  <option key={cat} value={cat}>
                    {cat.charAt(0).toUpperCase() + cat.slice(1)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Tags <span style={{ fontWeight: 400 }}>(comma-separated)</span>
              <input
                id="memory-tags"
                type="text"
                placeholder="e.g. auth, payments, api"
                value={tagsRaw}
                onChange={(e) => setTagsRaw(e.target.value)}
              />
            </label>
          </div>
          {addError && (
            <div className="notice warn">{addError.message}</div>
          )}
          <div className="button-row compact-buttons">
            <button
              className="primary-button"
              onClick={() => void handleAdd()}
              disabled={isAdding || !content.trim()}
            >
              {isAdding ? "Saving…" : "Save entry"}
            </button>
          </div>
        </div>
      </section>

      {/* Filter bar */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Memory log</p>
            <h2>Stored entries</h2>
          </div>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            <button
              id="memory-filter-all"
              className={`tiny-button ${filterCategory === "all" ? "active" : ""}`}
              onClick={() => setFilterCategory("all")}
            >
              All ({entries.length})
            </button>
            {CATEGORIES.map((cat) => (
              <button
                key={cat}
                id={`memory-filter-${cat}`}
                className={`tiny-button ${filterCategory === cat ? "active" : ""}`}
                onClick={() => setFilterCategory(cat)}
              >
                {cat} ({entries.filter((e) => e.category === cat).length})
              </button>
            ))}
          </div>
        </div>

        {isLoading ? (
          <p className="muted">Loading memory entries…</p>
        ) : filtered.length === 0 ? (
          <p className="muted">No entries yet. Add the first one above.</p>
        ) : (
          <div className="table-list">
            {filtered.map((entry) => (
              <MemoryRow key={entry.id} entry={entry} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function MemoryRow({ entry }: { entry: MemoryEntry }) {
  const tone = CATEGORY_TONE[entry.category as Category] ?? "neutral";
  const date = new Date(entry.timestamp).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  return (
    <div className="table-row" style={{ alignItems: "flex-start", gridTemplateColumns: "1fr auto" }}>
      <div>
        <strong style={{ display: "block", marginBottom: 4 }}>{entry.content}</strong>
        {entry.tags.length > 0 && (
          <div style={{ display: "flex", gap: 5, flexWrap: "wrap", marginTop: 6 }}>
            {entry.tags.map((tag) => (
              <code key={tag}>{tag}</code>
            ))}
          </div>
        )}
      </div>
      <div className="row-meta">
        <span className={`pill ${tone}`}>{entry.category}</span>
        <span className="muted" style={{ fontSize: 12 }}>{date}</span>
      </div>
    </div>
  );
}
