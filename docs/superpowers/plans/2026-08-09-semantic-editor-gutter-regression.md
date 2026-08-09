# Semantic Editor Gutter Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve the approved line-number, gutter-highlight, and right-scrollbar behavior after replacing the textarea editor with CodeMirror.

**Architecture:** Use CodeMirror's `lineNumbers` gutter event contract to move the selection to the clicked `BlockInfo.from`, focus the content surface, and let the existing update listener publish cursor state. Assert CodeMirror's real DOM and scrolling contract instead of retaining selectors for the removed textarea implementation.

**Tech Stack:** React, TypeScript, CodeMirror 6, CSS, Playwright, pnpm

## Global Constraints

- Keep CodeMirror as the semantic editor; do not restore `LightweightCodeEditor`.
- A clicked line number moves the caret to column 1 and focuses the editor.
- The active highlight fills the complete line-number gutter row.
- The only editor scrollbar remains on the right side of the CodeMirror scroller.
- Do not create a git commit unless the user explicitly requests one.

---

### Task 1: Port the editor regression contract to CodeMirror

**Files:**
- Modify: `apps/desktop/e2e/editor-ui.spec.ts`
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/semantic-code-editor.css`
- Generated mechanically: `apps/desktop/pnpm-lock.yaml`

**Interfaces:**
- Consumes: CodeMirror `lineNumbers({ domEventHandlers })`, `BlockInfo.from`, `EditorView.dispatch`, and `EditorView.focus`.
- Produces: CodeMirror gutter click behavior and stable Playwright assertions using `.cm-scroller`, `.cm-content`, `.cm-lineNumbers`, `.cm-gutterElement`, and `.cm-activeLineGutter`.

- [x] **Step 1: Replace stale textarea selectors with CodeMirror behavior assertions**

Update setup to wait for `.semantic-code-editor-host .cm-editor`. Assert the scroller owns overflow, line-number gutter stays horizontally fixed while scrolling, the WebKit scrollbar width is `10px`, and clicking the visible line 13 element results in `Ln 13, Col 1`, focused `.cm-content`, and an active gutter row whose bounds equal `.cm-lineNumbers`.

- [x] **Step 2: Run the focused suite and verify RED**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts`

Expected: the click/status or active-background assertion fails because the semantic editor has no line-number handler and currently makes `.cm-activeLineGutter` transparent.

- [x] **Step 3: Implement the CodeMirror gutter behavior**

Configure:

```ts
lineNumbers({
  domEventHandlers: {
    mousedown(view, line, event) {
      event.preventDefault();
      view.dispatch({ selection: { anchor: line.from } });
      view.focus();
      return true;
    },
  },
})
```

Give `.cm-activeLineGutter` the existing `var(--neutral-soft)` background and line-number elements a pointer cursor plus subtle hover feedback.

- [x] **Step 4: Verify dependencies, build, and all UI tests**

Run:

```bash
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop build
pnpm --dir apps/desktop exec playwright test
git diff --check
```

Expected: frozen install and production build pass, all UI tests pass, and diff check is clean.
