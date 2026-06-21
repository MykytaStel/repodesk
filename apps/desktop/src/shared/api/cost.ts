import { invoke } from "@tauri-apps/api/core";

// The cost rate card (costs.toml). Rates are per-1K-token in `currency_label`.
export type AgentRate = {
  agent: string;
  model: string;
  input_cost_per_1k_units: number;
  output_cost_per_1k_units: number;
  notes: string;
};

export type CostConfig = {
  currency_label: string;
  rates: AgentRate[];
};

export async function getCostConfig(): Promise<CostConfig> {
  return invoke("cost_config_get");
}

export async function saveCostConfig(config: CostConfig): Promise<CostConfig> {
  return invoke("cost_config_save", { config });
}

export async function resetCostConfig(): Promise<CostConfig> {
  return invoke("cost_config_reset");
}
