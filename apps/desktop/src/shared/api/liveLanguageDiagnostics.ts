import type { LanguageDiagnosticsEvent } from "./languageIntelligence";
import { captureLanguageDiagnostics } from "./problems";

const MAX_LSP_FILES = 200;
const diagnosticsByPath = new Map<string, LanguageDiagnosticsEvent["diagnostics"]>();

function publishAggregate() {
  captureLanguageDiagnostics([...diagnosticsByPath.values()].flat());
}

/**
 * LSP publishDiagnostics replaces diagnostics for one document URI. Preserve
 * the other document buckets instead of treating each event as a project-wide
 * replacement.
 */
export function captureLanguageDiagnosticsEvent(event: LanguageDiagnosticsEvent) {
  if (event.diagnostics.length === 0) diagnosticsByPath.delete(event.path);
  else diagnosticsByPath.set(event.path, event.diagnostics);

  while (diagnosticsByPath.size > MAX_LSP_FILES) {
    const oldest = diagnosticsByPath.keys().next().value as string | undefined;
    if (!oldest) break;
    diagnosticsByPath.delete(oldest);
  }
  publishAggregate();
}

export function clearLiveLanguageDiagnostics() {
  diagnosticsByPath.clear();
  captureLanguageDiagnostics([]);
}
