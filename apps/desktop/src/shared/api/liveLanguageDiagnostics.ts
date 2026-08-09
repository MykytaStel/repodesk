import type { LanguageDiagnosticsEvent } from "./languageIntelligence";
import { captureLanguageDiagnostics } from "./problems";

const MAX_LSP_FILES = 64;
const MAX_LSP_DIAGNOSTICS = 500;
const diagnosticsByPath = new Map<string, LanguageDiagnosticsEvent["diagnostics"]>();

function diagnosticCount(): number {
  let count = 0;
  for (const diagnostics of diagnosticsByPath.values()) count += diagnostics.length;
  return count;
}

function publishAggregate() {
  captureLanguageDiagnostics(
    [...diagnosticsByPath.values()].flat().slice(0, MAX_LSP_DIAGNOSTICS),
  );
}

/**
 * LSP publishDiagnostics replaces diagnostics for one document URI. Preserve
 * the other document buckets instead of treating each event as a project-wide
 * replacement, while keeping the complete live cache inside the same 500-item
 * budget exposed by the Problems store.
 */
export function captureLanguageDiagnosticsEvent(event: LanguageDiagnosticsEvent) {
  if (event.diagnostics.length === 0) diagnosticsByPath.delete(event.path);
  else diagnosticsByPath.set(event.path, event.diagnostics.slice(0, MAX_LSP_DIAGNOSTICS));

  while (diagnosticsByPath.size > MAX_LSP_FILES || diagnosticCount() > MAX_LSP_DIAGNOSTICS) {
    const oldest = diagnosticsByPath.keys().next().value as string | undefined;
    if (!oldest) break;
    // Keep a single very noisy file, already capped to MAX_LSP_DIAGNOSTICS.
    if (diagnosticsByPath.size === 1) break;
    diagnosticsByPath.delete(oldest);
  }
  publishAggregate();
}

export function clearLiveLanguageDiagnostics() {
  diagnosticsByPath.clear();
  captureLanguageDiagnostics([]);
}
