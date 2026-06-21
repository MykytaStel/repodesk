import { invoke } from "@tauri-apps/api/core";

// User-added OpenAI-compatible providers (DeepSeek, Groq, OpenRouter, …). Keys
// come back masked; saving the mask keeps the stored key.
export type CustomProvider = {
  id: string;
  label: string;
  base_url: string;
  api_key: string;
  default_model: string;
  enabled: boolean;
};

export type ProviderPreset = {
  id: string;
  label: string;
  base_url: string;
  default_model: string;
  key_env_hint: string;
};

export async function listCustomProviders(): Promise<CustomProvider[]> {
  return invoke("custom_providers_list");
}

export async function customProviderPresets(): Promise<ProviderPreset[]> {
  return invoke("custom_providers_presets");
}

export async function saveCustomProvider(provider: CustomProvider): Promise<CustomProvider[]> {
  return invoke("custom_providers_save", { provider });
}

export async function deleteCustomProvider(id: string): Promise<CustomProvider[]> {
  return invoke("custom_providers_delete", { id });
}
