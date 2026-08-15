import { invoke } from "@tauri-apps/api/core";

export type CredentialSource = "keychain" | "environment" | "none";

/** Non-secret credential metadata — the full secret never crosses the boundary. */
export type CredentialMetadata = {
  key: string;
  configured: boolean;
  hint: string;
  source: CredentialSource;
};

/** Canonical keychain keys the desktop is allowed to manage. */
export const CREDENTIAL_KEYS = {
  openai: "openai_api_key",
  anthropic: "anthropic_api_key",
  gemini: "gemini_api_key",
} as const;

function isCredentialSource(value: unknown): value is CredentialSource {
  return value === "keychain" || value === "environment" || value === "none";
}

export async function credentialStatus(): Promise<CredentialMetadata[]> {
  const result = await invoke<unknown>("credential_status");
  if (!Array.isArray(result)) return [];
  return result.filter((entry): entry is CredentialMetadata => (
    Boolean(entry)
    && typeof entry === "object"
    && typeof (entry as CredentialMetadata).key === "string"
    && typeof (entry as CredentialMetadata).configured === "boolean"
    && typeof (entry as CredentialMetadata).hint === "string"
    && isCredentialSource((entry as CredentialMetadata).source)
  ));
}

export async function credentialSet(key: string, value: string): Promise<CredentialMetadata> {
  return invoke("credential_set", { key, value });
}

export async function credentialDelete(key: string): Promise<CredentialMetadata> {
  return invoke("credential_delete", { key });
}
