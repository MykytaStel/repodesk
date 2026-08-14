import { invoke } from "@tauri-apps/api/core";

export type CanonicalAuditEvent = {
  timestamp: string;
  project: string;
  task_id: string;
  module_name: string;
  level: string;
  message: string;
  metadata: Record<string, string>;
};

export type CanonicalAuditSnapshot = {
  generated_at: string;
  total_entries: number;
  returned: number;
  counts_by_severity: Record<string, number>;
  entries: CanonicalAuditEvent[];
};

/**
 * Read the canonical SQLite engineering ledger. The backend verifies the full
 * sequence/hash chain before returning any projection, so corruption fails
 * closed instead of being represented as an empty or partially trusted view.
 */
export async function auditSnapshot(limit = 50): Promise<CanonicalAuditSnapshot> {
  return invoke("get_event_journal", { input: { limit } });
}
