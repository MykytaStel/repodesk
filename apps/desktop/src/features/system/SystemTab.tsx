import React from "react";
import { statusTone, formatNumber } from "../../shared/ui/SharedComponents";

interface SystemTabProps {
  systemCapabilities: any;
  systemPeripherals: any;
  systemAgents: any;
  systemModules: any[];
  isBusy: boolean;
  refreshAll: (label: string) => void;
}

export function SystemTab({
  systemCapabilities,
  systemPeripherals,
  systemAgents,
  systemModules,
  isBusy,
  refreshAll,
}: SystemTabProps) {
  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Registry</p>
        <h1>Cognitive capabilities, MCP tools, and orchestrations.</h1>
        <p className="lead">Inspect all registered modules, peripheral access permissions, and model agents active on the RepoDesk platform.</p>
        <div className="button-row">
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing registries")} disabled={isBusy}>Refresh system state</button>
        </div>
      </section>

      {/* Capabilities Panel */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Skills & Capabilities</p>
            <h2>Active Capabilities</h2>
          </div>
          <span className="pill ok">{systemCapabilities?.capabilities.filter((c: any) => c.enabled).length ?? 0} enabled</span>
        </div>
        <div className="table-list">
          {(systemCapabilities?.capabilities ?? []).map((cap: any) => (
            <div className="table-row flex-col items-start gap-md p-md" key={cap.name}>
              <div className="flex justify-between w-full items-center">
                <div className="flex items-center gap-md">
                  <strong className="text-lg">{cap.name}</strong>
                  <span className="text-sm text-muted">kind: {cap.kind}</span>
                </div>
                <div className="flex gap-sm">
                  <span className={`pill ${cap.enabled ? "ok" : "disabled"}`}>{cap.enabled ? "Enabled" : "Disabled"}</span>
                  <span className={`pill ${cap.local ? "ok" : "warn"}`}>{cap.local ? "Local Only" : "Remote API"}</span>
                  <span className={`pill ${statusTone(cap.risk)}`}>{cap.risk} risk</span>
                </div>
              </div>
              <div className="text-base" style={{ color: "var(--text)", lineHeight: "1.4" }}>
                <strong>Boundary:</strong> {cap.boundary}
              </div>
              {cap.allowed_actions.length > 0 && (
                <div className="text-sm text-muted">
                  <strong>Allowed:</strong> {cap.allowed_actions.join(", ")}
                </div>
              )}
            </div>
          ))}
        </div>
      </section>

      {/* MCP & Peripherals Panel */}
      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">MCP & Peripherals</p>
            <h2>Peripheral Tools</h2>
          </div>
        </div>
        <div className="table-list">
          {(systemPeripherals?.peripherals ?? []).map((p: any) => (
            <div className="table-row flex-col items-start gap-sm p-sm" key={p.name}>
              <div className="flex justify-between w-full">
                <strong><code>{p.name}</code></strong>
                <span className={`pill ${statusTone(p.risk)}`}>{p.risk} risk</span>
              </div>
              <span className="text-sm text-muted">Access: <strong>{p.access}</strong> &bull; Kind: {p.kind}</span>
              {p.allowed_actions.length > 0 && (
                <div className="text-xs text-muted mt-xs">
                  <strong>Allowed:</strong> {p.allowed_actions.join(", ")}
                </div>
              )}
            </div>
          ))}
        </div>
      </section>

      {/* Orchestrator Agents Panel */}
      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Orchestration</p>
            <h2>Configured Agents</h2>
          </div>
        </div>
        <div className="table-list">
          {(systemAgents?.agents ?? []).map((agent: any) => (
            <div className="table-row flex-col items-start gap-sm p-sm" key={agent.name}>
              <div className="flex justify-between w-full">
                <strong>{agent.name}</strong>
                <span className="pill ok text-xs">{formatNumber(agent.default_budget_tokens)} token budget</span>
              </div>
              <p className="text-sm text-muted m-0">{agent.role}</p>
              {agent.preferred_for.length > 0 && (
                <div className="text-xs text-muted mt-xs">
                  <strong>Preferred for:</strong> {agent.preferred_for.join(", ")}
                </div>
              )}
            </div>
          ))}
        </div>
      </section>

      {/* Cognitive Modules Panel */}
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Brain Modules & Hooks</p>
            <h2>Core Cognitive Flow</h2>
          </div>
          <span className="pill ok">{systemModules.length} active layers</span>
        </div>
        <div style={{ display: "grid", gap: "12px", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
          {systemModules.map((module: any) => (
            <div className="card flex-col gap-sm p-md" key={module.name}>
              <div className="flex justify-between items-center">
                <strong className="text-lg">{module.name}</strong>
                <span className={`pill text-xs ${module.status === "active" ? "ok" : "neutral"}`}>{module.status}</span>
              </div>
              <span className="text-xs text-muted">Layer: <strong>{module.layer}</strong></span>
              <p className="text-sm text-muted m-0" style={{ lineHeight: "1.4" }}>{module.purpose}</p>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
