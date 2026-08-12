import { invoke } from "@tauri-apps/api/core";

export type AiStrategyMode = "auto" | "lean" | "balanced" | "local_first" | "quality";
export type AiStrategyProfile = "lean" | "balanced" | "local_first" | "quality";
export type AiPlanShape = "single_writer" | "writer_with_review" | "analyze_writer_review";
export type AiStrategyReasonCode =
  | "explicit_mode"
  | "cold_start"
  | "narrow_scope"
  | "wide_or_unknown_scope"
  | "repeated_context"
  | "agent_fanout"
  | "prompt_heavy"
  | "execution_churn"
  | "change_rejection"
  | "verification_instability";

export type AiStrategyReason = {
  code: AiStrategyReasonCode;
  detail: string;
};

export type AiStrategyRecommendation = {
  requested_mode: AiStrategyMode;
  profile: AiStrategyProfile;
  plan_shape: AiPlanShape;
  economy_mode: string;
  reuse_prepared_context: boolean;
  max_agent_steps: number;
  independent_ai_review: boolean;
  reasons: AiStrategyReason[];
};

export type StrategyExecutionStep = {
  step_id: string;
  title: string;
  executor_label: string;
  executor_kind: string;
  model: string;
  allow_write: boolean;
  isolated_workspace: boolean;
  paid: boolean;
  estimated_input_tokens: number;
  estimated_output_tokens: number;
  estimated_cost_units: number;
};

export type StrategyExecutionContext = {
  prepared: boolean;
  context_tokens: number;
  candidate_tokens: number;
  token_budget: number | null;
  included_sources: number;
  excluded_sources: number;
  context_fingerprint: string | null;
  generated_at: string | null;
  warning: string | null;
};

export type StrategyExecutionContract = {
  goal: string;
  steps: StrategyExecutionStep[];
  context: StrategyExecutionContext;
  total_estimated_tokens: number;
  total_estimated_cost_units: number;
  currency_label: string;
  expected_writes: boolean;
  isolated_workspace: boolean;
  requires_coding_agent_approval: boolean;
  requires_paid_approval: boolean;
};

export type StrategyBaselineComparison = {
  baseline_steps: number;
  planned_steps: number;
  baseline_estimated_tokens: number;
  planned_estimated_tokens: number;
  estimated_saved_tokens: number;
  estimated_cost_delta_units: number;
};

export type StrategyExecutionPreview = {
  execution: StrategyExecutionContract;
  strategy: AiStrategyRecommendation;
  comparison: StrategyBaselineComparison;
  plan_fingerprint: string;
};

export async function workStrategyExecutionPreview(
  strategyMode: AiStrategyMode,
  goal?: string | null,
  overrideProvider?: string | null,
  overrideModel?: string | null,
): Promise<StrategyExecutionPreview> {
  return invoke("work_strategy_execution_preview", {
    goal: goal ?? null,
    overrideProvider: overrideProvider ?? null,
    overrideModel: overrideModel ?? null,
    strategyMode,
  });
}

export async function orchestrateStrategyRun(input: {
  strategyMode: AiStrategyMode;
  expectedPlanFingerprint: string | null;
  approvePaid: boolean;
  approveCodingAgents: boolean;
  goal?: string | null;
  dryRun?: boolean;
  maxCost?: number | null;
  overrideProvider?: string | null;
  overrideModel?: string | null;
}) {
  return invoke<import("./orchestrate").OrchestrationRun>("orchestrate_strategy_run", {
    goal: input.goal ?? null,
    dryRun: input.dryRun ?? false,
    maxCost: input.maxCost ?? null,
    approvePaid: input.approvePaid,
    approveCodingAgents: input.approveCodingAgents,
    overrideProvider: input.overrideProvider ?? null,
    overrideModel: input.overrideModel ?? null,
    strategyMode: input.strategyMode,
    expectedPlanFingerprint: input.expectedPlanFingerprint,
  });
}
