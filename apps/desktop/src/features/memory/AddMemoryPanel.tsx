import { CATEGORIES, type Category } from "./constants";

export function AddMemoryPanel({
  content,
  category,
  tagsRaw,
  pending,
  onContentChange,
  onCategoryChange,
  onTagsChange,
  onAdd,
}: {
  content: string;
  category: Category;
  tagsRaw: string;
  pending: boolean;
  onContentChange: (content: string) => void;
  onCategoryChange: (category: Category) => void;
  onTagsChange: (tags: string) => void;
  onAdd: () => void;
}) {
  return (
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
            placeholder="A decision, constraint, risk, or pattern every agent should know..."
            value={content}
            onChange={(event) => onContentChange(event.target.value)}
            style={{ resize: "vertical", minHeight: 60 }}
          />
        </label>
        <div className="settings-grid">
          <label>
            Category
            <select value={category} onChange={(event) => onCategoryChange(event.target.value as Category)}>
              {CATEGORIES.map((item) => (
                <option key={item} value={item}>
                  {item}
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
              onChange={(event) => onTagsChange(event.target.value)}
            />
          </label>
        </div>
        <div className="button-row compact-buttons">
          <button className="primary-button" onClick={onAdd} disabled={pending || !content.trim()}>
            {pending ? "Saving..." : "Save entry"}
          </button>
        </div>
      </div>
    </section>
  );
}
