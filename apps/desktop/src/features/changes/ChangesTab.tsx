import { lazy, Suspense, useState } from "react";
import { useGit } from "../git/useGit";

// "Changes" is one surface for everything between an agent's output and a
// commit: the workspace/diffs (Git) and the changed-files review (Code). A
// shared header summarizes the working tree; a segmented control swaps the view.
const GitTab = lazy(() => import("../git/GitTab").then((m) => ({ default: m.GitTab })));
const CodeTab = lazy(() => import("../code/CodeTab").then((m) => ({ default: m.CodeTab })));

type ChangesView = "workspace" | "code";

const VIEWS: { id: ChangesView; label: string }[] = [
  { id: "workspace", label: "Workspace & diffs" },
  { id: "code", label: "Changed files & review" },
];

export function ChangesTab() {
  const [view, setView] = useState<ChangesView>("workspace");
  const { branch, dirty, dirtyCount } = useGit();

  return (
    <div className="subnav-host changes-tab">
      <div className="changes-summary">
        <div>
          <p className="eyebrow">Changes</p>
          <strong>{branch}</strong>
        </div>
        <span className={`changes-pill ${dirty ? "dirty" : "clean"}`}>
          {dirty ? `${dirtyCount} uncommitted` : "Working tree clean"}
        </span>
      </div>
      <div className="subnav" role="tablist" aria-label="Changes views">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            role="tab"
            aria-selected={view === item.id}
            className={view === item.id ? "selected" : ""}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <Suspense fallback={<p className="muted">Loading…</p>}>
        {view === "workspace" ? <GitTab /> : <CodeTab />}
      </Suspense>
    </div>
  );
}
