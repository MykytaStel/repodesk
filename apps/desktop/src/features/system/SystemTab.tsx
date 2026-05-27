import React from "react";
import { statusTone } from "../../shared/ui/SharedComponents";
import { useSystem } from "./useSystem";

export function SystemTab() {
  const { systemAgents, systemCapabilities, systemPeripherals, systemModules, isLoading: isBusy } = useSystem();
  const refreshAll = () => {};

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">System registry</p>
        <h1>Agent skills & context boundaries</h1>
        <p className="lead">RepoDesk restricts what local or paid models can do using explicit system tools. MCP tools are managed here.</p>
        <div className="button-row">
          <button className="ghost-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh system</button>
        </div>
      </section>
      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Available</p><h2>Agents</h2></div></div>
        <div className="table-list">
          {(systemAgents?.agents ?? []).map((agent: any) => (
            <div className="table-row multi-line" key={agent.name}>
              <div><strong>{agent.name}</strong><span>{agent.role}</span></div>
              <div className="row-meta"><span className="pill neutral">{agent.kind}</span></div>
            </div>
          ))}
        </div>
      </section>
      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Loaded</p><h2>Capabilities</h2></div></div>
        <div className="table-list">
          {(systemCapabilities?.capabilities ?? []).map((cap: any) => (
            <div className="table-row" key={cap.name}>
              <div><strong>{cap.name}</strong><span>{cap.boundary}</span></div>
              <div className="row-meta"><span className={`pill ${statusTone(cap.enabled)}`}>{cap.enabled ? "enabled" : "disabled"}</span><span className="pill neutral">{cap.kind}</span></div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
