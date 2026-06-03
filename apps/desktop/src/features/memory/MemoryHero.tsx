export function MemoryHero({
  projectName,
  scanPending,
  consolidatePending,
  onScan,
  onConsolidate,
}: {
  projectName: string;
  scanPending: boolean;
  consolidatePending: boolean;
  onScan: () => void;
  onConsolidate: () => void;
}) {
  return (
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
        <button className="tiny-button" onClick={onScan} disabled={scanPending}>
          {scanPending ? "Scanning..." : "Scan for duplicates / conflicts"}
        </button>
        <button className="tiny-button" onClick={onConsolidate} disabled={consolidatePending}>
          {consolidatePending ? "Writing..." : "Rebuild memory.md"}
        </button>
      </div>
    </section>
  );
}

export function NoProjectMemoryState() {
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
