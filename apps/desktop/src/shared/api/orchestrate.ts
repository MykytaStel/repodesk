import { invoke } from "@tauri-apps/api/core";

export type ThinkingLevel = "none" | "low" | "medium" | "high";
export type SubAgentStatus = "ok" | "skipped" | "blocked" | "failed";
export type RunStatus = "completed" | "partial" | "failed" | "dry_run";

export type SubAgentTask = {
  id: string;
  title: string;
  kind: string;
  agent: string;
  provider: string;
  model?: string | null;
  thinking: ThinkingLevel;
  instruction: string;
  depends_on: string[];
  budget_tokens: number;
  allow_write: boolean;
};

export type SubAgentResult = {
  task_id: string;
  agent: string;
  provider: string;
  model: string;
  status: SubAgentStatus;
  output: string;
  input_tokens: number;
  output_tokens: number;
  cost_units: number;
  captured_proposals: number;
  notes: string[];
};

export type OrchestrationPlan = {
  project: string;
  task_id: string;
  goal: string;
  steps: SubAgentTask[];
};

export type OrchestrationRun = {
  run_id: string;
  project: string;
  task_id: string;
  goal: string;
  status: RunStatus;
  dry_run: boolean;
  started_at: string;
  finished_at: string;
  results: SubAgentResult[];
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_units: number;
};

export type RunSummary = {
  run_id: string;
  goal: string;
  status: RunStatus;
  dry_run: boolean;
  started_at: string;
  finished_at: string;
  step_count: number;
  total_cost_units: number;
};

export type TaskEvent = {
  timestamp: string;
  project: string;
  task_id: string;
  module_name: string;
  level: string;
  message: string;
  metadata: Record<string, string>;
};

const PAID_PROVIDERS = ["chatgpt", "codex", "openai", "gpt", "gemini", "anthropic", "claude"];

/** Whether a plan includes any paid-provider step (used to warn before running). */
export function planHasPaidStep(plan: OrchestrationPlan): boolean {
  return plan.steps.some((step) => PAID_PROVIDERS.includes(step.provider.toLowerCase()));
}

export async function orchestratePlan(goal?: string): Promise<OrchestrationPlan> {
  return invoke("orchestrate_plan", { goal: goal ?? null });
}

export async function orchestrateRun(
  goal: string | undefined,
  dryRun: boolean,
  maxCost?: number | null,
): Promise<OrchestrationRun> {
  return invoke("orchestrate_run", { goal: goal ?? null, dryRun, maxCost: maxCost ?? null });
}

export async function orchestrateStatus(): Promise<OrchestrationRun | null> {
  return invoke("orchestrate_status");
}

export async function orchestrateShow(runId: string): Promise<OrchestrationRun | null> {
  return invoke("orchestrate_show", { runId });
}

export async function orchestrationRuns(): Promise<RunSummary[]> {
  return invoke("orchestration_runs");
}

export async function taskTimeline(): Promise<TaskEvent[]> {
  return invoke("task_timeline");
}
