export type EconomyMode = "economy" | "balanced" | "quality";

interface EconomyControlProps {
  mode: EconomyMode;
  setMode: (mode: EconomyMode) => void;
  isBusy?: boolean;
}

export function EconomyControl({ mode, setMode, isBusy }: EconomyControlProps) {
  const modes: Array<{ id: EconomyMode; label: string; desc: string }> = [
    {
      id: "economy",
      label: "Economy (Local-First)",
      desc: "Uses only local models to save costs. Pauses on complex tasks.",
    },
    {
      id: "balanced",
      label: "Balanced",
      desc: "Local for drafts & checks, paid models for hard tasks.",
    },
    {
      id: "quality",
      label: "Maximum Quality",
      desc: "Always use best paid models for highest reasoning.",
    },
  ];

  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Routing Strategy</p>
          <h2>Economy Mode</h2>
        </div>
      </div>
      <div className="checklist-grid" style={{ gridTemplateColumns: "repeat(3, minmax(0, 1fr))" }}>
        {modes.map((m) => (
          <button
            key={m.id}
            disabled={isBusy}
            className={`check-card ${mode === m.id ? "selected ok" : "neutral"}`}
            onClick={() => setMode(m.id)}
          >
            <span className="economy-icon" aria-hidden="true">
              <EconomyModeIcon mode={m.id} />
            </span>
            <strong>{m.label}</strong>
            <span className="check-card-detail">{m.desc}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function EconomyModeIcon({ mode }: { mode: EconomyMode }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      {mode === "economy" ? (
        <>
          <path d="M5 19c7 0 12-5 14-14-9 2-14 7-14 14Z" />
          <path d="M5 19c2.5-4 5.7-6.8 10-9" />
        </>
      ) : mode === "balanced" ? (
        <>
          <path d="M12 4v16" />
          <path d="M6 8h12" />
          <path d="m7 8-3 6h6z" />
          <path d="m17 8-3 6h6z" />
        </>
      ) : (
        <>
          <path d="M12 3v4" />
          <path d="M12 17v4" />
          <path d="M3 12h4" />
          <path d="M17 12h4" />
          <path d="m6.5 6.5 2.8 2.8" />
          <path d="m14.7 14.7 2.8 2.8" />
          <path d="m17.5 6.5-2.8 2.8" />
          <path d="m9.3 14.7-2.8 2.8" />
          <circle cx="12" cy="12" r="2.5" />
        </>
      )}
    </svg>
  );
}
