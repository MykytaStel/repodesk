import { useState } from "react";
import type { TabId } from "../../shared/types/api";

type PlaybookShortcut = {
  title: string;
  summary: string;
  target: TabId;
  targetLabel: string;
  destination: string;
  action: string;
  artifact: string;
  startsAgent: boolean;
  result: string;
};

const SHORTCUTS: PlaybookShortcut[] = [
  {
    title: "Generate DB Migrations",
    summary: "Start the guarded Work flow, then run the agent with review and verification.",
    target: "work",
    targetLabel: "Open Work",
    destination: "Work / Execute",
    action: "Sets you in the six-phase task flow; the agent starts only when you approve and press Run agent.",
    artifact: "After the run: Review shows changed files, diff, proof, and memory proposals.",
    startsAgent: false,
    result: "Opened Work for the migration flow.",
  },
  {
    title: "Security Hotspot Review",
    summary: "Inspect changed files, run RepoPilot, and review blocking findings inline.",
    target: "changes",
    targetLabel: "Open Changes",
    destination: "Changes",
    action: "Opens the current git diff and RepoPilot review surface; it does not run a coding agent.",
    artifact: "You inspect file diffs, findings, and staged/unstaged status.",
    startsAgent: false,
    result: "Opened Changes for security review.",
  },
  {
    title: "React Component Scaffold",
    summary: "Delegate the scaffold to the orchestrator, then accept or reject its isolated diff.",
    target: "orchestrate",
    targetLabel: "Open Orchestrate",
    destination: "Orchestrate",
    action: "Opens the planner where you preview steps, choose approvals, then run a real or dry run.",
    artifact: "After a real run: run receipt, isolated worktree, diff, checks proof, and proposals.",
    startsAgent: false,
    result: "Opened Orchestrate for component scaffolding.",
  },
];

export function PlaybooksTab({ setActiveTab }: { setActiveTab: (tab: TabId, detail?: string) => void }) {
  const [lastShortcut, setLastShortcut] = useState<PlaybookShortcut | null>(null);

  function openShortcut(shortcut: PlaybookShortcut) {
    setLastShortcut(shortcut);
    setActiveTab(shortcut.target, `${shortcut.title}: ${shortcut.result}`);
  }

  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Playbooks</p>
        <h1>Workflow shortcuts</h1>
        <p className="lead">
          Each shortcut opens the surface that owns the work. Nothing starts an agent from here.
        </p>
        <div className="button-row">
          <button className="primary-button" onClick={() => openShortcut(SHORTCUTS[0])}>
            Open Work
          </button>
          <button className="ghost-button" onClick={() => openShortcut(SHORTCUTS[2])}>
            Open Orchestrate
          </button>
          <span className="pill neutral" title="Authoring and import are not wired yet.">
            Authoring planned
          </span>
        </div>
      </section>

      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Available</p>
            <h2>Playbook shortcuts</h2>
          </div>
          <span className="pill">{SHORTCUTS.length}</span>
        </div>

        {lastShortcut && (
          <div className="notice ok playbook-result" role="status">
            <strong>{lastShortcut.title}</strong>
            <span>
              {lastShortcut.result} Next visible result: {lastShortcut.artifact}
            </span>
          </div>
        )}

        <div className="playbook-list">
          {SHORTCUTS.map((shortcut) => (
            <div className="check-card playbook-card" key={shortcut.title}>
              <div className="playbook-copy">
                <strong>{shortcut.title}</strong>
                <p className="muted">{shortcut.summary}</p>
                <div className="playbook-route">
                  <div>
                    <span>Destination</span>
                    <strong>{shortcut.destination}</strong>
                  </div>
                  <div>
                    <span>Action</span>
                    <strong>{shortcut.action}</strong>
                  </div>
                  <div>
                    <span>Visible result</span>
                    <strong>{shortcut.artifact}</strong>
                  </div>
                </div>
              </div>
              <div className="playbook-card-actions">
                <span className={`pill ${shortcut.startsAgent ? "warn" : "neutral"}`}>
                  {shortcut.startsAgent ? "Starts agent" : "No hidden run"}
                </span>
                <button className="tiny-button" onClick={() => openShortcut(shortcut)}>
                  {shortcut.targetLabel}
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
