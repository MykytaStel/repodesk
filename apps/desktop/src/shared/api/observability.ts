import { invoke } from "@tauri-apps/api/core";
import type { EngineeringIntelligence, RunEvidenceSnapshot } from "./engineering";
import type { AiStrategyProfile } from "./strategy";

export const WORK_OBSERVABILITY_KEY = ["work", "observability-v1"] as const;

export type AiUsageSignalSeverity = "info" | "warning";
export type AiUsageSignalCode =
  | "repeated_context"
  | "agent_fanout"
  | "execution_churn"
  | "prompt_heavy"
  | "change_rejection"
  | "verification_instability";

export type AiUsageSignal = {
  code: AiUsageSignalCode;
  severity: AiUsageSignalSeverity;
  title: string;
  detail: string;
  recommendation: string;
};

export type AiContextEfficiency = {
  builds: number;
  measured_builds: number;
  total_candidate_tokens: number;
  total_included_tokens: number;
  total_saved_tokens: number;
  latest_candidate_tokens: number | null;
  latest_included_tokens: number | null;
  latest_compacted_tokens: number | null;
  latest_compactness_ratio: number | null;
  latest_repeated_tokens: number | null;
  latest_repeated_context_ratio: number | null;
};

export type AiOrchestrationEfficiency = {
  managed_executions: number;
  manual_executions: number;
  unique_workers: number;
  unique_coding_agents: number;
  handoffs: number;
  handoffs_per_managed_execution: number | null;
};

export type AiOutcomeEfficiency = {
  completed_executions: number;
  partial_executions: number;
  failed_executions: number;
  accepted_files: number;
  total_tokens: number;
  tokens_per_finished_execution: number | null;
  tokens_per_accepted_file: number | null;
  cost_per_completed_execution: number | null;
  input_output_ratio: number | null;
  execution_completion_rate: number | null;
  changeset_acceptance_rate: number | null;
  verification_pass_rate: number | null;
};

export type AiUsageReport = {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  cost_units: number;
  context: AiContextEfficiency;
  orchestration: AiOrchestrationEfficiency;
  outcomes: AiOutcomeEfficiency;
  signals: AiUsageSignal[];
};

export type StrategyOutcomeState = "pending" | "succeeded" | "failed";

export type StrategyRunFeedback = {
  run_id: string;
  requested_mode: string;
  profile: AiStrategyProfile;
  plan_shape: string;
  baseline_steps: number;
  planned_steps: number;
  baseline_estimated_tokens: number | null;
  planned_estimated_tokens: number | null;
  predicted_saved_tokens: number;
  actual_tokens: number | null;
  baseline_estimated_cost_units: number | null;
  planned_estimated_cost_units: number | null;
  actual_cost_units: number | null;
  token_estimate_error_ratio: number | null;
  execution_status: string | null;
  review_decision: string | null;
  verification_success: boolean | null;
  committed: boolean;
  outcome: StrategyOutcomeState;
};

export type StrategyProfileFeedback = {
  profile: AiStrategyProfile;
  runs: number;
  settled_runs: number;
  succeeded_runs: number;
  failed_runs: number;
  pending_runs: number;
  success_rate: number | null;
  total_actual_tokens: number;
  total_actual_cost_units: number;
  average_actual_tokens: number | null;
  average_actual_cost_units: number | null;
  average_token_estimate_error_ratio: number | null;
  adaptation_ready: boolean;
};

export type StrategyFeedbackReport = {
  strategy_runs: number;
  settled_runs: number;
  pending_runs: number;
  profiles: StrategyProfileFeedback[];
  recent_runs: StrategyRunFeedback[];
};

export type RunDispositionState = "complete" | "ready" | "attention" | "blocked";
export type RunDispositionStage = "execution" | "review" | "verification" | "acceptance" | "commit" | "complete";

export type RunDisposition = {
  state: RunDispositionState;
  stage: RunDispositionStage;
  code: string;
  title: string;
  detail: string;
};

export type RunContextObservability = {
  candidate_tokens: number | null;
  included_tokens: number | null;
  compacted_tokens: number | null;
  compactness_ratio: number | null;
  repeated_tokens: number | null;
  repeated_context_ratio: number | null;
};

export type RunStrategyObservability = {
  requested_mode: string;
  resolved_profile: string;
  plan_shape: string;
  plan_fingerprint: string;
  baseline_steps: number;
  planned_steps: number;
  estimated_saved_tokens: number;
  context_fingerprint: string | null;
};

export type RunEfficiency = {
  workers: number;
  successful_workers: number;
  failed_workers: number;
  blocked_workers: number;
  skipped_workers: number;
  handoffs: number;
  unique_providers: number;
  unique_models: number;
  total_tokens: number;
  tokens_per_changed_file: number | null;
  cost_per_changed_file: number | null;
  input_output_ratio: number | null;
};

export type RunObservabilityReport = {
  run_id: string;
  disposition: RunDisposition;
  strategy: RunStrategyObservability | null;
  context: RunContextObservability;
  efficiency: RunEfficiency;
};

export type WorkObservabilitySnapshot = {
  intelligence: EngineeringIntelligence | null;
  ai_usage_report: AiUsageReport | null;
  strategy_feedback: StrategyFeedbackReport | null;
  run_evidence: RunEvidenceSnapshot | null;
  run_observability: RunObservabilityReport | null;
};

export type RunEvidenceBundle = {
  evidence: RunEvidenceSnapshot;
  observability: RunObservabilityReport;
};

type InvokeInput = {
  runEvidenceId?: string | null;
  acceptanceCriterionId?: string | null;
  acceptanceCommand?: string | null;
};

async function invokeObservability(input?: InvokeInput): Promise<WorkObservabilitySnapshot> {
  return invoke("work_engineering_intelligence", {
    contractUpdate: null,
    scopeOverrideReason: null,
    runEvidenceId: input?.runEvidenceId ?? null,
    acceptanceCriterionId: input?.acceptanceCriterionId ?? null,
    acceptanceCommand: input?.acceptanceCommand ?? null,
    includeKnowledge: false,
    knowledgeAction: null,
  });
}

export async function workObservabilitySnapshot(): Promise<WorkObservabilitySnapshot> {
  return invokeObservability();
}

export async function runEvidenceBundle(runId: string): Promise<RunEvidenceBundle> {
  const snapshot = await invokeObservability({ runEvidenceId: runId });
  if (!snapshot.run_evidence) throw new Error(`Run evidence unavailable for ${runId}`);
  if (!snapshot.run_observability) throw new Error(`Run observability unavailable for ${runId}`);
  return { evidence: snapshot.run_evidence, observability: snapshot.run_observability };
}

export async function linkAcceptanceEvidenceBundle(
  runId: string,
  criterionId: string,
  command: string,
): Promise<RunEvidenceBundle> {
  const snapshot = await invokeObservability({
    runEvidenceId: runId,
    acceptanceCriterionId: criterionId,
    acceptanceCommand: command,
  });
  if (!snapshot.run_evidence) throw new Error(`Run evidence unavailable for ${runId}`);
  if (!snapshot.run_observability) throw new Error(`Run observability unavailable for ${runId}`);
  return { evidence: snapshot.run_evidence, observability: snapshot.run_observability };
}
