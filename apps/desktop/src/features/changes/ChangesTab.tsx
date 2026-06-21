import { lazy, Suspense, useState } from "react";

// "Changes" merges the workspace (Git) and changed-files/review (Code) surfaces
// behind one primary nav entry, switched by a segmented sub-nav.
const GitTab = lazy(() => import("../git/GitTab").then((m) => ({ default: m.GitTab })));
const CodeTab = lazy(() => import("../code/CodeTab").then((m) => ({ default: m.CodeTab })));

type ChangesView = "workspace" | "code";

const VIEWS: { id: ChangesView; label: string }[] = [
  { id: "workspace", label: "Workspace & diffs" },
  { id: "code", label: "Changed files & review" },
];

export function ChangesTab() {
  const [view, setView] = useState<ChangesView>("workspace");
  return (
    <div className="subnav-host">
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
