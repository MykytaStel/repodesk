import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { rust } from "@codemirror/lang-rust";
import {
  HighlightStyle,
  bracketMatching,
  defaultHighlightStyle,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import { lintGutter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { EditorState, RangeSet, type Extension } from "@codemirror/state";
import {
  EditorView,
  GutterMarker,
  drawSelection,
  gutter,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { CODE_OPEN_EVENT, consumeCodeWorkspaceLocation, type CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";
import {
  LANGUAGE_INTELLIGENCE_KEY,
  languageIntelligenceSnapshot,
  languageServerFor,
} from "../../shared/api/languageIntelligence";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useLiveRustLanguage } from "./useLiveRustLanguage";
import { useSemanticCodeState, type GitLineKind, type SemanticFileState } from "./useSemanticCodeState";
import "./semantic-code-editor.css";

function languageExtension(language: string): Extension {
  if (language === "rust") return rust();
  if (language === "typescript") return javascript({ typescript: true, jsx: true });
  if (language === "javascript") return javascript({ jsx: true });
  if (language === "json") return json();
  return [];
}

const repodeskHighlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.controlKeyword], color: "var(--syntax-keyword)", fontWeight: "600" },
  { tag: [tags.typeName, tags.className], color: "var(--syntax-type)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: "var(--syntax-function)" },
  { tag: tags.variableName, color: "var(--syntax-variable)" },
  { tag: tags.propertyName, color: "var(--syntax-property)" },
  { tag: [tags.string, tags.special(tags.string)], color: "var(--syntax-string)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--syntax-number)" },
  { tag: tags.comment, color: "var(--syntax-comment)", fontStyle: "italic" },
  { tag: tags.operator, color: "var(--syntax-operator)" },
  { tag: [tags.punctuation, tags.bracket], color: "var(--syntax-punctuation)" },
  { tag: tags.meta, color: "var(--syntax-meta)" },
  { tag: tags.invalid, color: "var(--danger)", textDecoration: "underline wavy" },
]);

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--text)",
    backgroundColor: "var(--bg)",
    fontSize: "12.5px",
  },
  ".cm-scroller": {
    overflow: "auto",
    fontFamily: "var(--font-mono)",
    lineHeight: "20px",
  },
  ".cm-content": {
    padding: "12px 0 0",
    caretColor: "var(--accent-2)",
  },
  ".cm-line": {
    padding: "0 18px 0 10px",
  },
  ".cm-gutters": {
    backgroundColor: "var(--surface-2)",
    color: "var(--muted-2)",
    borderRight: "1px solid var(--border)",
  },
  ".cm-lineNumbers .cm-gutterElement": {
    minWidth: "44px",
    padding: "0 10px 0 6px",
    textAlign: "right",
    fontVariantNumeric: "tabular-nums",
  },
  ".cm-activeLineGutter": {
    color: "var(--text)",
    backgroundColor: "transparent",
  },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in srgb, var(--accent-soft) 22%, transparent)",
  },
  "&.cm-focused": { outline: "none" },
  "&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent-2)" },
  "&.cm-focused .cm-selectionBackground, ::selection": { backgroundColor: "var(--accent-soft)" },
  ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--warning) 22%, transparent)" },
  ".cm-panels": { backgroundColor: "var(--surface)", color: "var(--text)" },
  ".cm-panel.cm-search": { padding: "6px 8px", borderBottom: "1px solid var(--border)" },
  ".cm-panel input": {
    backgroundColor: "var(--bg)",
    color: "var(--text)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border-strong)",
  },
}, { dark: true });

class GitMarker extends GutterMarker {
  constructor(readonly kind: GitLineKind) {
    super();
  }

  toDOM() {
    const marker = document.createElement("span");
    marker.className = `cm-git-line-marker ${this.kind}`;
    marker.title = this.kind === "added" ? "Added in Git" : this.kind === "modified" ? "Modified in Git" : "Deleted near this line";
    return marker;
  }
}

const GIT_MARKERS: Record<GitLineKind, GitMarker> = {
  added: new GitMarker("added"),
  modified: new GitMarker("modified"),
  deleted: new GitMarker("deleted"),
};

function gitGutterExtension(semantic: SemanticFileState): Extension {
  return gutter({
    class: "cm-git-gutter",
    markers(view) {
      const ranges = semantic.gitLines.flatMap((item) => {
        if (item.line < 1 || item.line > view.state.doc.lines) return [];
        return [GIT_MARKERS[item.kind].range(view.state.doc.line(item.line).from)];
      });
      return RangeSet.of(ranges, true);
    },
  });
}

function diagnosticOffset(state: EditorState, line: number | null, column: number | null): number {
  const safeLine = Math.max(1, Math.min(line ?? 1, state.doc.lines));
  const textLine = state.doc.line(safeLine);
  return Math.min(textLine.to, textLine.from + Math.max(0, (column ?? 1) - 1));
}

function codeMirrorDiagnostics(state: EditorState, semantic: SemanticFileState): Diagnostic[] {
  return semantic.problems.map((problem) => {
    const from = diagnosticOffset(state, problem.line, problem.column);
    const line = state.doc.lineAt(from);
    const to = Math.min(line.to, from + (from < line.to ? 1 : 0));
    return {
      from,
      to,
      severity: problem.severity === "info" ? "info" : problem.severity,
      message: problem.message,
      source: problem.command || problem.source,
    };
  });
}

function cursorForState(state: EditorState): { line: number; column: number } {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  return { line: line.number, column: head - line.from + 1 };
}

function offsetForLocation(state: EditorState, line: number, column: number): number {
  const safeLine = Math.max(1, Math.min(line, state.doc.lines));
  const target = state.doc.line(safeLine);
  return Math.min(target.to, target.from + Math.max(0, column - 1));
}

function SemanticStrip({ semantic, dirty }: { semantic: SemanticFileState; dirty: boolean }) {
  const visible = Boolean(
    semantic.workItemId
    || semantic.scopeState
    || semantic.errors
    || semantic.warnings
    || semantic.gitLines.length
    || dirty,
  );
  if (!visible) return null;

  const scopeLabel = semantic.scopeState === "allowed"
    ? "Scope allowed"
    : semantic.scopeState === "out_of_scope"
      ? "Out of scope"
      : semantic.scopeState === "protected"
        ? "Protected path"
        : semantic.scopeState === "ungoverned"
          ? "Ungoverned change"
          : null;
  const verificationLabel = dirty && semantic.verificationState === "passed"
    ? "Draft after verification"
    : semantic.verificationState === "passed"
      ? "Verified"
      : semantic.verificationState === "failed"
        ? "Verification failed"
        : semantic.verificationState === "running"
          ? "Verifying"
          : semantic.verificationState === "not_run"
            ? "Not verified"
            : null;

  return (
    <div className={`semantic-code-strip scope-${semantic.scopeState ?? "none"}`}>
      {semantic.workItemId ? <span title="Current Work Item"><strong>Work</strong> {semantic.workItemId}</span> : null}
      {scopeLabel ? <span className={semantic.scopeState === "allowed" ? "ok" : "danger"}>{scopeLabel}</span> : null}
      {semantic.reviewState && semantic.reviewState !== "proposed" ? <span>Review {semantic.reviewState}</span> : null}
      {verificationLabel ? <span className={dirty && semantic.verificationState === "passed" ? "warn" : semantic.verificationState === "passed" ? "ok" : ""}>{verificationLabel}</span> : null}
      {semantic.origin !== "unknown" ? (
        <span title="Origin describes the governed ChangeSet, not individual lines">
          ChangeSet {semantic.origin}{semantic.originLabel ? ` · ${semantic.originLabel}` : ""}
        </span>
      ) : null}
      {semantic.errors > 0 ? <span className="danger">{semantic.errors} error{semantic.errors === 1 ? "" : "s"}</span> : null}
      {semantic.warnings > 0 ? <span className="warn">{semantic.warnings} warning{semantic.warnings === 1 ? "" : "s"}</span> : null}
    </div>
  );
}

export function SemanticCodeEditor({
  path,
  value,
  dirty,
  language,
  bytes,
  status,
  saving,
  onChange,
  onSave,
}: {
  path: string;
  value: string;
  dirty: boolean;
  language: string;
  bytes: number;
  status: CodeWorkspaceFileStatus;
  saving: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}) {
  const { hasProject, projectName } = useWorkspace();
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onSaveRef = useRef(onSave);
  const liveActionsRef = useRef<ReturnType<typeof useLiveRustLanguage>["actions"] | null>(null);
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  onChangeRef.current = onChange;
  onSaveRef.current = onSave;

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
  const liveLanguage = useLiveRustLanguage({
    path,
    value,
    language,
    projectName,
    server: languageServer,
    cursor,
  });
  liveActionsRef.current = liveLanguage.actions;

  const semantic = useSemanticCodeState({ projectName, path, status, dirty });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        highlightSpecialChars(),
        history(),
        gitGutterExtension(semantic),
        lineNumbers(),
        highlightActiveLineGutter(),
        drawSelection(),
        indentOnInput(),
        bracketMatching(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        lintGutter(),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        syntaxHighlighting(repodeskHighlight),
        languageExtension(language),
        editorTheme,
        EditorState.tabSize.of(2),
        keymap.of([
          {
            key: "Mod-s",
            preventDefault: true,
            run: () => {
              if (dirty && !saving) onSaveRef.current();
              return true;
            },
          },
          { key: "F12", run: () => { liveActionsRef.current?.definition(); return true; } },
          { key: "Shift-F12", run: () => { liveActionsRef.current?.references(); return true; } },
          { key: "Mod-Shift-o", run: () => { liveActionsRef.current?.symbols(); return true; } },
          { key: "Alt-h", run: () => { liveActionsRef.current?.hover(); return true; } },
          indentWithTab,
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) onChangeRef.current(update.state.doc.toString());
          if (update.selectionSet || update.docChanged) setCursor(cursorForState(update.state));
        }),
      ],
    });
    const view = new EditorView({ state, parent: host });
    viewRef.current = view;
    view.dispatch(setDiagnostics(view.state, codeMirrorDiagnostics(view.state, semantic)));

    const applyPendingLocation = () => {
      const location = consumeCodeWorkspaceLocation(path);
      if (!location) return;
      const offset = offsetForLocation(view.state, location.line, location.column);
      view.dispatch({
        selection: { anchor: offset },
        effects: EditorView.scrollIntoView(offset, { y: "center" }),
      });
      view.focus();
    };
    requestAnimationFrame(applyPendingLocation);
    window.addEventListener(CODE_OPEN_EVENT, applyPendingLocation);

    return () => {
      window.removeEventListener(CODE_OPEN_EVENT, applyPendingLocation);
      viewRef.current = null;
      view.destroy();
    };
  // A document/language transition gets a clean CodeMirror state and history.
  // Semantic state and external content are updated by dedicated effects below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [language, path]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    view.dispatch({
      changes: { from: 0, to: current.length, insert: value },
    });
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch(setDiagnostics(view.state, codeMirrorDiagnostics(view.state, semantic)));
  }, [semantic.problems]);

  // Git line markers and the engineering strip are advisory projections. Rebuild
  // the editor configuration only when those line markers materially change.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    // Gutter projection is intentionally hidden while there is an unsaved draft;
    // its line numbers belong to the on-disk Git delta, not the draft buffer.
    const gitOnly = gitGutterExtension({ ...semantic, gitLines: dirty ? [] : semantic.gitLines });
    view.dispatch({ effects: EditorState.reconfigure.of([
      highlightSpecialChars(),
      history(),
      gitOnly,
      lineNumbers(),
      highlightActiveLineGutter(),
      drawSelection(),
      indentOnInput(),
      bracketMatching(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      lintGutter(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      syntaxHighlighting(repodeskHighlight),
      languageExtension(language),
      editorTheme,
      EditorState.tabSize.of(2),
      keymap.of([
        { key: "Mod-s", preventDefault: true, run: () => { if (dirty && !saving) onSaveRef.current(); return true; } },
        { key: "F12", run: () => { liveActionsRef.current?.definition(); return true; } },
        { key: "Shift-F12", run: () => { liveActionsRef.current?.references(); return true; } },
        { key: "Mod-Shift-o", run: () => { liveActionsRef.current?.symbols(); return true; } },
        { key: "Alt-h", run: () => { liveActionsRef.current?.hover(); return true; } },
        indentWithTab,
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
      ]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) onChangeRef.current(update.state.doc.toString());
        if (update.selectionSet || update.docChanged) setCursor(cursorForState(update.state));
      }),
    ]) });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dirty, semantic.gitLines]);

  const lineCount = value.length === 0 ? 1 : value.split("\n").length;
  const languageStatus = languageServer
    ? language === "rust" && languageServer.availability === "available"
      ? liveLanguage.statusLabel ?? "RA starting"
      : `LS ${languageServer.label} ${languageServer.availability === "available" ? "found" : "missing"}`
    : null;
  const languageStatusTitle = liveLanguage.statusTitle
    ?? (languageServer?.availability === "available"
      ? `${languageServer.label} discovered${languageServer.source === "project_local" ? " in this project" : " on PATH"}.`
      : languageServer ? `${languageServer.label} is supported but was not found.` : null);

  return (
    <section className={`semantic-code-editor scope-${semantic.scopeState ?? "none"}`} aria-label={`Editor for ${path}`}>
      <SemanticStrip semantic={semantic} dirty={dirty} />
      {liveLanguage.panel}
      <div ref={hostRef} className="semantic-code-editor-host" />
      <footer className="code-editor-status semantic-status">
        <span>Ln {cursor.line}, Col {cursor.column}</span>
        <span>{language}</span>
        {languageStatus ? (
          <span
            className={`code-language-service ${liveLanguage.status?.state ?? languageServer?.availability ?? "missing"}`}
            title={languageStatusTitle ?? undefined}
          >{languageStatus}</span>
        ) : null}
        {semantic.gitLines.length > 0 && !dirty ? <span>{semantic.gitLines.length} Git line{semantic.gitLines.length === 1 ? "" : "s"}</span> : null}
        <span>UTF-8</span>
        <span>{lineCount} lines</span>
        <span>{dirty ? `${value.length.toLocaleString()} chars` : `${bytes.toLocaleString()} bytes`}</span>
        {dirty ? <strong>Unsaved</strong> : <span>Saved</span>}
      </footer>
    </section>
  );
}
