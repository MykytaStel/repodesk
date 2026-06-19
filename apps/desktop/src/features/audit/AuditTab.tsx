import React from "react";
import { MetricCard } from "../../shared/ui/SharedComponents";

export function AuditTab() {
  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Enterprise Security</p>
        <h1>Immutable Audit Trail</h1>
        <p className="lead">Every action taken by an AI agent is cryptographically hashed and chained to ensure tamper-evident compliance logs.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => alert("Exporting CSV...")}>Export CSV (SOC2)</button>
          <button className="ghost-button" onClick={() => alert("Exporting JSON...")}>Export JSON</button>
        </div>
      </section>

      <div className="card-row">
        <MetricCard label="Chain Integrity" value="Verified" detail="SHA-256 Chaining Valid" tone="ok" />
        <MetricCard label="Total Events" value="1,402" detail="Across 3 projects" />
        <MetricCard label="Last Action" value="2 mins ago" detail="By Code Llama (Local)" />
      </div>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <h2>Recent Events</h2>
        </div>
        <div className="table-responsive">
          <table className="w-full text-left" style={{ width: "100%", fontSize: "0.9rem" }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border-color)", paddingBottom: "8px" }}>
                <th style={{ padding: "8px" }}>Timestamp</th>
                <th style={{ padding: "8px" }}>Action Type</th>
                <th style={{ padding: "8px" }}>Details</th>
                <th style={{ padding: "8px" }}>Hash</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td style={{ padding: "8px" }}>2026-06-20 01:23:45</td>
                <td style={{ padding: "8px" }}><span className="pill neutral">Command Executed</span></td>
                <td style={{ padding: "8px" }}>cargo test --workspace</td>
                <td style={{ padding: "8px", fontFamily: "monospace", color: "var(--text-muted)" }}>a3f9b2c...</td>
              </tr>
              <tr>
                <td style={{ padding: "8px" }}>2026-06-20 01:20:12</td>
                <td style={{ padding: "8px" }}><span className="pill ok">File Modified</span></td>
                <td style={{ padding: "8px" }}>src/security.rs</td>
                <td style={{ padding: "8px", fontFamily: "monospace", color: "var(--text-muted)" }}>c8d1e4a...</td>
              </tr>
              <tr>
                <td style={{ padding: "8px" }}>2026-06-20 01:15:00</td>
                <td style={{ padding: "8px" }}><span className="pill warn">Network Request</span></td>
                <td style={{ padding: "8px" }}>openai:gpt-4o</td>
                <td style={{ padding: "8px", fontFamily: "monospace", color: "var(--text-muted)" }}>f9e3d2c...</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
