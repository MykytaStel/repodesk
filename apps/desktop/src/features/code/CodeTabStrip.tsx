import { StatusBadge } from "../../shared/ui/primitives";
import { codeFileStatusSemantic } from "./codeSemantic";
import { fileName, type EditorTab } from "./codeTabs";
import { LibraryTabBadge } from "./LibraryTabBadge";

export function CodeTabStrip({
  tabs,
  activeTabId,
  onSelect,
  onClose,
}: {
  tabs: EditorTab[];
  activeTabId: string | null;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
}) {
  return (
    <div className="code-tab-strip" role="tablist" aria-label="Open files">
      {tabs.length === 0 ? <span className="code-tabs-empty">Open a file from Explorer.</span> : null}
      {tabs.map((tab) => {
        const status = codeFileStatusSemantic(tab.status);
        return (
          <div className={`code-file-tab${tab.id === activeTabId ? " active" : ""}`} key={tab.id}>
            <button
              type="button"
              role="tab"
              aria-selected={tab.id === activeTabId}
              className="code-file-tab-select"
              onClick={() => onSelect(tab.id)}
              title={tab.path}
            >
              <span>{fileName(tab.path)}</span>
              {tab.kind === "library" ? <LibraryTabBadge /> : null}
              {tab.recoveredDraft ? <small className="code-draft-badge">recovered</small> : null}
              {tab.kind === "workspace" && tab.status !== "clean" ? (
                <StatusBadge label={status.label} tone={status.tone} ariaLabel={status.detail ?? status.label} />
              ) : null}
              {tab.dirty ? <i aria-label="Unsaved">●</i> : null}
            </button>
            <button
              type="button"
              className="code-tab-close"
              aria-label={`Close ${fileName(tab.path)}`}
              onClick={() => onClose(tab.id)}
            >×</button>
          </div>
        );
      })}
    </div>
  );
}