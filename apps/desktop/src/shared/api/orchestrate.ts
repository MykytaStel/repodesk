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
  executor_kind?: string;
  executor_id?: string;
  provider_id?: string | null;
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
  changed_files?: string[];
  diff_path?: string | null;
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

export type ExecutorAvailability = {
  executor_id: string;
  label: string;
  binary: string;
  available: boolean;
  executable_path?: string | null;
  status: string;
  version?: string | null;
  authenticated?: boolean | null;
  notes: string[];
};

export type LoopStatus = "succeeded" | "needs_approval" | "guardrail_blocked" | "exhausted" | "dry_run";

export type LoopIteration = {
  index: number;
  run_id: string;
  run_status: RunStatus;
  cost_units: number;
  note: string;
};

export type LoopRun = {
  project: string;
  task_id: string;
  goal: string;
  status: LoopStatus;
  iterations: LoopIteration[];
  total_cost_units: number;
  started_at: string;
  finished_at: string;
};

export type Verdict = "good" | "bad" | "neutral";

export type OutcomeRecord = {
  id: number;
  created_at: string;
  project: string;
  task_id: string;
  run_id: string;
  step_id: string;
  task_kind: string;
  provider: string;
  model: string;
  status: string;
  input_tokens: number;
  output_tokens: number;
  cost_units: number;
  verdict: Verdict;
  verdict_source: string;
  confirmed: boolean;
  notes: string;
};

export type ProviderStat = {
  task_kind: string;
  provider: string;
  scored_runs: number;
  good: number;
  bad: number;
  neutral: number;
  success_rate: number | null;
  avg_cost_units: number;
};

const PAID_IDS = [
  "openai_api",
  "anthropic_api",
  "gemini_api",
  "openai",
  "chatgpt",
  "gpt",
  "anthropic",
  "gemini",
];

const CODING_AGENT_IDS = ["codex_cli", "claude_code_cli", "codex", "claude", "claude_code"];

/** Whether a plan includes any paid completion/manual-provider step. */
export function planHasPaidStep(plan: OrchestrationPlan): boolean {
  return plan.steps.some((step) => {
    const ids = [step.provider, step.provider_id ?? "", step.agent, step.executor_id ?? ""].map((id) => id.toLowerCase());
    return ids.some((id) => PAID_IDS.includes(id));
  });
}

/** Whether a plan includes any coding-agent CLI step. */
export function planHasCodingAgentStep(plan: OrchestrationPlan): boolean {
  return plan.steps.some((step) => {
    const ids = [step.provider, step.provider_id ?? "", step.agent, step.executor_id ?? ""].map((id) => id.toLowerCase());
    return ids.some((id) => CODING_AGENT_IDS.includes(id)) || step.executor_kind === "coding_agent";
  });
}

export async function orchestratePlan(
  goal?: string,
  overrideProvider?: string,
  overrideModel?: string
): Promise<OrchestrationPlan> {
  return invoke("orchestrate_plan", {
    goal: goal ?? null,
    overrideProvider: overrideProvider ?? null,
    overrideModel: overrideModel ?? null,
  });
}

export async function orchestrateRun(
  goal: string | undefined,
  dryRun: boolean,
  maxCost?: number | null,
  approveCodingAgents = false,
  overrideProvider?: string,
  overrideModel?: string
): Promise<OrchestrationRun> {
  return invoke("orchestrate_run", {
    goal: goal ?? null,
    dryRun,
    maxCost: maxCost ?? null,
    approveCodingAgents,
    overrideProvider: overrideProvider ?? null,
    overrideModel: overrideModel ?? null,
  });
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

export async function codingAgentExecutors(): Promise<ExecutorAvailability[]> {
  return invoke("coding_agent_executors");
}

export async function orchestrateLoop(args: {
  goal?: string;
  maxIterations?: number;
  maxCost?: number | null;
  dryRun: boolean;
  approvePaid: boolean;
  approveCodingAgents: boolean;
  overrideProvider?: string;
  overrideModel?: string;
}): Promise<LoopRun> {
  return invoke("orchestrate_loop", {
    goal: args.goal ?? null,
    maxIterations: args.maxIterations ?? null,
    maxCost: args.maxCost ?? null,
    dryRun: args.dryRun,
    approvePaid: args.approvePaid,
    approveCodingAgents: args.approveCodingAgents,
    overrideProvider: args.overrideProvider ?? null,
    overrideModel: args.overrideModel ?? null,
  });
}

export async function outcomesList(limit?: number): Promise<OutcomeRecord[]> {
  return invoke("outcomes_list", { limit: limit ?? null });
}

export async function outcomesStats(): Promise<ProviderStat[]> {
  return invoke("outcomes_stats");
}

export async function outcomesConfirm(id: number, verdict: Verdict): Promise<void> {
  return invoke("outcomes_confirm", { id, verdict });
}
