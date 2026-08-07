import { invoke } from "@tauri-apps/api/core";

export const WORK_ENGINEERING_SNAPSHOT_KEY = ["work", "engineering-snapshot"] as const;

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

export type ContextFileReason = "task_scope";
export type ContextFileStatus = "included" | "excluded";
export type ContextFileExclusionReason =
  | "invalid_path"
  | "ignored"
  | "sensitive"
  | "missing"
  | "not_file"
  | "outside_project"
  | "read_error"
  | "file_limit"
  | "too_large"
  | "budget_exceeded";

export type ContextFileEntry = {
  path: string;
  reason: ContextFileReason;
  status: ContextFileStatus;
  exclusion_reason: ContextFileExclusionReason | null;
  candidate_tokens: number | null;
  included_tokens: number | null;
  trimmed: boolean;
  fingerprint: string | null;
};

export type ContextManifest = {
  version: number;
  project: string;
  work_item_id: string;
  generated_at: string;
  included_files: number;
  excluded_files: number;
  included_file_tokens: number;
  entries: ContextFileEntry[];
};

export type ContextComponentCompactness = {
  name: string;
  candidate_tokens: number;
  included_tokens: number;
  trimmed: boolean;
  reused_from_previous_build: boolean;
};

export type ContextBuildCompactness = {
  candidate_tokens: number;
  included_tokens: number;
  compacted_tokens: number;
  compactness_ratio: number | null;
  repeated_tokens: number | null;
  repeated_context_ratio: number | null;
  components: ContextComponentCompactness[];
};

export type ContextCompactnessReport = {
  builds: number;
  measured_builds: number;
  total_candidate_tokens: number;
  total_included_tokens: number;
  total_compacted_tokens: number;
  latest: ContextBuildCompactness | null;
};

export type ContextChangeCoverage = {
  included_files: string[];
  excluded_files: string[];
  changed_files: string[];
  changed_files_present_in_context: string[];
  changed_files_missing_from_context: string[];
  change_coverage: number | null;
};

export type ContextFileEvidenceReport = {
  evidenced_context_builds: number;
  compared_changesets: number;
  latest: ContextChangeCoverage | null;
};

export type ContextInspectorReport = {
  manifest: ContextManifest | null;
  compactness: ContextCompactnessReport;
  file_evidence: ContextFileEvidenceReport;
};

export type WorkEngineeringSnapshot = {
  intelligence: EngineeringIntelligence;
  context_inspector: ContextInspectorReport;
};

export async function workEngineeringSnapshot(): Promise<WorkEngineeringSnapshot> {
  return invoke("work_engineering_intelligence");
}

export async function workEngineeringIntelligence(): Promise<EngineeringIntelligence> {
  return (await workEngineeringSnapshot()).intelligence;
}

export async function workContextInspector(): Promise<ContextInspectorReport> {
  return (await workEngineeringSnapshot()).context_inspector;
}
