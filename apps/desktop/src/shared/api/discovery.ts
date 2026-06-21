import { invoke } from "@tauri-apps/api/core";

// Machine-level AI discovery: a passive scan of installed CLIs, desktop apps and
// localhost endpoints (PATH / known app paths / open ports). This is "what AI is
// installed on this machine" — distinct from the project AI import (what AI
// artifacts live inside the connected repo).
export type AiToolProbe = {
  id: string;
  name: string;
  category: string;
  status: "available" | "missing" | "maybe" | "unknown";
  detection: string;
  executable_path?: string | null;
  app_path?: string | null;
  local_only: boolean;
  requires_paid_account: boolean;
  risk_level: string;
  notes: string[];
};

export type AiEndpointProbe = {
  id: string;
  name: string;
  url: string;
  status: "available" | "missing" | "maybe" | "unknown";
  local_only: boolean;
  notes: string[];
};

export type AiDiscoveryReport = {
  generated_at: string;
  host_os: string;
  tools: AiToolProbe[];
  endpoints: AiEndpointProbe[];
  recommendations: string[];
  warnings: string[];
  report_path?: string | null;
};

export async function aiDiscoveryScan(): Promise<AiDiscoveryReport> {
  return invoke("ai_discovery_scan");
}
