import React from "react";
import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { statusTone } from "../../shared/ui/SharedComponents";
import { queryKeys } from "../../shared/api/queries";
import { useSystem } from "./useSystem";
import { aiDiscoveryScan } from "../../shared/api/discovery";
import "../routing/routing-feature.css";

export function SystemTab() {
  const queryClient = useQueryClient();
  const { systemAgents, systemCapabilities, systemPeripherals, systemModules, isLoading } = useSystem();
  const [refreshing, setRefreshing] = useState(false);
  const [lastRefresh, setLastRefresh] = useState<string | null>(null);
  const isBusy = isLoading || refreshing;
  // Machine-level AI discovery scan (on demand).
  const discovery = useMutation({ mutationFn: aiDiscoveryScan });
  const report = discovery.data;
  const refreshAll = async () => {
    setRefreshing(true);
    try {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.system.agents }),
        queryClient.invalidateQueries({ queryKey: queryKeys.system.capabilities }),
        queryClient.invalidateQueries({ queryKey: queryKeys.system.peripherals }),
        queryClient.invalidateQueries({ queryKey: queryKeys.system.modules }),
      ]);
      setLastRefresh(new Date().toLocaleTimeString());
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">System registry</p>
        <h1>Agent skills & context boundaries</h1>
        <p className="lead">RepoDesk restricts what local or paid models can do using explicit system tools. MCP tools are managed here.</p>
        <div className="button-row">
          <button className="ghost-button" onClick={() => void refreshAll()} disabled={isBusy}>
            {refreshing ? "Refreshing..." : "Refresh system"}
          </button>
        </div>
        <p className="muted refresh-note">
          Refresh re-reads agent definitions, capability boundaries, peripherals, and brain modules.
          {lastRefresh ? ` Last refreshed ${lastRefresh}.` : ""}
        </p>
      </section>
      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Available</p><h2>Agents</h2></div></div>
        <div className="table-list">
          {(systemAgents?.agents ?? []).map((agent: any) => (
            <div className="table-row flex-col items-start gap-sm" key={agent.name} style={{ paddingBottom: 16 }}>
              <div className="w-full flex justify-between items-center">
                <strong>{agent.name} <span className="muted" style={{ fontWeight: "normal", fontSize: "0.9em" }}>— {agent.role}</span></strong>
                <span className="pill neutral">{agent.kind}</span>
              </div>
              <div className="route-summary-grid" style={{ width: "100%", marginTop: 8 }}>
                <div><span>Budget</span><strong>{agent.default_budget_tokens.toLocaleString()} tokens</strong></div>
                <div><span>Preferred for</span><strong>{agent.preferred_for.join(", ")}</strong></div>
              </div>
              <div className="route-detail-grid" style={{ width: "100%", marginTop: 8 }}>
                {agent.allowed_actions?.length > 0 && (
                  <div className={`route-list ok`}>
                    <strong>Allowed actions</strong>
                    {agent.allowed_actions.map((act: string) => <span key={act}>{act}</span>)}
                  </div>
                )}
                {agent.forbidden_actions?.length > 0 && (
                  <div className={`route-list danger`}>
                    <strong>Forbidden actions</strong>
                    {agent.forbidden_actions.map((act: string) => <span key={act}>{act}</span>)}
                  </div>
                )}
              </div>
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

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Detected on this machine</p>
            <h2>Installed AI tools</h2>
          </div>
          <button className="ghost-button" onClick={() => discovery.mutate()} disabled={discovery.isPending}>
            {discovery.isPending ? "Scanning…" : report ? "Re-scan" : "Scan system"}
          </button>
        </div>
        <p className="muted">
          Passive scan of installed CLIs, desktop apps, and localhost endpoints — what AI tooling
          this machine has. (To import AI files from the connected repo, use Import from project.)
        </p>
        {discovery.error && <div className="notice danger">{(discovery.error as Error).message}</div>}
        {report && (
          <>
            <div className="table-list">
              {report.tools.map((tool) => (
                <div className="table-row" key={tool.id}>
                  <div>
                    <strong>{tool.name}</strong>
                    <span>{tool.detection}{tool.executable_path ? ` · ${tool.executable_path}` : ""}</span>
                  </div>
                  <div className="row-meta">
                    <span className={`pill ${tool.status === "available" ? "ok" : tool.status === "maybe" ? "warn" : "neutral"}`}>
                      {tool.status}
                    </span>
                    <span className="pill neutral">{tool.category}</span>
                  </div>
                </div>
              ))}
              {report.endpoints.map((ep) => (
                <div className="table-row" key={ep.id}>
                  <div><strong>{ep.name}</strong><span>{ep.url}</span></div>
                  <div className="row-meta">
                    <span className={`pill ${ep.status === "available" ? "ok" : "neutral"}`}>{ep.status}</span>
                    <span className="pill neutral">endpoint</span>
                  </div>
                </div>
              ))}
            </div>
            {report.recommendations.length > 0 && (
              <div className="route-list ok" style={{ marginTop: 12 }}>
                <strong>Recommendations</strong>
                {report.recommendations.map((r, i) => <span key={i}>{r}</span>)}
              </div>
            )}
            {report.warnings.length > 0 && (
              <div className="route-list danger" style={{ marginTop: 12 }}>
                <strong>Warnings</strong>
                {report.warnings.map((w, i) => <span key={i}>{w}</span>)}
              </div>
            )}
          </>
        )}
      </section>
    </div>
  );
}
