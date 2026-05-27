import React from "react";

export type EconomyMode = "economy" | "balanced" | "quality";

interface EconomyControlProps {
  mode: EconomyMode;
  setMode: (mode: EconomyMode) => void;
  isBusy?: boolean;
}

export function EconomyControl({ mode, setMode, isBusy }: EconomyControlProps) {
  const modes = [
    {
      id: "economy",
      label: "Economy (Local-First)",
      desc: "Uses only local models to save costs. Pauses on complex tasks.",
      icon: "🌱",
    },
    {
      id: "balanced",
      label: "Balanced",
      desc: "Local for drafts & checks, paid models for hard tasks.",
      icon: "⚖️",
    },
    {
      id: "quality",
      label: "Maximum Quality",
      desc: "Always use best paid models for highest reasoning.",
      icon: "✨",
    },
  ];

  return (
    <section className="panel wide-panel" style={{ marginTop: 16 }}>
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
            className={`check-card ${mode === m.id ? "ok" : "neutral"}`}
            onClick={() => setMode(m.id as EconomyMode)}
            style={{
              borderColor: mode === m.id ? "var(--accent)" : undefined,
              boxShadow: mode === m.id ? "0 0 16px rgba(94, 224, 174, 0.2)" : undefined,
            }}
          >
            <span style={{ fontSize: 24, marginBottom: 8 }}>{m.icon}</span>
            <strong style={{ fontSize: 15, marginTop: 4 }}>{m.label}</strong>
            <span style={{ marginTop: 6, textTransform: "none", letterSpacing: "normal" }}>{m.desc}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
