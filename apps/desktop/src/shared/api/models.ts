import { invoke } from "@tauri-apps/api/core";

export type ModelStatus = {
  id: string;
  provider: string;
  available: boolean;
  loaded?: boolean | null;
  context_window?: number | null;
  notes?: string | null;
};

export type ProviderHealth = {
  id: string;
  label: string;
  enabled: boolean;
  auth_status: string;
  reachability: string;
  models: ModelStatus[];
  error_summary?: string | null;
};

export type ModelHealthSnapshot = {
  generated_at_ms: number;
  providers: ProviderHealth[];
  warnings: string[];
};

export async function modelHealthSnapshot(): Promise<ModelHealthSnapshot> {
  return invoke("model_health_snapshot");
}

export async function refreshModelHealth(): Promise<ModelHealthSnapshot> {
  return invoke("refresh_model_health");
}
