import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { callCommand } from "./queries";

export type LanguageServerAvailability = "available" | "missing";
export type LanguageServerSource = "project_local" | "path";
export type LanguageServerProfileState = "active" | "discovery_only";
export type LanguageServerInitializationProfile = "default" | "taplo";

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
  profile_state: LanguageServerProfileState;
  initialization_profile: LanguageServerInitializationProfile;
  install_recipe_id: string | null;
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

export type LanguageServerSessionState = "starting" | "ready" | "error";

export type LanguageServerStatus = {
  project: string;
  server_id: string;
  state: LanguageServerSessionState;
  pid: number;
  open_documents: number;
  started_at: string;
  last_error: string | null;
};

export type LanguageHover = {
  markdown: string;
  range: LspRange | null;
};

export type LanguageLocation = {
  path: string;
  library_handle: string | null;
  read_only: boolean;
  line: number;
  column: number;
  end_line: number;
  end_column: number;
};

export type LanguageSymbol = {
  name: string;
  detail: string | null;
  kind: number;
  range: LspRange;
  selection_range: LspRange;
  container_name: string | null;
};

export type LanguageDiagnosticsEvent = {
  project: string;
  server_id: string;
  path: string;
  diagnostics: LanguageDiagnostic[];
};

export const LANGUAGE_INTELLIGENCE_KEY = ["language_intelligence"] as const;

type LanguageAction =
  | { kind: "status" }
  | { kind: "sync_document"; path: string; language: string; text: string }
  | { kind: "close_document"; path: string }
  | { kind: "hover"; path: string; text: string; line: number; column: number }
  | { kind: "definition"; path: string; text: string; line: number; column: number }
  | { kind: "references"; path: string; text: string; line: number; column: number }
  | { kind: "document_symbols"; path: string; text: string }
  | { kind: "stop" };

function languageAction<T>(action: LanguageAction): Promise<T> {
  return callCommand<T>("language_intelligence_snapshot", { action });
}

export function languageIntelligenceSnapshot(): Promise<LanguageIntelligenceSnapshot> {
  return callCommand<LanguageIntelligenceSnapshot>("language_intelligence_snapshot", { action: null });
}

export function languageServerStatus(): Promise<LanguageServerStatus | null> {
  return languageAction<LanguageServerStatus | null>({ kind: "status" });
}

export function syncLanguageDocument(input: {
  path: string;
  language: string;
  text: string;
}): Promise<LanguageServerStatus> {
  return languageAction<LanguageServerStatus>({ kind: "sync_document", ...input });
}

export function closeLanguageDocument(path: string): Promise<null> {
  return languageAction<null>({ kind: "close_document", path });
}

export function requestLanguageHover(input: {
  path: string;
  text: string;
  line: number;
  column: number;
}): Promise<LanguageHover | null> {
  return languageAction<LanguageHover | null>({ kind: "hover", ...input });
}

export function requestLanguageDefinition(input: {
  path: string;
  text: string;
  line: number;
  column: number;
}): Promise<LanguageLocation[]> {
  return languageAction<LanguageLocation[]>({ kind: "definition", ...input });
}

export function requestLanguageReferences(input: {
  path: string;
  text: string;
  line: number;
  column: number;
}): Promise<LanguageLocation[]> {
  return languageAction<LanguageLocation[]>({ kind: "references", ...input });
}

export function requestLanguageDocumentSymbols(input: {
  path: string;
  text: string;
}): Promise<LanguageSymbol[]> {
  return languageAction<LanguageSymbol[]>({ kind: "document_symbols", ...input });
}

export function stopLanguageServer(): Promise<null> {
  return languageAction<null>({ kind: "stop" });
}

export function subscribeLanguageDiagnostics(
  listener: (event: LanguageDiagnosticsEvent) => void,
): Promise<UnlistenFn> {
  return listen<LanguageDiagnosticsEvent>("language-diagnostics", (event) => listener(event.payload));
}

export function subscribeLanguageServerStatus(
  listener: (status: LanguageServerStatus) => void,
): Promise<UnlistenFn> {
  return listen<LanguageServerStatus>("language-server-status", (event) => listener(event.payload));
}

export function languageServerFor(
  snapshot: LanguageIntelligenceSnapshot | undefined,
  language: string | null | undefined,
): LanguageServerDescriptor | null {
  if (!snapshot || !language) return null;
  const candidates = snapshot.servers.filter((server) => server.languages.includes(language));
  return candidates.find((server) => server.availability === "available") ?? candidates[0] ?? null;
}
