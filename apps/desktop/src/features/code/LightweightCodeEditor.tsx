import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type UIEvent } from "react";

function lineAndColumn(value: string, offset: number): { line: number; column: number } {
  const safeOffset = Math.max(0, Math.min(offset, value.length));
  const before = value.slice(0, safeOffset);
  const lastNewline = before.lastIndexOf("\n");
  return {
    line: before.split("\n").length,
    column: safeOffset - lastNewline,
  };
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
  const findRef = useRef<HTMLInputElement>(null);
  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [cursor, setCursor] = useState({ line: 1, column: 1 });

  const lineCount = useMemo(() => value.length === 0 ? 1 : value.split("\n").length, [value]);
  const lineNumbers = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => String(index + 1)).join("\n"),
    [lineCount],
  );

  useEffect(() => {
    setCursor({ line: 1, column: 1 });
    setFindOpen(false);
    setFindQuery("");
    requestAnimationFrame(() => editorRef.current?.focus());
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
    if (gutterRef.current) gutterRef.current.scrollTop = event.currentTarget.scrollTop;
  };

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
        <pre ref={gutterRef} className="code-editor-gutter" aria-hidden="true">{lineNumbers}</pre>
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
