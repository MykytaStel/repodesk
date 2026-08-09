import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type UIEvent } from "react";
import { CODE_OPEN_EVENT, consumeCodeWorkspaceLocation } from "../../shared/api/codeWorkspace";

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
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const gutterRef = useRef<HTMLPreElement>(null);
  const gutterShellRef = useRef<HTMLDivElement>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [cursor, setCursor] = useState({ line: 1, column: 1 });

  const lineCount = useMemo(() => value.length === 0 ? 1 : value.split("\n").length, [value]);
  const lineNumbers = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => String(index + 1)).join("\n"),
    [lineCount],
  );

  const syncGutter = (editor: HTMLTextAreaElement) => {
    const scrollTop = editor.scrollTop;
    const gutter = gutterRef.current;
    if (gutter) {
      // The textarea loses vertical viewport space only when the WebView renders
      // a horizontal scrollbar. Mirror exactly that physical scrollbar height
      // into the non-visible end of the gutter instead of adding fake source
      // padding. Both surfaces can then share the same scrollTop 1:1 from the
      // first line through the absolute end of the file.
      const horizontalScrollbarHeight = Math.max(0, editor.offsetHeight - editor.clientHeight);
      const nextPadding = `${horizontalScrollbarHeight}px`;
      if (gutter.style.paddingBottom !== nextPadding) gutter.style.paddingBottom = nextPadding;
      gutter.scrollTop = scrollTop;
    }
    if (gutterShellRef.current) {
      gutterShellRef.current.style.setProperty("--editor-scroll-top", `${scrollTop}px`);
    }
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
    syncGutter(editor);
    return true;
  };

  useEffect(() => {
    setCursor({ line: 1, column: 1 });
    setFindOpen(false);
    setFindQuery("");
    if (gutterRef.current) gutterRef.current.style.paddingBottom = "0px";
    if (gutterShellRef.current) gutterShellRef.current.style.setProperty("--editor-scroll-top", "0px");

    requestAnimationFrame(() => {
      const editor = editorRef.current;
      if (!editor) return;
      if (applyPendingLocation()) return;
      editor.focus();
      syncGutter(editor);
    });
  // The opened document owns this transition. Location changes for the same
  // path are handled by the event listener below instead of every keystroke.
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
    syncGutter(event.currentTarget);
  };

  const activeLineStyle = {
    top: `calc(${EDITOR_TOP_PADDING_PX}px + ${(cursor.line - 1) * EDITOR_LINE_HEIGHT_PX}px - var(--editor-scroll-top, 0px))`,
  } as CSSProperties;

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

      <div className="code-editor-body">
        <div ref={gutterShellRef} className="code-editor-gutter-shell" aria-hidden="true">
          <pre ref={gutterRef} className="code-editor-gutter">{lineNumbers}</pre>
          <span className="code-editor-active-line-number" style={activeLineStyle}>{cursor.line}</span>
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
      </div>

      <footer className="code-editor-status">
        <span>Ln {cursor.line}, Col {cursor.column}</span>
        <span>{language}</span>
        <span>UTF-8</span>
        <span>{lineCount} lines</span>
        <span>{dirty ? `${value.length.toLocaleString()} chars` : `${bytes.toLocaleString()} bytes`}</span>
        {dirty ? <strong>Unsaved</strong> : <span>Saved</span>}
      </footer>
    </section>
  );
}
