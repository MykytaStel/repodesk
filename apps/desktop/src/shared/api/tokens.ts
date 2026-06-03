import { invoke } from "@tauri-apps/api/core";

export type TokenUsageItem = {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_cost_units?: number | null;
  currency_label?: string | null;
};

export type TokenUsageSnapshot = {
  generated_at_ms: number;
  totals: {
    entries_count: number;
    total_input_tokens: number;
    total_output_tokens: number;
    total_tokens: number;
    today_total_tokens: number;
    remaining_daily_tokens: number;
  };
  by_provider: TokenUsageItem[];
  by_model: TokenUsageItem[];
  active_artifacts: Array<{
    kind: string;
    title: string;
    path?: string | null;
    exists: boolean;
    size_bytes: number;
    estimated_tokens?: number | null;
    status: string;
    recommendation: string;
    error?: string | null;
  }>;
  cost_estimate: {
    estimated_total_units: number;
    currency_label: string;
    note: string;
  };
};

export type LogTokenUsageInput = {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  category: string;
  notes?: string | null;
};

export type FileTokenEstimate = {
  path: string;
  bytes: number;
  estimated_tokens: number;
  status: string;
};

export async function tokenUsageSnapshot(): Promise<TokenUsageSnapshot> {
  return invoke("token_usage_snapshot");
}

export async function logTokenUsage(input: LogTokenUsageInput): Promise<TokenUsageSnapshot> {
  return invoke("log_token_usage", { input });
}

export async function getProjectFileTokenEstimates(): Promise<FileTokenEstimate[]> {
  return invoke("get_project_file_token_estimates");
}
