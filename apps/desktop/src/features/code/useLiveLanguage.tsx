import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import {
  closeLanguageDocument,
  requestLanguageDefinition,
  requestLanguageDocumentSymbols,
  requestLanguageHover,
  requestLanguageReferences,
  stopLanguageServer,
  subscribeLanguageDiagnostics,
  subscribeLanguageServerStatus,
  syncLanguageDocument,
  type LanguageHover,
  type LanguageLocation,
  type LanguageServerDescriptor,
  type LanguageServerStatus,
  type LanguageSymbol,
} from "../../shared/api/languageIntelligence";
import {
  captureLanguageDiagnosticsEvent,
  clearLiveLanguageDiagnostics,
} from "../../shared/api/liveLanguageDiagnostics";
import { errorToMessage } from "../../shared/utils/helpers";
import "./live-language.css";

const CHANGE_DEBOUNCE_MS = 350;
const OWNER_STOP_GRACE_MS = 250;
let liveEditorOwners = 0;
let pendingStop: number | null = null;

type LanguageKeyEvent = {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  preventDefault(): void;
};

type LanguagePosition = { line: number; column: number };

export type LanguageDefinitionPreview = {
  hover: LanguageHover | null;
  definitions: LanguageLocation[];
};

function acquireLanguageServerOwner(): () => void {
  liveEditorOwners += 1;
  if (pendingStop !== null) {
    window.clearTimeout(pendingStop);
    pendingStop = null;
  }

  return () => {
    liveEditorOwners = Math.max(0, liveEditorOwners - 1);
    if (liveEditorOwners !== 0 || pendingStop !== null) return;
    pendingStop = window.setTimeout(() => {
      pendingStop = null;
      if (liveEditorOwners !== 0) return;
      clearLiveLanguageDiagnostics();
      void stopLanguageServer().catch(() => undefined);
    }, OWNER_STOP_GRACE_MS);
  };
}

type LanguagePanel =
  | { kind: "hover"; title: string; hover: LanguageHover }
  | { kind: "locations"; title: string; locations: LanguageLocation[] }
  | { kind: "symbols"; title: string; symbols: LanguageSymbol[] };

function locationLabel(location: LanguageLocation): string {
  return `${location.path}:${location.line}:${location.column}`;
}

function languageServiceLabel(language: string, server: LanguageServerDescriptor): string {
  if (server.id === "rust-analyzer") return "RA";
  if (language === "typescript" || language === "javascript") return "TS";
  if (language === "toml") return "TOML";
  if (language === "json") return "JSON";
  if (language === "yaml") return "YAML";
  return "LS";
}

export function useLiveLanguage({
  path,
  value,
  language,
  projectName,
  server,
  cursor,
}: {
  path: string;
  value: string;
  language: string;
  projectName: string | null | undefined;
  server: LanguageServerDescriptor | null;
  cursor: LanguagePosition;
}): {
  status: LanguageServerStatus | null;
  statusLabel: string | null;
  statusTitle: string | null;
  busy: boolean;
  panel: ReactNode;
  actions: {
    hover: () => void;
    definition: () => void;
    definitionAt: (position: LanguagePosition) => void;
    previewAt: (position: LanguagePosition) => Promise<LanguageDefinitionPreview | null>;
    clearPreview: () => void;
    references: () => void;
    symbols: () => void;
    dismiss: () => void;
  };
  handleKeyDown: (event: LanguageKeyEvent) => boolean;
} {
  const enabled = server?.profile_state === "active"
    && server.availability === "available"
    && server.languages.includes(language);
  const capabilities = server?.capabilities;
  const [status, setStatus] = useState<LanguageServerStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [panel, setPanel] = useState<LanguagePanel | null>(null);
  const [error, setError] = useState<string | null>(null);
  const lastSynced = useRef<{ path: string; text: string } | null>(null);
  const previewRequest = useRef(0);
  const valueRef = useRef(value);
  const cursorRef = useRef(cursor);
  valueRef.current = value;
  cursorRef.current = cursor;

  useEffect(() => {
    if (!enabled) return;
    return acquireLanguageServerOwner();
  }, [enabled, projectName]);

  useEffect(() => {
    clearLiveLanguageDiagnostics();
    setStatus(null);
  }, [projectName]);

  useEffect(() => {
    if (!enabled || !projectName) return;
    let disposed = false;
    let unlistenDiagnostics: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;

    void subscribeLanguageDiagnostics((event) => {
      if (event.project !== projectName || event.server_id !== server?.id || disposed) return;
      captureLanguageDiagnosticsEvent(event);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenDiagnostics = unlisten;
    }).catch(() => undefined);

    void subscribeLanguageServerStatus((next) => {
      if (next.project === projectName && next.server_id === server?.id && !disposed) setStatus(next);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenStatus = unlisten;
    }).catch(() => undefined);

    return () => {
      disposed = true;
      unlistenDiagnostics?.();
      unlistenStatus?.();
    };
  }, [enabled, projectName, server?.id]);

  useEffect(() => {
    setPanel(null);
    setError(null);
    lastSynced.current = null;
    if (!enabled) {
      setStatus(null);
      return;
    }

    let cancelled = false;
    const initialText = valueRef.current;
    lastSynced.current = { path, text: initialText };
    void syncLanguageDocument({ path, language, text: initialText })
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((cause) => {
        if (!cancelled) setError(errorToMessage(cause));
      });

    return () => {
      cancelled = true;
      if (lastSynced.current?.path === path) lastSynced.current = null;
      void closeLanguageDocument(path).catch(() => undefined);
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, language, path, projectName, server?.id]);

  useEffect(() => {
    if (!enabled) return;
    const previous = lastSynced.current;
    if (!previous || previous.path !== path || previous.text === value) return;

    const timer = window.setTimeout(() => {
      const text = valueRef.current;
      const current = lastSynced.current;
      if (!current || current.path !== path || current.text === text) return;
      lastSynced.current = { path, text };
      void syncLanguageDocument({ path, language, text })
        .then(setStatus)
        .catch((cause) => setError(errorToMessage(cause)));
    }, CHANGE_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [enabled, language, path, value]);

  const withPosition = useCallback((position: LanguagePosition = cursorRef.current) => ({
    path,
    text: valueRef.current,
    line: position.line,
    column: position.column,
  }), [path]);

  const runHover = useCallback(async () => {
    if (!enabled || !capabilities?.hover || busy) return;
    setBusy(true);
    setError(null);
    try {
      const hover = await requestLanguageHover(withPosition());
      if (hover) setPanel({ kind: "hover", title: "Hover", hover });
      else setPanel(null);
    } catch (cause) {
      setError(errorToMessage(cause));
    } finally {
      setBusy(false);
    }
  }, [busy, capabilities?.hover, enabled, withPosition]);

  const runDefinitionAt = useCallback(async (position: LanguagePosition) => {
    if (!enabled || !capabilities?.definition || busy) return;
    setBusy(true);
    setError(null);
    try {
      const locations = await requestLanguageDefinition(withPosition(position));
      if (locations.length === 1) {
        const target = locations[0];
        requestCodeWorkspaceOpen(target.path, {
          line: target.line,
          column: target.column,
          endLine: target.end_line,
          endColumn: target.end_column,
        });
        setPanel(null);
      } else if (locations.length > 1) {
        setPanel({ kind: "locations", title: "Definitions", locations });
      } else {
        setPanel(null);
      }
    } catch (cause) {
      setError(errorToMessage(cause));
    } finally {
      setBusy(false);
    }
  }, [busy, capabilities?.definition, enabled, withPosition]);

  const previewAt = useCallback(async (
    position: LanguagePosition,
  ): Promise<LanguageDefinitionPreview | null> => {
    if (!enabled || (!capabilities?.hover && !capabilities?.definition)) return null;
    const request = ++previewRequest.current;
    setError(null);
    try {
      const input = withPosition(position);
      const [hover, definitions] = await Promise.all([
        capabilities?.hover ? requestLanguageHover(input) : Promise.resolve(null),
        capabilities?.definition ? requestLanguageDefinition(input) : Promise.resolve([]),
      ]);
      if (request !== previewRequest.current) return null;
      return { hover, definitions };
    } catch (cause) {
      if (request === previewRequest.current) setError(errorToMessage(cause));
      return null;
    }
  }, [capabilities?.definition, capabilities?.hover, enabled, withPosition]);

  const clearPreview = useCallback(() => {
    previewRequest.current += 1;
  }, []);

  const runDefinition = useCallback(() => runDefinitionAt(cursorRef.current), [runDefinitionAt]);

  const runReferences = useCallback(async () => {
    if (!enabled || !capabilities?.references || busy) return;
    setBusy(true);
    setError(null);
    try {
      const locations = await requestLanguageReferences(withPosition());
      setPanel(locations.length > 0 ? { kind: "locations", title: "References", locations } : null);
    } catch (cause) {
      setError(errorToMessage(cause));
    } finally {
      setBusy(false);
    }
  }, [busy, capabilities?.references, enabled, withPosition]);

  const runSymbols = useCallback(async () => {
    if (!enabled || !capabilities?.document_symbols || busy) return;
    setBusy(true);
    setError(null);
    try {
      const symbols = await requestLanguageDocumentSymbols({ path, text: valueRef.current });
      setPanel(symbols.length > 0 ? { kind: "symbols", title: "Document symbols", symbols } : null);
    } catch (cause) {
      setError(errorToMessage(cause));
    } finally {
      setBusy(false);
    }
  }, [busy, capabilities?.document_symbols, enabled, path]);

  const dismiss = useCallback(() => {
    clearPreview();
    setPanel(null);
    setError(null);
  }, [clearPreview]);

  const handleKeyDown = useCallback((event: LanguageKeyEvent): boolean => {
    if (!enabled) return false;
    const mod = event.metaKey || event.ctrlKey;
    if (event.key === "F12" && event.shiftKey && capabilities?.references) {
      event.preventDefault();
      void runReferences();
      return true;
    }
    if (event.key === "F12" && capabilities?.definition) {
      event.preventDefault();
      void runDefinition();
      return true;
    }
    if (mod && event.shiftKey && event.key.toLowerCase() === "o" && capabilities?.document_symbols) {
      event.preventDefault();
      void runSymbols();
      return true;
    }
    if (event.altKey && event.key.toLowerCase() === "h" && capabilities?.hover) {
      event.preventDefault();
      void runHover();
      return true;
    }
    if (event.key === "Escape" && (panel || error)) {
      event.preventDefault();
      dismiss();
      return true;
    }
    return false;
  }, [capabilities, dismiss, enabled, error, panel, runDefinition, runHover, runReferences, runSymbols]);

  const serviceLabel = server ? languageServiceLabel(language, server) : "LS";

  const statusLabel = !enabled
    ? null
    : error
      ? `${serviceLabel} error`
      : status?.state === "ready"
        ? `${serviceLabel} ready`
        : status?.state === "error"
          ? `${serviceLabel} error`
          : `${serviceLabel} starting`;
  const statusTitle = error
    ?? status?.last_error
    ?? (status?.state === "ready"
      ? `${server?.label ?? "Language server"} PID ${status.pid} · ${status.open_documents} open document${status.open_documents === 1 ? "" : "s"}`
      : enabled ? `Starting ${server?.label ?? "language server"} for the active project` : null);

  const panelNode = panel || error ? (
    <aside className="code-language-panel" aria-label={`${server?.label ?? "Language"} intelligence`}>
      <header>
        <strong>{error ? "Language server" : panel?.title}</strong>
        {busy ? <span>Working…</span> : null}
        <button type="button" onClick={dismiss} aria-label="Close language panel">×</button>
      </header>
      {error ? <div className="code-language-error">{error}</div> : null}
      {panel?.kind === "hover" ? <pre className="code-language-hover">{panel.hover.markdown}</pre> : null}
      {panel?.kind === "locations" ? (
        <div className="code-language-results">
          {panel.locations.map((location, index) => (
            <button
              type="button"
              key={`${location.path}:${location.line}:${location.column}:${index}`}
              onClick={() => {
                requestCodeWorkspaceOpen(location.path, {
                  line: location.line,
                  column: location.column,
                  endLine: location.end_line,
                  endColumn: location.end_column,
                });
                setPanel(null);
              }}
            >
              <code>{locationLabel(location)}</code>
            </button>
          ))}
        </div>
      ) : null}
      {panel?.kind === "symbols" ? (
        <div className="code-language-results">
          {panel.symbols.map((symbol, index) => (
            <button
              type="button"
              key={`${symbol.name}:${symbol.selection_range.start.line}:${index}`}
              onClick={() => {
                requestCodeWorkspaceOpen(path, {
                  line: symbol.selection_range.start.line + 1,
                  column: symbol.selection_range.start.character + 1,
                  endLine: symbol.selection_range.end.line + 1,
                  endColumn: symbol.selection_range.end.character + 1,
                });
                setPanel(null);
              }}
            >
              <strong>{symbol.name}</strong>
              {symbol.detail ? <span>{symbol.detail}</span> : null}
              <code>Ln {symbol.selection_range.start.line + 1}</code>
            </button>
          ))}
        </div>
      ) : null}
    </aside>
  ) : null;

  return {
    status,
    statusLabel,
    statusTitle,
    busy,
    panel: panelNode,
    actions: {
      hover: () => { void runHover(); },
      definition: () => { void runDefinition(); },
      definitionAt: (position) => { void runDefinitionAt(position); },
      previewAt,
      clearPreview,
      references: () => { void runReferences(); },
      symbols: () => { void runSymbols(); },
      dismiss,
    },
    handleKeyDown,
  };
}
