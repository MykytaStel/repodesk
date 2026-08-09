import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type UIEvent } from "react";
import { consumeCodeWorkspaceLocation } from "../../shared/api/codeWorkspace";

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
    const editorMax = Math.max(0, editor.scrollHeight - editor.clientHeight);
    const gutter = gutterRef.current;
    if (gutter) {
      const gutterMax = Math.max(0, gutter.scrollHeight - gutter.clientHeight);
      const progress = editorMax > 0 ? Math.min(1, Math.max(0, scrollTop / editorMax)) : 0;
      gutter.scrollTop = progress * gutterMax;
    }
    if (gutterShellRef.current) {
      gutterShellRef.current.style.setProperty("--editor-scroll-top", `${scrollTop}px`);
    }
  };

  useEffect(() => {
    setCursor({ line: 1, column: 1 });
    setFindOpen(false);
    setFindQuery("");
    if (gutterShellRef.current) gutterShellRef.current.style.setProperty("--editor-scroll-top", "0px");

    requestAnimationFrame(() => {
      const editor = editorRef.current;
      if (!editor) return;
      const location = consumeCodeWorkspaceLocation(path);
      if (!location) {
        editor.focus();
        syncGutter(editor);
        return;
      }

      const offset = offsetForLocation(value, location.line, location.column);
      editor.focus();
      editor.setSelectionRange(offset, offset);
      setCursor(lineAndColumn(value, offset));
      const targetTop = EDITOR_TOP_PADDING_PX + (location.line - 1) * EDITOR_LINE_HEIGHT_PX;
      editor.scrollTop = Math.max(0, targetTop - editor.clientHeight * 0.32);
      syncGutter(editor);
    });
  // `value` belongs to the same opened document; a location is consumed only on
  // a path transition/request, not every keystroke.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

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
