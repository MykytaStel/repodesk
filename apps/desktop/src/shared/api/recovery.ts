import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { callCommand } from "./queries";

export type RecoveryState =
  | "healthy"
  | "degraded"
  | "repairing"
  | "needs_approval"
  | "blocked"
  | "unknown";
export type RecoverySeverity = "info" | "warning" | "error";
export type RecoveryFailureCode =
  | "missing_executable"
  | "incompatible_version"
  | "process_crashed"
  | "initialization_failed"
  | "request_timed_out"
  | "invalid_configuration"
  | "permission_denied"
  | "unknown_failure";
export type RecoveryActionKind = "automatic" | "confirmable" | "manual";
export type RecoveryAttemptResult =
  | "verified"
  | "failed"
  | "verification_failed"
  | "cancelled";
export type RecoveryRisk = "low" | "moderate" | "high";

export type RecoveryAction = {
  id: string;
  label: string;
  kind: RecoveryActionKind;
  recipe_id: string | null;
};

export type RecoveryEvidence = {
  label: string;
  value: string;
};

export type RecoveryRecord = {
  capability_id: string;
  module_id: string;
  generation: number;
  diagnosis_revision: string;
  observed_at: string;
  state: RecoveryState;
  severity: RecoverySeverity;
  code: RecoveryFailureCode | null;
  title: string;
  explanation: string;
  affected: string[];
  unaffected: string[];
  evidence: RecoveryEvidence[];
  actions: RecoveryAction[];
  automatic_attempts: number;
};

export type RecoverySnapshot = {
  project: string;
  records: RecoveryRecord[];
  actionable_count: number;
  warnings: string[];
  generated_at: string;
};

export type RecoveryAttempt = {
  id: string;
  capability_id: string;
  diagnosis_revision: string;
  action_id: string;
  started_at: string;
  finished_at: string | null;
  result: RecoveryAttemptResult | null;
  verification_summary: string | null;
};

export type RecoveryRepairPreview = {
  capability_id: string;
  diagnosis_revision: string;
  action_id: string;
  title: string;
  summary: string;
  risk: RecoveryRisk;
  recipe_id: string;
  recipe_revision: string;
  changes: string[];
  network_required: boolean;
  verification: string;
  confirmation_token: string;
  expires_at: string;
};

export const RECOVERY_CHANGED_EVENT = "recovery-record-changed";
export const RECOVERY_QUERY_KEY = ["recovery_snapshot"] as const;
export const RECOVERY_HISTORY_QUERY_KEY = ["recovery_history"] as const;

export function recoverySnapshot(): Promise<RecoverySnapshot> {
  return callCommand<RecoverySnapshot>("recovery_snapshot");
}

export function recoveryHistory(): Promise<RecoveryAttempt[]> {
  return callCommand<RecoveryAttempt[]>("recovery_history");
}

export function recoveryCheck(capabilityId: string): Promise<RecoveryRecord> {
  return callCommand<RecoveryRecord>("recovery_check", { capabilityId });
}

export function recoveryRepairPreview(
  capabilityId: string,
  actionId: string,
): Promise<RecoveryRepairPreview> {
  return callCommand<RecoveryRepairPreview>("recovery_repair_preview", {
    capabilityId,
    actionId,
  });
}

export function recoveryRepairConfirm(confirmationToken: string): Promise<RecoveryRecord> {
  return callCommand<RecoveryRecord>("recovery_repair_confirm", { confirmationToken });
}

export function recoveryRepairCancel(recipeId: string): Promise<boolean> {
  return callCommand<boolean>("recovery_repair_cancel", { recipeId });
}

export function subscribeRecoveryChanges(
  listener: (record: RecoveryRecord) => void,
): Promise<UnlistenFn> {
  return listen<RecoveryRecord>(RECOVERY_CHANGED_EVENT, (event) => listener(event.payload));
}
