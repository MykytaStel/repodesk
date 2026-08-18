# Workbench Interaction Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish one canonical workbench interaction contract for Navigator, Inspector, and Bottom Panel before migrating route-specific panels.

**Architecture:** Keep the existing shell layout and persistence compatible, but replace product-level `sidebar/drawer` semantics with canonical Navigator/Inspector terminology. Introduce a reusable Inspector surface primitive that owns close affordance, Escape dismissal, and focus restoration, then lock the behavior with E2E and architecture ratchets. Route-specific Work/Code/Changes migrations remain separate follow-up PRs.

**Tech Stack:** React 18, TypeScript, Playwright, CSS, Node architecture-ratchet script, Tauri desktop shell.

**Spec:** `docs/superpowers/specs/2026-08-18-workbench-interaction-contract-design.md`

## Global Constraints

- Preserve `Work → Code → Changes → Runs → Projects` as the five engineering destinations.
- Preserve existing local-storage key names for backward compatibility in this slice.
- `Cmd/Ctrl+B` toggles Navigator.
- `Cmd/Ctrl+J` toggles Bottom Panel; global `Escape` must not close Bottom Panel.
- Inspector closes via explicit close control and `Escape` when no modal dialog owns `Escape`.
- Inspector close restores focus to its opener when possible.
- No new `*-polish.css`, `-vN.css`, or versioned visual-generation classes.
- No new product behavior beyond workbench interaction semantics and structured error normalization.
- Existing 28 KiB frontend source-file hard limit/ratchet remains binding.
- User-visible structured errors in migrated workbench code must use the canonical error normalizer rather than `String(error)`.

---

### Task 1: Lock the workbench interaction contract with failing E2E tests

**Files:**
- Create: `apps/desktop/e2e/workbench-interaction-contract.spec.ts`
- Modify: none

**Interfaces:**
- Consumes: Activity Rail accessible button names and existing keyboard handlers in `App.tsx`.
- Produces: executable behavior contract for Navigator, Inspector, and Bottom Panel.

- [ ] **Step 1: Write failing tests**

Create tests that boot the current desktop fixture and assert:

```ts
import { expect, test } from "@playwright/test";
import { bootCurrentWorkspace } from "./current-fixtures";

test.beforeEach(async ({ page }) => {
  await bootCurrentWorkspace(page);
});

test("uses Navigator terminology and Cmd/Ctrl+B toggles the structural left pane", async ({ page }) => {
  const toggle = page.getByRole("button", { name: /Navigator.*Ctrl\+B/i });
  await expect(toggle).toBeVisible();
  await toggle.click();
  await expect(page.getByRole("complementary", { name: "Workspace navigator" })).toBeVisible();
  await page.keyboard.press(process.platform === "darwin" ? "Meta+B" : "Control+B");
  await expect(page.getByRole("complementary", { name: "Workspace navigator" })).toHaveCount(0);
});

test("Inspector exposes local close and Escape restores focus to its opener", async ({ page }) => {
  const opener = page.getByRole("button", { name: /Show inspector/i });
  await opener.focus();
  await opener.click();
  const inspector = page.getByRole("complementary", { name: "Engineering evidence inspector" });
  await expect(inspector).toBeVisible();
  await expect(inspector.getByRole("button", { name: "Close inspector" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(inspector).toHaveCount(0);
  await expect(opener).toBeFocused();
});

test("Escape does not close the Bottom Panel", async ({ page }) => {
  const toggle = page.getByRole("button", { name: /bottom panel.*Ctrl\+J/i });
  await toggle.click();
  const panel = page.getByRole("region", { name: "Workbench bottom panel" });
  await expect(panel).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(panel).toBeVisible();
});
```

Adapt fixture setup and platform modifier helper to the existing E2E conventions if their exported names differ, while preserving the exact behaviors above.

- [ ] **Step 2: Push only the test commit and verify RED in GitHub Actions**

Expected: Native E2E or CI Playwright job fails because current Activity Rail says `workspace sidebar`, WorkspaceSidebar lacks the canonical accessible name, and shell Inspector lacks local close/Escape/focus restoration.

- [ ] **Step 3: Record the exact failing assertions in the PR description/implementation ledger**

No production files are modified before this RED evidence exists.

### Task 2: Add the canonical Inspector surface primitive

**Files:**
- Create: `apps/desktop/src/app/WorkbenchInspectorSurface.tsx`
- Modify: `apps/desktop/src/app/WorkspaceInspector.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: existing canonical shell CSS owner that styles `.workspace-inspector`

**Interfaces:**
- Consumes: `ReactNode`, `onClose(): void`, optional opener ref supplied by App.
- Produces:
  - `WorkbenchInspectorSurface({ ariaLabel, title, eyebrow, description, onClose, children })`
  - stable close button with `aria-label="Close inspector"`.

- [ ] **Step 1: Implement `WorkbenchInspectorSurface`**

The component renders an `aside`/complementary surface with a standard header and explicit close control. It does not own route state.

- [ ] **Step 2: Move shell WorkspaceInspector content inside the primitive**

`WorkspaceInspector` must accept `onClose` and stop being responsible for outer workbench chrome. Preserve all evidence content and footer actions.

- [ ] **Step 3: Add shell-level Escape handling and focus restoration**

In `App.tsx`:

```ts
const inspectorOpenerRef = useRef<HTMLButtonElement | null>(null);

const closeInspector = useCallback(() => {
  setInspectorOpen(false);
  queueMicrotask(() => inspectorOpenerRef.current?.isConnected && inspectorOpenerRef.current.focus());
}, []);
```

The global keydown handler must close Inspector on plain `Escape` only when no `[role="dialog"][aria-modal="true"]` exists. It must not close Bottom Panel.

Pass the Activity Rail inspector button ref through a focused, typed prop rather than querying the DOM by selector.

- [ ] **Step 4: Normalize the existing WorkspaceInspector structured error**

Replace direct `String(snapshot.error)` with canonical `normalizeError(snapshot.error).message` (or the existing approved shared helper if the architecture ratchet requires that helper).

- [ ] **Step 5: Run the focused E2E test and type/build checks through CI**

Expected: Inspector test is GREEN.

### Task 3: Canonicalize shell Navigator terminology without persistence breakage

**Files:**
- Modify: `apps/desktop/src/app/ActivityRail.tsx`
- Modify: `apps/desktop/src/app/WorkspaceSidebar.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: shell CSS selectors only where semantic naming requires it; do not create a new CSS layer.

**Interfaces:**
- Consumes: existing `STORAGE_KEYS.sidebarCollapsed` key.
- Produces: user-facing and TypeScript-level Navigator terminology while retaining the persisted storage key.

- [ ] **Step 1: Rename component props/state in touched shell code**

Use `navigatorOpen`, `onToggleNavigator`, and `WorkspaceNavigator` product terminology in touched code. The storage key remains `sidebarCollapsed` for compatibility and receives a comment documenting that it is a legacy persistence key.

- [ ] **Step 2: Give the Navigator a stable accessible label**

The structural left pane must render:

```tsx
<aside aria-label="Workspace navigator" ...>
```

- [ ] **Step 3: Update Activity Rail copy**

The toggle label must read `Show/Hide Navigator — ⌘/Ctrl+B` rather than `workspace sidebar`.

- [ ] **Step 4: Preserve route-change behavior for this foundation slice**

Do not redesign route-specific content yet. Existing shell navigation may continue closing the shell Navigator on explicit route selection until the route-content migration slice replaces it; document this transitional exception in code.

- [ ] **Step 5: Verify the Navigator E2E contract is GREEN**

### Task 4: Add architecture ratchets for workbench terminology and structured errors

**Files:**
- Modify: architecture ratchet script used by the `Architecture Ratchet` workflow.
- Test: the same script/self-test mechanism already used by existing route ratchets.

**Interfaces:**
- Consumes: repository source scan under `apps/desktop/src`.
- Produces: fail-fast architecture assertions.

- [ ] **Step 1: Add a focused failing ratchet before cleanup**

The ratchet must reject new product-level structural class/component identifiers containing `drawer` outside an explicit legacy allowlist of route-specific debt scheduled for later migration. The allowlist must list exact current files/identifiers and must not permit wildcards.

- [ ] **Step 2: Add a user-visible error coercion ratchet**

Reject direct `String(...error...)` patterns in these migrated workbench files:

- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/app/WorkspaceInspector.tsx`
- `apps/desktop/src/app/WorkbenchInspectorSurface.tsx`

The rule is intentionally scoped in this PR; later route migrations extend the protected set.

- [ ] **Step 3: Run Architecture Ratchet and confirm GREEN after production migration**

### Task 5: Full branch verification and review

**Files:**
- No new production files expected.

**Interfaces:**
- Consumes: exact PR head SHA.
- Produces: merge-ready evidence.

- [ ] **Step 1: Open a draft PR from `refactor/workbench-interaction-contract` to `main`**

PR title: `refactor(ui): define workbench interaction contract`

- [ ] **Step 2: Fetch the full PR patch and self-review against the spec**

Check specifically:

- no route-specific product redesign leaked into the foundation slice;
- no duplicate Inspector close owners;
- no Bottom Panel Escape regression;
- no storage-key migration;
- no new visual-generation CSS;
- focus restoration is typed and deterministic;
- structured errors are normalized.

- [ ] **Step 3: Check review threads/reviews**

All load-bearing findings must be resolved before merge.

- [ ] **Step 4: Require exact-head GitHub Actions success**

Required workflows:

- Architecture Ratchet
- CI
- Native E2E

All must be `completed/success` for the same exact head SHA.

- [ ] **Step 5: Mark ready and squash-merge with `expected_head_sha`**

Commit title: `refactor(ui): define workbench interaction contract`

- [ ] **Step 6: Verify `main` moved to the returned merge SHA**
