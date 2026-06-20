import React from "react";
import { asRecord, getString, ActorBadge } from "../../shared/ui/SharedComponents";
import { STEP_META, staticJourney, stepMeta } from "./journeyMeta";

interface RawStep {
  id: string;
  title: string;
  status: string;
  blocker?: string;
}

export interface StepRunResult {
  stepId: string;
  ok: boolean;
  message: string;
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
  onRunStep,
  isRunning = false,
  runResult,
  preview = false,
}: {
  steps: unknown[];
  currentStepId?: string;
  selectedStepId?: string;
  onSelectStep?: (id: string) => void;
  onRunStep?: (actionId: string, stepId: string) => void;
  isRunning?: boolean;
  runResult?: StepRunResult | null;
  preview?: boolean;
}) {
  const rows = normalizeSteps(steps);
  // Only the *first* current step is the "now" step — the engine can mark a
  // later step (e.g. review) current too, but showing two "NOW"s is confusing.
  const activeIndex = rows.findIndex((s) => s.status === "current");
  const activeId = currentStepId ?? (activeIndex >= 0 ? rows[activeIndex].id : undefined);
  const detailId = selectedStepId ?? activeId ?? rows[0]?.id;
  const detailRow = rows.find((s) => s.id === detailId);

  function stateTag(step: RawStep, index: number): { label: string; tone: string } {
    if (preview) return { label: `Step ${index + 1}`, tone: "neutral" };
    if (step.status === "done") return { label: "Done", tone: "ok" };
    if (step.status === "current") return index === activeIndex ? { label: "Now", tone: "accent" } : { label: "Soon", tone: "neutral" };
    return { label: "Locked", tone: "muted" };
  }

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
        {rows.map((step, index) => {
          const meta = stepMeta(step.id);
          const isCurrent = step.id === activeId && !preview;
          const isSelected = step.id === detailId && !preview;
          const tone = stateTag(step, index).tone;
          return (
            <li key={step.id}>
              <button
                type="button"
                className={`journey-node ${tone} ${isCurrent ? "current" : ""} ${isSelected ? "selected" : ""}`}
                disabled={preview || !onSelectStep}
                onClick={() => onSelectStep?.(step.id)}
                title={meta?.oneLiner ?? step.title}
              >
                <span className="journey-node-head">
                  <span className="journey-index">{meta?.order ?? index + 1}</span>
                  <span className={`journey-state-tag ${tone}`}>{stateTag(step, index).label}</span>
                </span>
                <strong className="journey-title">{step.title}</strong>
                {meta && <span className={`journey-tag ${meta.mode}`} title={meta.mode === "auto" ? "RepoDesk does this for you" : "This step needs your input"}>{meta.mode === "auto" ? "Auto" : "You"}</span>}
              </button>
            </li>
          );
        })}
      </ol>

      {!preview && detailRow && (
        <StepDetailCard
          step={detailRow}
          onRunStep={onRunStep}
          isRunning={isRunning}
          runResult={runResult?.stepId === detailRow.id ? runResult : null}
        />
      )}
    </section>
  );
}

/** Explains the focused step: what it does, who does it, why now / what
 *  unblocks it, and what comes next — and lets you run it if it's an automatic
 *  step that's ready. */
function StepDetailCard({
  step,
  onRunStep,
  isRunning,
  runResult,
}: {
  step: RawStep;
  onRunStep?: (actionId: string, stepId: string) => void;
  isRunning?: boolean;
  runResult?: StepRunResult | null;
}) {
  const meta = stepMeta(step.id);
  if (!meta) return null;
  const isCurrent = step.status === "current";
  const isBlocked = step.status === "blocked";
  const isDone = step.status === "done";

  // Runnable = an automatic step with a backing action whose prerequisites are met.
  const runnable = Boolean(onRunStep && meta.actionId && meta.mode === "auto" && !isBlocked);

  return (
    <div className="journey-detail">
      <div className="journey-detail-head">
        <h3>
          {meta.order}. {meta.title}
        </h3>
        <ActorBadge mode={meta.mode} />
      </div>
      <p className="lead">{meta.oneLiner}</p>
      {isDone && !runResult && <p className="muted">✓ Done — RepoDesk has this covered.</p>}
      {isCurrent && <p className="journey-why">→ {meta.whyNow}</p>}
      {isBlocked && (
        <p className="journey-why muted">⛔ {step.blocker ?? meta.unblockHint}</p>
      )}
      {meta.mode === "manual" && !isBlocked && !isDone && (
        <p className="journey-why muted">This one's on you — RepoDesk can't do it automatically.</p>
      )}
      <p className="muted journey-next">Next: {meta.next}</p>

      {runnable && (
        <div className="button-row" style={{ marginTop: 12 }}>
          <button
            className={isCurrent ? "primary-button" : "ghost-button"}
            disabled={isRunning}
            onClick={() => onRunStep!(meta.actionId!, step.id)}
          >
            {isRunning ? "Running…" : isDone ? `Re-run ${meta.title}` : `Run ${meta.title}`}
          </button>
        </div>
      )}

      {runResult && (
        <div className={`notice ${runResult.ok ? "ok" : "danger"}`} style={{ marginTop: 12 }}>
          {runResult.ok ? "✓ " : "✗ "}
          {runResult.message}
        </div>
      )}
    </div>
  );
}
