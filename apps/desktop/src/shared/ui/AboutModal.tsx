// Always-available "What is RepoDesk?" explainer. Onboarding on the Dashboard
// only shows before a project is connected and then disappears; this panel is
// reachable from the header at any time so the app can always answer "what does
// this do and where do I start?".

const PHASES: { name: string; blurb: string }[] = [
  { name: "Scope", blurb: "Lock work to one repo + one task." },
  { name: "Prepare", blurb: "Build bounded context, scan it, route the task." },
  { name: "Execute", blurb: "Run the chosen agent in an isolated worktree." },
  { name: "Review", blurb: "Accept or reject the exact changeset." },
  { name: "Verify", blurb: "Run project checks against the reviewed tree." },
  { name: "Finish", blurb: "Commit only the reviewed, staged changes." },
];

const SURFACES: { name: string; blurb: string }[] = [
  { name: "Work", blurb: "The home flow, Scope → Finish." },
  { name: "Changes", blurb: "Diffs, files and code review before commit." },
  { name: "History", blurb: "Past runs, memory and the audit trail." },
  { name: "Models & Cost", blurb: "Which models are reachable and what work costs." },
  { name: "Settings", blurb: "Projects, providers and keys." },
];

export function AboutModal({
  isOpen,
  onClose,
  onGetStarted,
}: {
  isOpen: boolean;
  onClose: () => void;
  onGetStarted: () => void;
}) {
  if (!isOpen) return null;
  return (
    <div
      className="modal-backdrop"
      onClick={onClose}
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
    >
      <div
        className="modal-content panel"
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "90%",
          maxWidth: "720px",
          maxHeight: "90vh",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          backgroundColor: "var(--bg-panel)",
          boxShadow: "0 4px 24px rgba(0,0,0,0.2)",
        }}
      >
        <div className="panel-title-row" style={{ padding: "16px", borderBottom: "1px solid var(--border)", flexShrink: 0 }}>
          <div>
            <p className="eyebrow">What is RepoDesk?</p>
            <h2>Your local-first AI operations cockpit</h2>
          </div>
          <button className="ghost-button" onClick={onClose}>Close</button>
        </div>

        <div className="modal-body" style={{ padding: "16px", overflowY: "auto", flexGrow: 1 }}>
          <p className="lead" style={{ marginTop: 0 }}>
            RepoDesk runs autonomous coding agents (Claude, Codex, local models) on your
            codebase <strong>safely</strong>: it scopes one task, builds a bounded context that
            never leaks secrets, routes the work to the right model within budget, and lets you
            review exactly what changed before anything is committed.
          </p>

          <h3 style={{ marginBottom: 8 }}>The task flow</h3>
          <ol className="about-flow" style={{ display: "grid", gap: 8, paddingLeft: "1.1rem", margin: 0 }}>
            {PHASES.map((p) => (
              <li key={p.name}>
                <strong>{p.name}</strong> — <span className="muted">{p.blurb}</span>
              </li>
            ))}
          </ol>

          <h3 style={{ marginTop: 20, marginBottom: 8 }}>Where things live</h3>
          <ul className="about-surfaces" style={{ display: "grid", gap: 8, paddingLeft: "1.1rem", margin: 0 }}>
            {SURFACES.map((s) => (
              <li key={s.name}>
                <strong>{s.name}</strong> — <span className="muted">{s.blurb}</span>
              </li>
            ))}
          </ul>
          <p className="muted" style={{ marginTop: 16 }}>
            Everything else (Dashboard, Orchestrate, Playbooks, System Registry, Debug) is depth
            and diagnostics, grouped under the sidebar's Advanced sections.
          </p>
        </div>

        <div className="panel-title-row" style={{ padding: "16px", borderTop: "1px solid var(--border)", flexShrink: 0, justifyContent: "flex-end", gap: 8 }}>
          <button className="ghost-button" onClick={onClose}>Maybe later</button>
          <button
            className="primary-button"
            onClick={() => {
              onGetStarted();
              onClose();
            }}
          >
            Get started
          </button>
        </div>
      </div>
    </div>
  );
}
