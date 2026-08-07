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
  | "blocked_by_security"
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

export type ScopeComplianceStatus =
  | "not_evaluated"
  | "unconfigured"
  | "compliant"
  | "violation";

export type WorkItemContract = {
  version: number;
  project: string;
  work_item_id: string;
  goal: string;
  allowed_paths: string[];
  protected_paths: string[];
  acceptance_criteria: string[];
  updated_at: string;
};

export type WorkItemContractUpdate = {
  goal: string;
  allowed_paths: string[];
  protected_paths: string[];
  acceptance_criteria: string[];
};

export type WorkItemContractReadiness = {
  goal_defined: boolean;
  scope_defined: boolean;
  acceptance_defined: boolean;
  protected_paths_defined: boolean;
};

export type ScopeComplianceReport = {
  status: ScopeComplianceStatus;
  changed_files: string[];
  allowed_changed_files: string[];
  out_of_scope_files: string[];
  protected_changed_files: string[];
};

export type WorkItemContractSnapshot = {
  configured: boolean;
  contract: WorkItemContract;
  readiness: WorkItemContractReadiness;
  compliance: ScopeComplianceReport;
};

export type WorkerKind =
  | "human"
  | "coding_agent"
  | "inference"
  | "check_runner"
  | "script"
  | "ci"
  | "manual"
  | "unknown";

export type WorkerRef = {
  kind: WorkerKind;
  id: string;
  provider: string | null;
  model: string | null;
};

export type ChangeFileScopeState = "allowed" | "out_of_scope" | "protected" | "ungoverned";

export type ChangeFileGovernance = {
  path: string;
  scope_state: ChangeFileScopeState;
};

export type ChangeOrigin = {
  execution_id: string | null;
  execution_mode: string | null;
  workers: WorkerRef[];
};

export type ChangeReviewState = "proposed" | "accepted" | "rejected";
export type ChangeVerificationState = "not_run" | "running" | "passed" | "failed";

export type EvidenceRef = { kind: string; locator: string };

export type ChangeVerificationEvidence = {
  state: ChangeVerificationState;
  verification_id: string | null;
  command_count: number;
  evidence: EvidenceRef[];
  error: string | null;
};

export type ScopeOverrideEvidence = {
  event_id: string;
  reason: string;
  occurred_at: string;
};

export type CommitGateState =
  | "no_change_set"
  | "scope_violation"
  | "needs_review"
  | "rejected"
  | "verification_required"
  | "verification_running"
  | "verification_failed"
  | "ready"
  | "committed";

export type CommitGate = {
  state: CommitGateState;
  ready: boolean;
  blockers: string[];
  warnings: string[];
};

export type ChangeGovernanceSnapshot = {
  work_item_id: string;
  changeset_id: string | null;
  origin: ChangeOrigin;
  files: ChangeFileGovernance[];
  scope_status: ScopeComplianceStatus;
  review_state: ChangeReviewState;
  verification: ChangeVerificationEvidence;
  scope_override: ScopeOverrideEvidence | null;
  committed: boolean;
  commit_sha: string | null;
  gate: CommitGate;
};

export type AcceptanceCriterionStatus = "unproven" | "proven" | "failed";

export type AcceptanceCriterionEvidence = {
  criterion_id: string;
  criterion: string;
  status: AcceptanceCriterionStatus;
  command: string | null;
  run_id: string | null;
  linked_at: string | null;
  stale: boolean;
  stale_reason: string | null;
};

export type AcceptanceEvidenceReport = {
  configured: boolean;
  work_item_id: string;
  current_run_id: string | null;
  criteria: AcceptanceCriterionEvidence[];
  proven: number;
  failed: number;
  unproven: number;
};

export type VerificationCommandEvidence = {
  command: string;
  success: boolean;
};

export type RunWorkerEvidence = {
  step_id: string;
  agent: string;
  provider: string;
  model: string;
  status: "ok" | "skipped" | "blocked" | "failed";
  changed_files: string[];
  input_tokens: number;
  output_tokens: number;
  cost_units: number;
};

export type RunContextEvidence = {
  estimated_tokens: number | null;
  evidence: EvidenceRef[];
  source: string;
};

export type RunReviewEvidence = {
  state: string;
  reviewed_paths: string[];
  source: string;
};

export type RunVerificationEvidence = {
  state: string;
  verification_id: string | null;
  commands: VerificationCommandEvidence[];
  evidence: EvidenceRef[];
  verified_at: string | null;
  source: string;
};

export type RunCommitEvidence = {
  committed: boolean;
  commit_sha: string | null;
  committed_paths: string[];
  source: string;
};

export type RunEvidenceSnapshot = {
  run_id: string;
  project: string;
  work_item_id: string;
  goal: string;
  status: "completed" | "partial" | "failed" | "dry_run";
  dry_run: boolean;
  started_at: string;
  finished_at: string;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_units: number;
  workers: RunWorkerEvidence[];
  changed_files: string[];
  context: RunContextEvidence;
  review: RunReviewEvidence;
  verification: RunVerificationEvidence;
  commit: RunCommitEvidence;
  acceptance: AcceptanceEvidenceReport;
};

export type WorkEngineeringSnapshot = {
  intelligence: EngineeringIntelligence;
  context_inspector: ContextInspectorReport;
  work_item_contract: WorkItemContractSnapshot;
  change_governance: ChangeGovernanceSnapshot;
  run_evidence: RunEvidenceSnapshot | null;
};

async function invokeWorkEngineering(input?: {
  contractUpdate?: WorkItemContractUpdate | null;
  scopeOverrideReason?: string | null;
  runEvidenceId?: string | null;
  acceptanceCriterionId?: string | null;
  acceptanceCommand?: string | null;
}): Promise<WorkEngineeringSnapshot> {
  return invoke("work_engineering_intelligence", {
    contractUpdate: input?.contractUpdate ?? null,
    scopeOverrideReason: input?.scopeOverrideReason ?? null,
    runEvidenceId: input?.runEvidenceId ?? null,
    acceptanceCriterionId: input?.acceptanceCriterionId ?? null,
    acceptanceCommand: input?.acceptanceCommand ?? null,
  });
}

export async function workEngineeringSnapshot(): Promise<WorkEngineeringSnapshot> {
  return invokeWorkEngineering();
}

export async function workEngineeringIntelligence(): Promise<EngineeringIntelligence> {
  return (await workEngineeringSnapshot()).intelligence;
}

export async function workContextInspector(): Promise<ContextInspectorReport> {
  return (await workEngineeringSnapshot()).context_inspector;
}

export async function saveWorkItemContract(
  update: WorkItemContractUpdate,
): Promise<WorkItemContractSnapshot> {
  return (await invokeWorkEngineering({ contractUpdate: update })).work_item_contract;
}

export async function recordScopeOverride(reason: string): Promise<ChangeGovernanceSnapshot> {
  return (await invokeWorkEngineering({ scopeOverrideReason: reason })).change_governance;
}

export async function runEvidenceSnapshot(runId: string): Promise<RunEvidenceSnapshot> {
  const snapshot = await invokeWorkEngineering({ runEvidenceId: runId });
  if (!snapshot.run_evidence) throw new Error(`Run evidence unavailable for ${runId}`);
  return snapshot.run_evidence;
}

export async function linkAcceptanceEvidence(
  runId: string,
  criterionId: string,
  command: string,
): Promise<RunEvidenceSnapshot> {
  const snapshot = await invokeWorkEngineering({
    runEvidenceId: runId,
    acceptanceCriterionId: criterionId,
    acceptanceCommand: command,
  });
  if (!snapshot.run_evidence) throw new Error(`Run evidence unavailable for ${runId}`);
  return snapshot.run_evidence;
}
