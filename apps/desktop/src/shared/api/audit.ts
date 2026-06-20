import { invoke } from "@tauri-apps/api/core";

/** One entry in the hash-chained audit trail. */
export type AuditEvent = {
  timestamp: string;
  project_name: string;
  action_type: string;
  details: string;
  previous_hash: string;
  hash: string;
};

/** Result of verifying the audit-trail hash chain. */
export type ChainVerification = {
  valid: boolean;
  total_events: number;
  broken_at: number | null;
  message: string;
};

export async function auditRecent(limit = 50): Promise<AuditEvent[]> {
  return invoke("audit_recent", { limit });
}

export async function auditVerify(): Promise<ChainVerification> {
  return invoke("audit_verify");
}
