import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import {
  closeLanguageDocument,
  requestLanguageDefinition,
  requestLanguageDocumentSymbols,
  requestLanguageHover,
  requestLanguageReferences,
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

type LanguagePanel =
  | { kind: "hover"; title: string; hover: LanguageHover }
  | { kind: "locations"; title: string; locations: LanguageLocation[] }
  | { kind: "symbols"; title: string; symbols: LanguageSymbol[] };

function locationLabel(location: LanguageLocation): string {
  return `${location.path}:${location.line}:${location.column}`;
}

export function useLiveRustLanguage({
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
  cursor: { line: number; column: number };
}): {
  status: LanguageServerStatus | null;
  statusLabel: string | null;
  statusTitle: string | null;
  busy: boolean;
  panel: ReactNode;
  handleKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => boolean;
} {
  const enabled = language === "rust" && server?.id === "rust-analyzer" && server.availability === "available";
  const [status, setStatus] = useState<LanguageServerStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [panel, setPanel] = useState<LanguagePanel | null>(null);
  const [error, setError] = useState<string | null>(null);
  const lastSynced = useRef<{ path: string; text: string } | null>(null);
  const valueRef = useRef(value);
  const cursorRef = useRef(cursor);
  valueRef.current = value;
  cursorRef.current = cursor;

  useEffect(() => {
    clearLiveLanguageDiagnostics();
    setStatus(null);
  }, [projectName]);

  useEffect(() => {
    if (!projectName) return;
    let disposed = false;
    let unlistenDiagnostics: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;

    void subscribeLanguageDiagnostics((event) => {
      if (event.project !== projectName || disposed) return;
      captureLanguageDiagnosticsEvent(event);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenDiagnostics = unlisten;
    });

    void subscribeLanguageServerStatus((next) => {
      if (next.project === projectName && !disposed) setStatus(next);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenStatus = unlisten;
    });

    return () => {
      disposed = true;
      unlistenDiagnostics?.();
      unlistenStatus?.();
    };
  }, [projectName]);

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
    void syncLanguageDocument({ path, language: "rust", text: initialText })
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
  // Opening a different document owns didOpen/didClose. Text changes are
  // handled by the debounced effect below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, path]);

  useEffect(() => {
    if (!enabled) return;
    const previous = lastSynced.current;
    if (!previous || previous.path !== path || previous.text === value) return;

    const timer = window.setTimeout(() => {
      const text = valueRef.current;
      const current = lastSynced.current;
      if (!current || current.path !== path || current.text === text) return;
      lastSynced.current = { path, text };
      void syncLanguageDocument({ path, language: "rust", text })
        .then(setStatus)
        .catch((cause) => setError(errorToMessage(cause)));
    }, CHANGE_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [enabled, path, value]);

  const withPosition = useCallback(() => ({
    path,
    text: valueRef.current,
    line: cursorRef.current.line,
    column: cursorRef.current.column,
  }), [path]);

  const runHover = useCallback(async () => {
    if (!enabled || busy) return;
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
  }, [busy, enabled, withPosition]);

  const runDefinition = useCallback(async () => {
    if (!enabled || busy) return;
    setBusy(true);
    setError(null);
    try {
      const locations = await requestLanguageDefinition(withPosition());
      if (locations.length === 1) {
        const target = locations[0];
        requestCodeWorkspaceOpen(target.path, { line: target.line, column: target.column });
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
  }, [busy, enabled, withPosition]);

  const runReferences = useCallback(async () => {
    if (!enabled || busy) return;
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
  }, [busy, enabled, withPosition]);

  const runSymbols = useCallback(async () => {
    if (!enabled || busy) return;
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
  }, [busy, enabled, path]);

  const handleKeyDown = useCallback((event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    if (!enabled) return false;
    const mod = event.metaKey || event.ctrlKey;
    if (event.key === "F12" && event.shiftKey) {
      event.preventDefault();
      void runReferences();
      return true;
    }
    if (event.key === "F12") {
      event.preventDefault();
      void runDefinition();
      return true;
    }
    if (mod && event.shiftKey && event.key.toLowerCase() === "o") {
      event.preventDefault();
      void runSymbols();
      return true;
    }
    if (event.altKey && event.key.toLowerCase() === "h") {
      event.preventDefault();
      void runHover();
      return true;
    }
    if (event.key === "Escape" && (panel || error)) {
      event.preventDefault();
      setPanel(null);
      setError(null);
      return true;
    }
    return false;
  }, [enabled, error, panel, runDefinition, runHover, runReferences, runSymbols]);

  const statusLabel = !enabled
    ? null
    : error
      ? "RA error"
      : status?.state === "ready"
        ? "RA ready"
        : status?.state === "error"
          ? "RA error"
          : "RA starting";
  const statusTitle = error
    ?? status?.last_error
    ?? (status?.state === "ready"
      ? `rust-analyzer PID ${status.pid} · ${status.open_documents} open document${status.open_documents === 1 ? "" : "s"}`
      : enabled ? "Starting rust-analyzer for the active project" : null);

  const panelNode = panel || error ? (
    <aside className="code-language-panel" aria-label="Rust language intelligence">
      <header>
        <strong>{error ? "Language server" : panel?.title}</strong>
        {busy ? <span>Working…</span> : null}
        <button type="button" onClick={() => { setPanel(null); setError(null); }} aria-label="Close language panel">×</button>
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
                requestCodeWorkspaceOpen(location.path, { line: location.line, column: location.column });
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
    handleKeyDown,
  };
}
