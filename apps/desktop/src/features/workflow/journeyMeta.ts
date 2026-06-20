// Single source of truth for the human-facing copy that makes the 8-step
// workflow legible: what each step does, who performs it (RepoDesk vs you),
// why it's recommended now, what unblocks it, and what comes next.
//
// The step ids mirror the deterministic state machine in the Rust core
// (crates/repodesk-core/src/workflow/engine.rs). This copy is pure
// presentation, so it lives in the frontend — keep the ids in sync.

export type ActorMode = "auto" | "manual";

export interface StepMeta {
  /** 1-based position in the journey. */
  order: number;
  /** Display title (matches the backend WorkflowStep title). */
  title: string;
  /** One sentence: what this step does. */
  oneLiner: string;
  /** "auto" = RepoDesk does it; "manual" = you do it. */
  mode: ActorMode;
  /** Why this step is recommended when it is the current step. */
  whyNow: string;
  /** What the user must do to unblock it when it is blocked. */
  unblockHint: string;
  /** Title of the next step, for the "what's next" line. */
  next: string;
  /** Action id this step triggers, when it maps to a catalog action. */
  actionId?: string;
}

/** Ordered step ids — also used as the static fallback when the backend
 *  hasn't emitted steps yet (first run / onboarding). */
export const STEP_ORDER = [
  "project",
  "task",
  "context",
  "smart_context",
  "safety",
  "prompts",
  "checks",
  "review",
] as const;

export type StepId = (typeof STEP_ORDER)[number];

export const STEP_META: Record<string, StepMeta> = {
  project: {
    order: 1,
    title: "Project",
    oneLiner: "Point RepoDesk at one repository to work in.",
    mode: "manual",
    whyNow: "RepoDesk needs an active project before it can build context or run checks.",
    unblockHint: "Connect a project to begin.",
    next: "Task",
  },
  task: {
    order: 2,
    title: "Task",
    oneLiner: "Scope the work to one concrete task so context stays bounded.",
    mode: "manual",
    whyNow: "Every AI workflow should be scoped to a single task.",
    unblockHint: "Connect a project first, then create a task.",
    next: "Context",
  },
  context: {
    order: 3,
    title: "Context",
    oneLiner: "Bundles your task + git metadata into context.md (never a raw repo dump).",
    mode: "auto",
    whyNow: "Your task is set but its context hasn't been built yet.",
    unblockHint: "Create an active task first.",
    next: "Smart Context",
    actionId: "context-build",
  },
  smart_context: {
    order: 4,
    title: "Smart Context",
    oneLiner: "Trims the context so agents don't read unnecessary files (saves tokens).",
    mode: "auto",
    whyNow: "Base context exists; compress it before any model reads it.",
    unblockHint: "Build the base context first.",
    next: "Safety",
    actionId: "smart-context-build",
  },
  safety: {
    order: 5,
    title: "Safety",
    oneLiner: "Scans the context for secrets and judges it before any AI sees it.",
    mode: "auto",
    whyNow: "Smart context is ready; gate it for secrets before hand-off.",
    unblockHint: "Build smart context first.",
    next: "Prompts",
    actionId: "safety-scan-context",
  },
  prompts: {
    order: 6,
    title: "Prompts",
    oneLiner: "Generates bounded prompts for Codex, ChatGPT, and review.",
    mode: "auto",
    whyNow: "The context passed safety; turn it into agent-ready prompts.",
    unblockHint: "Pass the safety scan first.",
    next: "Checks",
    actionId: "prompt-all",
  },
  checks: {
    order: 7,
    title: "Checks",
    oneLiner: "Runs your configured project checks and keeps only the useful summary.",
    mode: "auto",
    whyNow: "Prompts exist; run checks so the summary can inform review.",
    unblockHint: "Generate prompts first.",
    next: "Review",
    actionId: "checks-run",
  },
  review: {
    order: 8,
    title: "Review",
    oneLiner: "You review prompts, checks, and history before using an external agent.",
    mode: "manual",
    whyNow: "Everything is prepared — give it a final human review before hand-off.",
    unblockHint: "Complete the earlier steps first.",
    next: "Hand off & commit",
    actionId: "judge-codex",
  },
};

/** Map a recommended action id back to the journey step that owns it, so the
 *  primary CTA can name the step and its auto/manual mode before you click. */
export const ACTION_TO_STEP: Record<string, string> = Object.entries(STEP_META).reduce(
  (acc, [stepId, meta]) => {
    if (meta.actionId) acc[meta.actionId] = stepId;
    return acc;
  },
  {} as Record<string, string>,
);

/** Static 8-step list (used when the backend hasn't emitted steps yet). */
export function staticJourney(): Array<{ id: string; title: string; status: string; blocker?: string }> {
  return STEP_ORDER.map((id) => ({ id, title: STEP_META[id].title, status: "blocked" }));
}

export function stepMeta(id: string): StepMeta | undefined {
  return STEP_META[id];
}
