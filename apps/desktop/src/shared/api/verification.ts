import { callCommand } from "./queries";

export type VerificationDecisionKind =
  | "run_now"
  | "run_targeted"
  | "defer_with_debt"
  | "ask_for_approval"
  | "pause_and_review"
  | "stop_with_partial_result";

export type VerificationDebt = {
  check_id: string;
  title: string;
  reason: string;
  required_before: string;
};

export type EvidenceRef = {
  kind: string;
  locator: string;
};

export type VerificationCheckCandidate = {
  id: string;
  title: string;
  command: string;
  kind: string;
  required: boolean;
  estimated_seconds: number | null;
  estimated_cost_units: number | null;
  relevant_paths: string[];
  last_status: string | null;
};

export type VerificationAdvisorInput = {
  project: string;
  work_item_id: string;
  tree_identity: string;
  changed_files: string[];
  risk_label: string;
  proof_obligations: string[];
  checks: VerificationCheckCandidate[];
  estimated_budget_units: number | null;
  prior_attempts: number;
  prior_failures: number;
  policy_version: string;
};

export type VerificationRecommendation = {
  decision: VerificationDecisionKind;
  rationale: string[];
  selected_check_ids: string[];
  deferred_checks: VerificationDebt[];
  estimated_cost_units: number | null;
  estimated_wall_clock_ms: number | null;
  risk_label: string;
  uncertainty_label: string;
  policy_version: string;
};

export type DecisionReceipt = {
  decision_id: string;
  project: string;
  work_item_id: string;
  execution_id: string | null;
  tree_identity: string;
  changeset_identity: string | null;
  decision_kind: VerificationDecisionKind;
  policy_version: string;
  observed_facts: Record<string, unknown>;
  evidence_refs: EvidenceRef[];
  selected_actions: string[];
  skipped_actions: string[];
  estimated_cost_units: number | null;
  estimated_wall_clock_ms: number | null;
  risk_label: string;
  uncertainty_label: string;
  verification_debt: VerificationDebt[];
  human_override: string | null;
  outcome: string | null;
  created_at: string;
};

export type RecordDecisionInput = Omit<DecisionReceipt, "decision_id" | "created_at" | "outcome"> & {
  decision_id?: string | null;
};

export type VerificationSourceStatus = {
  source: string;
  status: string;
  detail: string;
};

export type VerificationAdvisorSnapshot = {
  project: string;
  work_item_id: string;
  current_tree_identity: string | null;
  input: VerificationAdvisorInput;
  recommendation: VerificationRecommendation;
  latest_receipt: DecisionReceipt | null;
  sources: VerificationSourceStatus[];
};

export function workVerificationAdvisor(): Promise<VerificationAdvisorSnapshot> {
  return callCommand("work_verification_advisor");
}

export function recordWorkDecision(input: RecordDecisionInput): Promise<DecisionReceipt> {
  return callCommand("work_record_decision", { input });
}

export function getDecisionReceipt(decisionId?: string): Promise<DecisionReceipt | null> {
  return callCommand("work_decision_receipt", { decisionId: decisionId ?? null });
}
