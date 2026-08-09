import { callCommand } from "./queries";

export type LanguageServerAvailability = "available" | "missing";
export type LanguageServerSource = "project_local" | "path";

export type LanguageServerCapabilities = {
  diagnostics: boolean;
  hover: boolean;
  definition: boolean;
  references: boolean;
  completion: boolean;
  rename: boolean;
  formatting: boolean;
  document_symbols: boolean;
};

export type LanguageServerDescriptor = {
  id: string;
  label: string;
  executable: string;
  arguments: string[];
  languages: string[];
  availability: LanguageServerAvailability;
  source: LanguageServerSource | null;
  capabilities: LanguageServerCapabilities;
};

export type LanguageIntelligenceSnapshot = {
  project: string;
  primary_language: string | null;
  servers: LanguageServerDescriptor[];
  available_count: number;
  generated_at: string;
};

export type LspPosition = {
  line: number;
  character: number;
};

export type LspRange = {
  start: LspPosition;
  end: LspPosition;
};

export type LanguageDiagnosticSeverity = "error" | "warning" | "information" | "hint";

export type LanguageDiagnostic = {
  server_id: string;
  path: string;
  range: LspRange;
  severity: LanguageDiagnosticSeverity;
  message: string;
  code: string | null;
  source: string | null;
};

export const LANGUAGE_INTELLIGENCE_KEY = ["language_intelligence"] as const;

export function languageIntelligenceSnapshot(): Promise<LanguageIntelligenceSnapshot> {
  return callCommand<LanguageIntelligenceSnapshot>("language_intelligence_snapshot");
}

export function languageServerFor(
  snapshot: LanguageIntelligenceSnapshot | undefined,
  language: string | null | undefined,
): LanguageServerDescriptor | null {
  if (!snapshot || !language) return null;
  const candidates = snapshot.servers.filter((server) => server.languages.includes(language));
  return candidates.find((server) => server.availability === "available") ?? candidates[0] ?? null;
}
