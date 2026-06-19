import React from "react";
import { MetricCard } from "../../shared/ui/SharedComponents";

export function PlaybooksTab() {
  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Knowledge Sharing</p>
        <h1>Team Playbooks</h1>
        <p className="lead">Run company-approved standard workflows and recipes for common tasks. Ensure everyone follows the same safe orchestration patterns.</p>
        <div className="button-row">
          <button className="primary-button">New Playbook</button>
          <button className="ghost-button">Import</button>
        </div>
      </section>

      <section className="panel">
        <div className="panel-title-row">
          <h2>Available Playbooks</h2>
        </div>
        <div className="checklist-grid" style={{ gridTemplateColumns: "1fr", marginTop: "1rem" }}>
          
          <div className="check-card ok" style={{ display: "flex", justifyContent: "space-between", padding: "16px" }}>
            <div>
              <strong>Generate DB Migrations</strong>
              <p className="muted" style={{ fontSize: "13px", marginTop: "4px" }}>Analyzes models, creates migration files, and runs tests.</p>
            </div>
            <button className="tiny-button">Run</button>
          </div>

          <div className="check-card neutral" style={{ display: "flex", justifyContent: "space-between", padding: "16px" }}>
            <div>
              <strong>Security Hotspot Review</strong>
              <p className="muted" style={{ fontSize: "13px", marginTop: "4px" }}>Runs Snyk and Repopilot, fixing high-severity issues automatically.</p>
            </div>
            <button className="tiny-button">Run</button>
          </div>

          <div className="check-card neutral" style={{ display: "flex", justifyContent: "space-between", padding: "16px" }}>
            <div>
              <strong>React Component Scaffold</strong>
              <p className="muted" style={{ fontSize: "13px", marginTop: "4px" }}>Generates a standard React component with tests and stories.</p>
            </div>
            <button className="tiny-button">Run</button>
          </div>

        </div>
      </section>
    </div>
  );
}
