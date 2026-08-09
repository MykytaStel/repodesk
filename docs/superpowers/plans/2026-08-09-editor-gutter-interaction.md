# Editor Gutter Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the active-line highlight fill the gutter and make every line number navigate the textarea caret to column 1.

**Architecture:** Keep the textarea as the only scroll source and keep the gutter translated from its `scrollTop`. Render one lightweight button per line inside the existing gutter track; a local handler converts the clicked one-based line to a text offset, focuses the textarea, places the caret, and updates cursor state.

**Tech Stack:** React, TypeScript, CSS, Playwright

## Global Constraints

- Keep the current right-side custom scrollbar unchanged.
- The gutter must remain clipped and must not create its own scrollbar.
- Gutter buttons must not enter the Tab sequence.
- Do not create a git commit unless the user explicitly requests one.

---

### Task 1: Full-width gutter selection and line navigation

**Files:**
- Modify: `apps/desktop/src/features/code/LightweightCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/code-editor-polish.css`
- Test: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Consumes: `offsetForLocation(value: string, line: number, column: number): number`, `editorRef`, and `setCursor`.
- Produces: `goToLine(line: number): void` and `.code-editor-line-number` buttons with `data-line`.

- [x] **Step 1: Write the failing interaction and geometry test**

Add a Playwright test that clicks `.code-editor-line-number[data-line="13"]`, then asserts that the textarea is focused, its `selectionStart` equals the start offset of line 13, the status contains `Ln 13, Col 1`, and the active marker spans from the gutter's left edge through its right edge.

- [x] **Step 2: Run the focused test and confirm failure**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts`

Expected: FAIL because `.code-editor-line-number` does not exist and the active marker currently stops before the divider.

- [x] **Step 3: Render clickable line-number rows**

Replace the gutter track text with buttons generated from `lineCount`:

```tsx
{Array.from({ length: lineCount }, (_, index) => {
  const line = index + 1;
  return (
    <button
      key={line}
      type="button"
      tabIndex={-1}
      className="code-editor-line-number"
      data-line={line}
      aria-label={`Go to line ${line}`}
      onClick={() => goToLine(line)}
    >
      {line}
    </button>
  );
})}
```

Implement `goToLine` by calling `offsetForLocation(value, line, 1)`, focusing `editorRef.current`, setting a collapsed selection at that offset, and setting cursor state to `{ line, column: 1 }`.

- [x] **Step 4: Style the buttons and full-width active state**

Make `.code-editor-gutter-track` a vertical grid with one `20px` row per line. Make `.code-editor-line-number` fill the gutter width, right-align its label with the existing inset, inherit the mono typography, and use transparent borders/backgrounds. Restore hover feedback. Set `.code-editor-active-line-number { right: 0; }` so its background reaches the divider while remaining behind the buttons.

- [x] **Step 5: Run focused and full verification**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts
pnpm --dir apps/desktop build
pnpm --dir apps/desktop exec playwright test
git diff --check
```

Expected: editor tests pass, production build succeeds, all UI tests pass, and diff check prints no errors.
