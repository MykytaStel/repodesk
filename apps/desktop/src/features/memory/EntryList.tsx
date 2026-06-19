import { useState } from "react";
import type { MemoryEntry } from "../../shared/api/memory";
import { CATEGORIES, CATEGORY_TONE, type Category, type EntryStatusFilter } from "./constants";
import { parseTags } from "./utils";
import { EmptyState } from "../../shared/ui/SharedComponents";

export function EntryList({
  entries,
  loading,
  search,
  category,
  status,
  onSearchChange,
  onCategoryChange,
  onStatusChange,
  onPin,
  onArchive,
  onDelete,
  onSaveEdit,
}: {
  entries: MemoryEntry[];
  loading: boolean;
  search: string;
  category: Category | "all";
  status: EntryStatusFilter;
  onSearchChange: (search: string) => void;
  onCategoryChange: (category: Category | "all") => void;
  onStatusChange: (status: EntryStatusFilter) => void;
  onPin: (id: number, pinned: boolean) => void;
  onArchive: (entry: MemoryEntry) => void;
  onDelete: (id: number) => void;
  onSaveEdit: (id: number, content: string, category: string, tags: string[]) => void;
}) {
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Brain log</p>
          <h2>Stored entries</h2>
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {(["active", "archived", "all"] as const).map((item) => (
            <button
              key={item}
              className={`tiny-button ${status === item ? "active" : ""}`}
              onClick={() => onStatusChange(item)}
            >
              {item}
            </button>
          ))}
        </div>
      </div>

      <EntryFilters
        search={search}
        category={category}
        onSearchChange={onSearchChange}
        onCategoryChange={onCategoryChange}
      />

      {loading ? (
        <p className="muted">Loading...</p>
      ) : entries.length === 0 ? (
        <EmptyState message="No entries match." hint="Adjust your filters or add a new memory." />
      ) : (
        <div className="table-list">
          {entries.map((entry) => (
            <MemoryRow
              key={entry.id}
              entry={entry}
              onPin={(pinned) => onPin(entry.id, pinned)}
              onArchive={() => onArchive(entry)}
              onDelete={() => onDelete(entry.id)}
              onSaveEdit={(content, nextCategory, tags) => onSaveEdit(entry.id, content, nextCategory, tags)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function EntryFilters({
  search,
  category,
  onSearchChange,
  onCategoryChange,
}: {
  search: string;
  category: Category | "all";
  onSearchChange: (search: string) => void;
  onCategoryChange: (category: Category | "all") => void;
}) {
  return (
    <div className="settings-grid" style={{ marginBottom: 10 }}>
      <label>
        Search
        <input
          type="text"
          placeholder="Filter by content, tag, or agent..."
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
        />
      </label>
      <label>
        Category
        <select value={category} onChange={(event) => onCategoryChange(event.target.value as Category | "all")}>
          <option value="all">all</option>
          {CATEGORIES.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </label>
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

  function save() {
    onSaveEdit(draft.trim(), draftCategory, parseTags(draftTags));
    setEditing(false);
  }

  if (editing) {
    return (
      <div className="table-row flex-col items-start gap-sm" style={{ paddingBottom: 12 }}>
        <textarea
          rows={2}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          style={{ width: "100%", resize: "vertical" }}
        />
        <div className="settings-grid" style={{ width: "100%" }}>
          <label>
            Category
            <select value={draftCategory} onChange={(event) => setDraftCategory(event.target.value)}>
              {CATEGORIES.map((item) => (
                <option key={item} value={item}>
                  {item}
                </option>
              ))}
            </select>
          </label>
          <label>
            Tags
            <input type="text" value={draftTags} onChange={(event) => setDraftTags(event.target.value)} />
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

  const tone = CATEGORY_TONE[entry.category] ?? "neutral";
  const provenanceTone = entry.source === "human" ? "neutral" : "ok";
  const date = new Date(entry.timestamp).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  return (
    <div className="table-row" style={{ alignItems: "flex-start", gridTemplateColumns: "1fr auto" }}>
      <div>
        <strong style={{ display: "block", marginBottom: 4, opacity: entry.status === "active" ? 1 : 0.55 }}>
          {entry.pinned ? "[Pinned] " : ""}
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
        {entry.status !== "active" && <span className="muted" style={{ fontSize: 11 }}>{entry.status}</span>}
      </div>
    </div>
  );
}
