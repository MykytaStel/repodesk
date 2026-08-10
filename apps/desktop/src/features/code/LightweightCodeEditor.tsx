import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type UIEvent } from "react";
import { CODE_OPEN_EVENT, consumeCodeWorkspaceLocation } from "../../shared/api/codeWorkspace";
import {
  LANGUAGE_INTELLIGENCE_KEY,
  languageIntelligenceSnapshot,
  languageServerFor,
} from "../../shared/api/languageIntelligence";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useLiveLanguage } from "./useLiveLanguage";

const EDITOR_LINE_HEIGHT_PX = 20;
const EDITOR_TOP_PADDING_PX = 12;

function lineAndColumn(value: string, offset: number): { line: number; column: number } {
  const safeOffset = Math.max(0, Math.min(offset, value.length));
  const before = value.slice(0, safeOffset);
  const lastNewline = before.lastIndexOf("\n");
  return {
    line: before.split("\n").length,
    column: safeOffset - lastNewline,
  };
}

function offsetForLocation(value: string, line: number, column: number): number {
  const targetLine = Math.max(1, line);
  const targetColumn = Math.max(1, column);
  let offset = 0;
  let currentLine = 1;
  while (currentLine < targetLine && offset < value.length) {
    const newline = value.indexOf("\n", offset);
    if (newline < 0) return value.length;
    offset = newline + 1;
    currentLine += 1;
  }
  const lineEnd = value.indexOf("\n", offset);
  const maxOffset = lineEnd < 0 ? value.length : lineEnd;
  return Math.min(maxOffset, offset + targetColumn - 1);
}

export function LightweightCodeEditor({
  path,
  value,
  dirty,
  language,
  bytes,
  saving,
  onChange,
  onSave,
}: {
  path: string;
  value: string;
  dirty: boolean;
  language: string;
  bytes: number;
  saving: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}) {
  const { hasProject, projectName } = useWorkspace();
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const gutterShellRef = useRef<HTMLDivElement>(null);
  const scrollbarRef = useRef<HTMLDivElement>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [cursor, setCursor] = useState({ line: 1, column: 1 });

  const languageIntelligence = useQuery({
    queryKey: [...LANGUAGE_INTELLIGENCE_KEY, projectName ?? "none"],
    queryFn: languageIntelligenceSnapshot,
    enabled: hasProject,
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  });
  const languageServer = useMemo(
    () => languageServerFor(languageIntelligence.data, language),
    [language, languageIntelligence.data],
  );
  const liveLanguage = useLiveLanguage({
    path,
    value,
    language,
    projectName,
    server: languageServer,
    cursor,
  });

  const lineCount = useMemo(() => value.length === 0 ? 1 : value.split("\n").length, [value]);
  const lineNumbers = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => index + 1),
    [lineCount],
  );

  const syncScrollGeometry = (editor: HTMLTextAreaElement) => {
    // The textarea is now the only scroll surface. The gutter is a clipped,
    // non-scrollable visual layer translated by the textarea's exact scrollTop.
    // That removes the independent scrollHeight/clamping behaviour that caused
    // line numbers to stop before the document reached its absolute end.
    gutterShellRef.current?.style.setProperty("--editor-scroll-top", `${editor.scrollTop}px`);
    const scrollbar = scrollbarRef.current;
    if (!scrollbar) return;
    const trackHeight = scrollbar.clientHeight || editor.clientHeight;
    const maxScroll = Math.max(0, editor.scrollHeight - editor.clientHeight);
    const thumbHeight = maxScroll === 0
      ? trackHeight
      : Math.max(28, trackHeight * editor.clientHeight / editor.scrollHeight);
    const travel = Math.max(0, trackHeight - thumbHeight);
    const thumbTop = maxScroll === 0 ? 0 : travel * editor.scrollTop / maxScroll;
    scrollbar.style.setProperty("--editor-thumb-height", `${thumbHeight}px`);
    scrollbar.style.setProperty("--editor-thumb-top", `${thumbTop}px`);
    scrollbar.toggleAttribute("data-scrollable", maxScroll > 0);
  };

  const applyPendingLocation = (): boolean => {
    const editor = editorRef.current;
    if (!editor) return false;
    const location = consumeCodeWorkspaceLocation(path);
    if (!location) return false;

    const offset = offsetForLocation(value, location.line, location.column);
    editor.focus();
    editor.setSelectionRange(offset, offset);
    setCursor(lineAndColumn(value, offset));
    const targetTop = EDITOR_TOP_PADDING_PX + (location.line - 1) * EDITOR_LINE_HEIGHT_PX;
    editor.scrollTop = Math.max(0, targetTop - editor.clientHeight * 0.32);
    syncScrollGeometry(editor);
    return true;
  };

  useEffect(() => {
    setCursor({ line: 1, column: 1 });
    setFindOpen(false);
    setFindQuery("");
    gutterShellRef.current?.style.setProperty("--editor-scroll-top", "0px");

    requestAnimationFrame(() => {
      const editor = editorRef.current;
      if (!editor) return;
      if (applyPendingLocation()) return;
      editor.focus();
      syncScrollGeometry(editor);
    });
  // The opened document owns this transition. Location changes for the same
  // path are handled by the event listener below instead of every keystroke.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      if (editorRef.current) syncScrollGeometry(editorRef.current);
    });
    return () => cancelAnimationFrame(frame);
  }, [value]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => syncScrollGeometry(editor));
    observer.observe(editor);
    return () => observer.disconnect();
  // The observer must follow the newly mounted editor when the document changes.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  useEffect(() => {
    const onOpenCode = () => {
      requestAnimationFrame(() => {
        void applyPendingLocation();
      });
    };
    window.addEventListener(CODE_OPEN_EVENT, onOpenCode);
    return () => window.removeEventListener(CODE_OPEN_EVENT, onOpenCode);
  // Rebind when source changes so a location uses the current in-memory text.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, value]);

  const updateCursor = () => {
    const editor = editorRef.current;
    if (!editor) return;
    setCursor(lineAndColumn(value, editor.selectionStart));
  };

  const goToLine = (line: number) => {
    const editor = editorRef.current;
    if (!editor) return;
    const offset = offsetForLocation(value, line, 1);
    editor.focus();
    editor.setSelectionRange(offset, offset);
    setCursor({ line, column: 1 });
  };

  const findNext = (reverse = false) => {
    const editor = editorRef.current;
    const query = findQuery;
    if (!editor || !query) return;

    const source = value.toLowerCase();
    const needle = query.toLowerCase();
    const start = reverse ? editor.selectionStart - 1 : editor.selectionEnd;
    let index = reverse
      ? source.lastIndexOf(needle, Math.max(0, start))
      : source.indexOf(needle, Math.max(0, start));
    if (index < 0) index = reverse ? source.lastIndexOf(needle) : source.indexOf(needle);
    if (index < 0) return;

    editor.focus();
    editor.setSelectionRange(index, index + query.length);
    setCursor(lineAndColumn(value, index));
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (liveLanguage.handleKeyDown(event)) return;
    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key.toLowerCase() === "s") {
      event.preventDefault();
      if (dirty && !saving) onSave();
      return;
    }
    if (mod && event.key.toLowerCase() === "f") {
      event.preventDefault();
      setFindOpen(true);
      requestAnimationFrame(() => findRef.current?.focus());
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      const editor = event.currentTarget;
      const start = editor.selectionStart;
      const end = editor.selectionEnd;
      const next = `${value.slice(0, start)}  ${value.slice(end)}`;
      onChange(next);
      requestAnimationFrame(() => {
        editor.selectionStart = editor.selectionEnd = start + 2;
        setCursor(lineAndColumn(next, start + 2));
      });
    }
  };

  const handleScroll = (event: UIEvent<HTMLTextAreaElement>) => {
    syncScrollGeometry(event.currentTarget);
  };

  const handleScrollbarPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    const editor = editorRef.current;
    const track = scrollbarRef.current;
    if (!editor || !track || !track.hasAttribute("data-scrollable")) return;
    event.preventDefault();
    track.setPointerCapture(event.pointerId);

    const rect = track.getBoundingClientRect();
    const thumb = track.querySelector<HTMLElement>(".code-editor-scrollbar-thumb");
    const thumbHeight = thumb?.getBoundingClientRect().height ?? 28;
    const maxScroll = Math.max(0, editor.scrollHeight - editor.clientHeight);
    const travel = Math.max(1, rect.height - thumbHeight);
    const startY = event.clientY;
    const startScroll = editor.scrollTop;
    const clickedThumb = event.target === thumb;

    if (!clickedThumb) {
      editor.scrollTop = Math.max(0, Math.min(maxScroll, ((event.clientY - rect.top - thumbHeight / 2) / travel) * maxScroll));
      syncScrollGeometry(editor);
    }

    const move = (moveEvent: PointerEvent) => {
      if (!clickedThumb) return;
      editor.scrollTop = Math.max(0, Math.min(maxScroll, startScroll + ((moveEvent.clientY - startY) / travel) * maxScroll));
      syncScrollGeometry(editor);
    };
    const finish = () => {
      track.removeEventListener("pointermove", move);
      track.removeEventListener("pointerup", finish);
      track.removeEventListener("pointercancel", finish);
    };
    track.addEventListener("pointermove", move);
    track.addEventListener("pointerup", finish);
    track.addEventListener("pointercancel", finish);
  };

  const activeLineStyle = {
    top: `calc(${EDITOR_TOP_PADDING_PX}px + ${(cursor.line - 1) * EDITOR_LINE_HEIGHT_PX}px - var(--editor-scroll-top, 0px))`,
  } as CSSProperties;

  const languageStatus = languageServer
    ? languageServer.profile_state === "active" && languageServer.availability === "available"
      ? liveLanguage.statusLabel ?? "LS starting"
      : `LS ${languageServer.label} ${languageServer.availability === "available" ? "found" : "missing"}`
    : null;
  const languageStatusTitle = liveLanguage.statusTitle
    ?? (languageServer?.availability === "available"
      ? `${languageServer.label} discovered${languageServer.source === "project_local" ? " in this project" : " on PATH"}.`
      : languageServer ? `${languageServer.label} is supported but was not found.` : null);

  return (
    <section className="code-editor-shell" aria-label={`Editor for ${path}`}>
      {findOpen ? (
        <div className="code-find-bar">
          <input
            ref={findRef}
            value={findQuery}
            onChange={(event) => setFindQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") findNext(event.shiftKey);
              if (event.key === "Escape") {
                setFindOpen(false);
                editorRef.current?.focus();
              }
            }}
            placeholder="Find in file"
            aria-label="Find in file"
          />
          <button type="button" onClick={() => findNext(true)} title="Previous match">↑</button>
          <button type="button" onClick={() => findNext(false)} title="Next match">↓</button>
          <button type="button" onClick={() => setFindOpen(false)} title="Close find">×</button>
        </div>
      ) : null}

      {liveLanguage.panel}

      <div className="code-editor-body">
        <div ref={gutterShellRef} className="code-editor-gutter-shell" aria-label="Line numbers">
          <pre className="code-editor-gutter-track">
            {lineNumbers.map((line) => (
              <button
                key={line}
                type="button"
                tabIndex={-1}
                className="code-editor-line-number"
                data-line={line}
                aria-label={`Go to line ${line}`}
                aria-current={cursor.line === line ? "true" : undefined}
                onClick={() => goToLine(line)}
              >
                {line}
              </button>
            ))}
          </pre>
          <span className="code-editor-active-line-number" style={activeLineStyle} />
        </div>
        <textarea
          ref={editorRef}
          className="code-editor-input"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={handleKeyDown}
          onKeyUp={updateCursor}
          onClick={updateCursor}
          onSelect={updateCursor}
          onScroll={handleScroll}
          wrap="off"
          spellCheck={false}
          aria-label={`Editing ${path}`}
        />
        <div
          ref={scrollbarRef}
          className="code-editor-scrollbar"
          aria-hidden="true"
          onPointerDown={handleScrollbarPointerDown}
        >
          <span className="code-editor-scrollbar-thumb" />
        </div>
      </div>

      <footer className="code-editor-status">
        <span>Ln {cursor.line}, Col {cursor.column}</span>
        <span>{language}</span>
        {languageStatus ? (
          <span
            className={`code-language-service ${liveLanguage.status?.state ?? languageServer?.availability ?? "missing"}`}
            title={languageStatusTitle ?? undefined}
          >
            {languageStatus}
          </span>
        ) : null}
        <span>UTF-8</span>
        <span>{lineCount} lines</span>
        <span>{dirty ? `${value.length.toLocaleString()} chars` : `${bytes.toLocaleString()} bytes`}</span>
        {dirty ? <strong>Unsaved</strong> : <span>Saved</span>}
      </footer>
    </section>
  );
}
