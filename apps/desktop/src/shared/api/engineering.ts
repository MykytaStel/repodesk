import { invoke } from "@tauri-apps/api/core";

export type ExecutionIntelligence = {
  attempts: number;
  finished: number;
  completed: number;
  partial: number;
  failed: number;
  dry_runs: number;
  unfinished: number;
  managed: number;
  manual: number;
  unknown_mode: number;
  unique_workers: number;
  unique_coding_agents: number;
  handoffs: number;
};

export type AiUsageIntelligence = {
  input_tokens: number;
  output_tokens: number;
  cost_units: number;
};

export type ContextIntelligence = {
  builds: number;
  total_estimated_tokens: number;
  latest_estimated_tokens: number | null;
};

export type ChangeIntelligence = {
  proposed_changesets: number;
  proposed_files: number;
  reviewed_changesets: number;
  accepted_changesets: number;
  rejected_changesets: number;
  pending_review_changesets: number;
  accepted_files: number;
  rejected_files: number;
};

export type VerificationIntelligence = {
  attempts: number;
  finished: number;
  passed: number;
  failed: number;
  unfinished: number;
  commands_run: number;
};

export type CompletionIntelligence = {
  committed: boolean;
  commits: number;
  committed_files: number;
  latest_commit_sha: string | null;
};

export type IntelligenceRates = {
  execution_completion_rate: number | null;
  changeset_acceptance_rate: number | null;
  verification_pass_rate: number | null;
};

export type EngineeringIntelligence = {
  project: string | null;
  work_item_id: string | null;
  event_count: number;
  execution: ExecutionIntelligence;
  ai_usage: AiUsageIntelligence;
  context: ContextIntelligence;
  changes: ChangeIntelligence;
  verification: VerificationIntelligence;
  completion: CompletionIntelligence;
  rates: IntelligenceRates;
};

export async function workEngineeringIntelligence(): Promise<EngineeringIntelligence> {
  return invoke("work_engineering_intelligence");
}
