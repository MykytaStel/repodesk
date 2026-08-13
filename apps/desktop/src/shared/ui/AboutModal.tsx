import { Dialog } from "./Dialog";

const PHASES: { name: string; blurb: string }[] = [
  { name: "Scope", blurb: "Define one repository, goal, scope and acceptance criteria." },
  { name: "Prepare", blurb: "Build bounded context and inspect the exact execution plan." },
  { name: "Execute", blurb: "Work manually or launch an approved worker in isolation." },
  { name: "Review", blurb: "Accept or reject the attributable ChangeSet." },
  { name: "Verify", blurb: "Run project checks against the reviewed tree." },
  { name: "Finish", blurb: "Commit only current, reviewed and verified changes." },
];

const SURFACES: { name: string; blurb: string }[] = [
  { name: "Work", blurb: "The active Work Item and its next safe action." },
  { name: "Code", blurb: "Repository exploration, editing and diagnostics." },
  { name: "Changes", blurb: "ChangeSets, diffs, review and commit readiness." },
  { name: "Runs", blurb: "Worker execution, checks and durable evidence." },
  { name: "Projects", blurb: "Repository rules, knowledge and workspace setup." },
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
  return (
    <Dialog
      open={isOpen}
      title="Your local-first engineering workspace"
      eyebrow="What is RepoDesk?"
      onClose={onClose}
      footer={(
        <>
          <button type="button" className="ghost-button" onClick={onClose}>Maybe later</button>
          <button
            type="button"
            className="primary-button"
            data-dialog-initial-focus
            onClick={() => {
              onGetStarted();
              onClose();
            }}
          >
            Get started
          </button>
        </>
      )}
    >
      <p className="lead app-dialog-lead">
        RepoDesk turns one bounded Work Item into an attributable, reviewed and verified software change.
        Humans, local tools and coding agents can all do the work; RepoDesk keeps scope, approvals, evidence
        and the final Git state connected.
      </p>

      <h3>The change flow</h3>
      <ol className="about-flow app-dialog-list">
        {PHASES.map((phase) => (
          <li key={phase.name}>
            <strong>{phase.name}</strong> — <span className="muted">{phase.blurb}</span>
          </li>
        ))}
      </ol>

      <h3>Primary workspace</h3>
      <ul className="about-surfaces app-dialog-list">
        {SURFACES.map((surface) => (
          <li key={surface.name}>
            <strong>{surface.name}</strong> — <span className="muted">{surface.blurb}</span>
          </li>
        ))}
      </ul>
      <p className="muted app-dialog-secondary-copy">
        Settings, provider routing, orchestration details and diagnostics stay available as secondary tools;
        they support the engineering workflow rather than define it.
      </p>
    </Dialog>
  );
}
