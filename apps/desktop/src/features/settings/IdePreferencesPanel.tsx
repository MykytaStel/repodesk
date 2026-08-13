import { useIdePreferences, saveIdePreferences, resetIdePreferences } from "../code/idePreferences";
import "../code/ide-chrome.css";

export function IdePreferencesPanel() {
  const preferences = useIdePreferences();

  return (
    <section className="panel wide-panel flex-col gap-lg" aria-labelledby="ide-preferences-title">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">IDE</p>
          <h2 id="ide-preferences-title">Code workspace</h2>
          <p className="muted text-sm">Local presentation preferences for the editor and Explorer.</p>
        </div>
        <button type="button" className="ghost-button" onClick={() => resetIdePreferences()}>
          Reset defaults
        </button>
      </div>

      <div className="settings-grid">
        <label>
          Editor font size
          <select
            aria-label="Editor font size"
            value={preferences.editorFontSize}
            onChange={(event) => saveIdePreferences({ editorFontSize: Number(event.target.value) })}
          >
            {[11, 12, 13, 14, 15, 16, 18].map((size) => <option key={size} value={size}>{size}px</option>)}
          </select>
        </label>
        <label>
          Tab size
          <select
            aria-label="Editor tab size"
            value={preferences.tabSize}
            onChange={(event) => saveIdePreferences({ tabSize: Number(event.target.value) as 2 | 4 | 8 })}
          >
            <option value={2}>2 spaces</option>
            <option value={4}>4 spaces</option>
            <option value={8}>8 spaces</option>
          </select>
        </label>
        <label>
          Explorer density
          <select
            aria-label="Explorer density"
            value={preferences.explorerDensity}
            onChange={(event) => saveIdePreferences({ explorerDensity: event.target.value as "compact" | "comfortable" })}
          >
            <option value="compact">Compact</option>
            <option value="comfortable">Comfortable</option>
          </select>
        </label>
        <label className="settings-toggle-row">
          <input
            type="checkbox"
            checked={preferences.wordWrap}
            onChange={(event) => saveIdePreferences({ wordWrap: event.target.checked })}
          />
          <span>Word wrap</span>
        </label>
        <label className="settings-toggle-row">
          <input
            type="checkbox"
            checked={preferences.confirmDelete}
            onChange={(event) => saveIdePreferences({ confirmDelete: event.target.checked })}
          />
          <span>Confirm file deletion</span>
        </label>
      </div>
    </section>
  );
}
