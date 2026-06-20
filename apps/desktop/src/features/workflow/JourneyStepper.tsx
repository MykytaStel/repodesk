import React from "react";
import { asRecord, getString, statusTone, ActorBadge } from "../../shared/ui/SharedComponents";
import { STEP_META, staticJourney, stepMeta } from "./journeyMeta";

interface RawStep {
  id: string;
  title: string;
  status: string;
  blocker?: string;
}

function normalizeSteps(steps: unknown[]): RawStep[] {
  if (!steps || steps.length === 0) return staticJourney();
  return steps.map((step, index) => {
    const record = asRecord(step);
    return {
      id: getString(record, "id", String(index)),
      title: getString(record, "title", `Step ${index + 1}`),
      status: getString(record, "status", "blocked"),
      blocker: typeof record.blocker === "string" ? record.blocker : undefined,
    };
  });
}

/** The always-visible spine of the daily loop: shows all 8 steps, which are
 *  done / current / blocked, who performs each, and lets the user inspect any
 *  one. `preview` renders it greyed for first-run onboarding. */
export function JourneyStepper({
  steps,
  currentStepId,
  selectedStepId,
  onSelectStep,
  preview = false,
}: {
  steps: unknown[];
  currentStepId?: string;
  selectedStepId?: string;
  onSelectStep?: (id: string) => void;
  preview?: boolean;
}) {
  const rows = normalizeSteps(steps);
  const activeId = currentStepId ?? rows.find((s) => s.status === "current")?.id;
  const detailId = selectedStepId ?? activeId ?? rows[0]?.id;
  const detailRow = rows.find((s) => s.id === detailId);

  return (
    <section className={`panel journey wide-panel ${preview ? "journey-preview" : ""}`}>
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Your workflow</p>
          <h2>One task, eight steps</h2>
        </div>
        {preview && <span className="pill neutral">Preview</span>}
      </div>

      <ol className="journey-track">
        {rows.map((step) => {
          const meta = stepMeta(step.id);
          const isCurrent = step.id === activeId && !preview;
          const isSelected = step.id === detailId && !preview;
          const tone = preview ? "neutral" : statusTone(step.status);
          return (
            <li key={step.id}>
              <button
                type="button"
                className={`journey-node ${tone} ${isCurrent ? "current" : ""} ${isSelected ? "selected" : ""}`}
                disabled={preview || !onSelectStep}
                onClick={() => onSelectStep?.(step.id)}
                title={meta?.oneLiner ?? step.title}
              >
                <span className="journey-index">{meta?.order ?? "?"}</span>
                <span className="journey-node-body">
                  <strong>{step.title}</strong>
                  {meta && <ActorBadge mode={meta.mode} className="journey-actor" />}
                </span>
                {!preview && <small className="journey-status">{step.status}</small>}
              </button>
            </li>
          );
        })}
      </ol>

      {!preview && detailRow && <StepDetailCard step={detailRow} />}
    </section>
  );
}

/** Explains the focused step: what it does, who does it, why now / what
 *  unblocks it, and what comes next. */
function StepDetailCard({ step }: { step: RawStep }) {
  const meta = stepMeta(step.id);
  if (!meta) return null;
  const isCurrent = step.status === "current";
  const isBlocked = step.status === "blocked";
  const isDone = step.status === "done";

  return (
    <div className="journey-detail">
      <div className="journey-detail-head">
        <h3>
          {meta.order}. {meta.title}
        </h3>
        <ActorBadge mode={meta.mode} />
      </div>
      <p className="lead">{meta.oneLiner}</p>
      {isDone && <p className="muted">✓ Done — RepoDesk has this covered.</p>}
      {isCurrent && <p className="journey-why">→ {meta.whyNow}</p>}
      {isBlocked && (
        <p className="journey-why muted">⛔ {step.blocker ?? meta.unblockHint}</p>
      )}
      <p className="muted journey-next">Next: {meta.next}</p>
    </div>
  );
}
