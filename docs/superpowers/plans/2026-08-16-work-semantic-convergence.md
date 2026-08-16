# Work Semantic Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the Work owning surface onto RepoDesk's shared semantic primitives and typed domain adapters while preserving workflow authority, reducing historical Work CSS debt, and keeping the one-primary-action invariant.

**Architecture:** Keep `workPhaseState()` and existing Work mutations as the sole workflow authority. Add one focused `workSemantic.ts` adapter that exhaustively maps Work domain states into the already-established `SemanticState`/`SemanticTone` vocabulary; route and review components then compose shared primitives without parsing display strings. Canonicalize the Work workbench shell instead of creating another visual generation, remove the obsolete polish layer, and strengthen the architecture ratchet so the migration cannot regress.

**Tech Stack:** React 19, TypeScript, TanStack Query, Playwright mock-IPC E2E, shared semantic UI primitives, Node architecture-ratchet tests, Tauri/Rust backend unchanged.

## Global Constraints

- Preserve the approved Cut F semantic vocabulary exactly: `positive | attention | critical | neutral | info`.
- Do not change backend/domain workflow policy to fit presentation.
- Do not add a UI framework, utility-CSS framework, runtime CSS-in-JS dependency, or extra presentation-only backend call.
- A Work phase may expose at most one primary action.
- Typed domain state must reach UI through exhaustive adapters; no substring/status-text inference.
- Do not add `*-vN`, `*-polish.css`, `new-ui`, `design-v2`, raw TSX hex, or new static inline layout styles.
- Feature-local CSS may only stay flat or shrink relative to `main`.
- Preserve Work → Changes → Runs lifecycle ownership and the existing fail-closed review evidence behavior.
- Scope this PR to Work only; Runs, Projects, and Code remain later Cut F slices.

---

### Task 1: Lock the Work semantic contract RED

**Files:**
- Create: `apps/desktop/e2e/work-design-system.spec.ts`
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`

**Interfaces:**
- Consumes: `PhaseStatus`, `ExecutionEvidenceStatus`, existing mock IPC fixtures, shared primitive boundary from Cut F slice 1.
- Produces: architecture requirements for `apps/desktop/src/features/work/workSemantic.ts`, primitive consumption in Work owning surfaces, canonical non-versioned Work shell, and no obsolete Work polish layer.

- [ ] **Step 1: Add representative Work semantic Playwright tests before production changes**

Create `apps/desktop/e2e/work-design-system.spec.ts` with mock-IPC cases that assert behavior rather than pixels:

```ts
import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { incompleteReviewFixtures, type CommandFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

async function boot(page: Page, fixtures: CommandFixtures) {
  await installMockIpc(page, fixtures);
  await page.goto("/");
}

test("current Work phase and completed phases expose typed semantic state", async ({ page }) => {
  await boot(page, currentOnboardedFixtures);
  const current = page.getByRole("status", { name: "Current phase: Execute" });
  await expect(current).toHaveAttribute("data-semantic-tone", "info");
  await expect(page.getByRole("status", { name: "Phase Scope: Done" })).toHaveAttribute("data-semantic-tone", "positive");
});

test("prepared execution context is positive while missing approvals stay attention", async ({ page }) => {
  await boot(page, currentOnboardedFixtures);
  await expect(page.getByText("Prepared", { exact: true }).last()).toHaveAttribute("data-semantic-tone", "positive");
  await expect(page.getByText("Action required", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
});

test("Work route uses shared surface loading and error ownership", async ({ page }) => {
  const delayed = {
    ...currentOnboardedFixtures,
    work_phase_state: { __mock_delay_ms: 1_000, __mock_value: currentOnboardedFixtures.work_phase_state },
  } as CommandFixtures;
  await boot(page, delayed);
  await expect(page.getByRole("status").filter({ hasText: "Loading Work Item flow" })).toBeVisible();
});

test("incomplete review evidence is a critical semantic blocker", async ({ page }) => {
  await boot(page, incompleteReviewFixtures);
  await expect(page.getByRole("alert").filter({ hasText: "cannot prove which tracked paths changed" }))
    .toHaveAttribute("data-semantic-tone", "critical");
});
```

Add a separate error-fixture assertion using `work_phase_state: { __mock_error: "fixture work failure" }` and require a surface `ErrorState` with retry/open-runs actions.

- [ ] **Step 2: Extend the architecture ratchet with a Work migration contract**

In `scripts/check-source-architecture.mjs`, add and export `evaluateWorkSemanticContract()` with exact ownership checks:

```js
const WORK_SEMANTIC_ADAPTER = "apps/desktop/src/features/work/workSemantic.ts";
const WORK_TYPED_SURFACES = [
  "apps/desktop/src/features/work/WorkTab.tsx",
  "apps/desktop/src/features/work/ReviewPanel.tsx",
  "apps/desktop/src/features/work/WorkSurface.tsx",
];
const OBSOLETE_WORK_STYLES = [
  "apps/desktop/src/features/work/work-focus-polish.css",
  "apps/desktop/src/app/styles/work-hierarchy-v3.css",
];
```

The evaluator must fail when the adapter is absent, when migrated surfaces do not consume `../../shared/ui/primitives`, when Work domain state is inferred through `statusTone()`/status-text substring heuristics, when `work-workbench-vN` remains in migrated Work source, or when either obsolete stylesheet still exists after the migration implementation.

Call `evaluateWorkSemanticContract()` from `runArchitectureRatchet()`.

- [ ] **Step 3: Unit-test the new Work architecture contract**

Extend `scripts/design-system-ratchet.test.mjs` to prove the Work contract is exported and rejects missing/legacy ownership. The test should invoke `evaluateWorkSemanticContract()` against the branch working tree and intentionally be RED until Tasks 2–4 land.

- [ ] **Step 4: Run the architecture test and targeted Work E2E to record RED**

Run:

```bash
node --test scripts/*.test.mjs
pnpm --prefix apps/desktop exec playwright test e2e/work-design-system.spec.ts --project=chromium
```

Expected: FAIL because `workSemantic.ts` and migrated primitive ownership do not exist yet; failures must be contract failures, not syntax/fixture failures.

- [ ] **Step 5: Commit the RED contract**

```bash
git add apps/desktop/e2e/work-design-system.spec.ts scripts/check-source-architecture.mjs scripts/design-system-ratchet.test.mjs
git commit -m "test(ui): lock Work semantic convergence contract"
```

---

### Task 2: Add exhaustive Work semantic adapters and migrate the route shell

**Files:**
- Create: `apps/desktop/src/features/work/workSemantic.ts`
- Modify: `apps/desktop/src/features/work/WorkSurface.tsx`
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`

**Interfaces:**
- Consumes: `PhaseStatus`, `ExecutionEvidenceStatus`, `SemanticState`, `SemanticTone`, shared `PanelHeader`, `StatusBadge`, `Metric`, `LoadingState`, `ErrorState`, `ActionBar`, `EvidenceState`.
- Produces:
  - `phaseSemanticState(status: PhaseStatus, current: boolean): SemanticState`
  - `executionEvidenceSemanticState(status: ExecutionEvidenceStatus): SemanticState`
  - `preparedContextSemanticState(prepared: boolean): SemanticState`
  - `approvalSemanticState(ready: boolean): SemanticState`

- [ ] **Step 1: Implement exhaustive adapters with no fallback**

Create `workSemantic.ts`:

```ts
import type { ExecutionEvidenceStatus, PhaseStatus } from "../../shared/api/orchestrate";
import type { SemanticState } from "../../shared/ui/primitives";

function assertNever(value: never): never {
  throw new Error(`Unhandled Work semantic state: ${String(value)}`);
}

export function phaseSemanticState(status: PhaseStatus, current: boolean): SemanticState {
  if (current) return { label: "Current", tone: "info" };
  switch (status) {
    case "done": return { label: "Done", tone: "positive" };
    case "in_progress": return { label: "In progress", tone: "info" };
    case "available": return { label: "Available", tone: "neutral" };
    case "locked": return { label: "Locked", tone: "neutral" };
    default: return assertNever(status);
  }
}

export function executionEvidenceSemanticState(status: ExecutionEvidenceStatus): SemanticState {
  switch (status) {
    case "ready": return { label: "Complete", tone: "positive" };
    case "recovery_required": return { label: "Recovery required", tone: "attention" };
    case "incomplete": return { label: "Incomplete", tone: "critical" };
    case "not_required": return { label: "Not required", tone: "neutral" };
    default: return assertNever(status);
  }
}

export function preparedContextSemanticState(prepared: boolean): SemanticState {
  return prepared
    ? { label: "Prepared", tone: "positive" }
    : { label: "Rebuild required", tone: "attention" };
}

export function approvalSemanticState(ready: boolean): SemanticState {
  return ready
    ? { label: "Ready", tone: "positive" }
    : { label: "Action required", tone: "attention" };
}
```

- [ ] **Step 2: Migrate Work surface phase identity to typed shared semantics**

In `WorkSurface.tsx`, import `StatusBadge` and `phaseSemanticState`. Rename the outer shell class from `work-workbench-v3` to canonical `work-workbench`. Keep the data-driven progress-width inline style because it is explicitly allowed by the design contract.

Render current phase status through `StatusBadge` with an accessible label such as `Current phase: Execute`; do not infer tone from phase copy.

- [ ] **Step 3: Migrate route loading/error/header/facts**

In `WorkTab.tsx`:

- replace route error markup with `ErrorState scope="surface"` and an `ActionBar` containing Retry as the sole primary action and Open Runs as secondary;
- replace `focus-empty` loading markup with `LoadingState scope="surface"`;
- replace the local `work-phase-header` title row with `PanelHeader`;
- render latest-run files/tokens/cost with `Metric` only when a real non-dry-run exists;
- render each phase rail state using `StatusBadge` driven by `phaseSemanticState()` while retaining the six-phase rail geometry and accessible phase title;
- preserve exact existing phase copy and navigation behavior.

- [ ] **Step 4: Run targeted Work E2E and architecture tests**

```bash
node --test scripts/*.test.mjs
pnpm --prefix apps/desktop exec playwright test e2e/work-design-system.spec.ts e2e/work-action-ownership.spec.ts e2e/work-golden-path.spec.ts --project=chromium
```

Expected: route loading/error/header/phase tests move GREEN; later Review/CSS contract assertions may still be RED until Tasks 3–4.

- [ ] **Step 5: Commit the route-shell migration**

```bash
git add apps/desktop/src/features/work/workSemantic.ts apps/desktop/src/features/work/WorkSurface.tsx apps/desktop/src/features/work/WorkTab.tsx
git commit -m "refactor(work): adopt typed semantic route states"
```

---

### Task 3: Converge Work evidence and action ownership

**Files:**
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`
- Modify: `apps/desktop/src/features/work/ReviewPanel.tsx`
- Modify: `apps/desktop/e2e/work-action-ownership.spec.ts`
- Modify: `apps/desktop/e2e/work-golden-path.spec.ts`

**Interfaces:**
- Consumes: `executionEvidenceSemanticState`, `preparedContextSemanticState`, `approvalSemanticState`, shared `EvidenceState`, `StatusBadge`, `LoadingState`, `ErrorState`, `EmptyState`, `ActionBar`, `Metric`, `InspectorSection`.
- Produces: one shared ActionBar owner for each phase decision area and explicit semantic evidence for execution/review trust state.

- [ ] **Step 1: Migrate execution packet evidence**

Inside `ExecutionPreviewCompact`:

- use `LoadingState scope="inline"` while the exact packet is prepared;
- use `ErrorState scope="inline"` when preview evidence cannot be read;
- use `PanelHeader` plus `StatusBadge` with `preparedContextSemanticState(context.prepared)`;
- use `EvidenceState` for context and workspace provenance;
- use `Metric` for token estimate and cost ceiling because both can change launch decisions;
- keep packet fingerprint/routing details in `InspectorSection`/`details` rather than hiding launch blockers there.

- [ ] **Step 2: Make launch approval state semantic**

Replace the local text-only `ready` / `action required` marker with `StatusBadge` using `approvalSemanticState(executeApprovalsMet)`. Required approval checkboxes remain explicit controls; stale approval copy remains visible.

- [ ] **Step 3: Move phase decisions into `ActionBar`**

Use `ActionBar` for:

- scope navigation when no project exists;
- manual import actions (secondary actions, no artificial primary action);
- Review: Accept & stage → Verify as primary, Reject → re-run as destructive;
- Finish: Commit reviewed changes as the single primary action adjacent to the commit message;
- prepare/execute/verify generic CTA: the existing single primary CTA;
- mutation errors: shared inline `ErrorState`, not `.work-error` text.

Delete the hidden `work-tools-strip`/Open Orchestrate block from `WorkTab`; Work remains the owning workflow surface and existing lower-level utility access remains outside the primary flow.

- [ ] **Step 4: Migrate fail-closed Review evidence**

In `ReviewPanel.tsx`:

- map `ExecutionEvidenceStatus` through `executionEvidenceSemanticState()`;
- loading evidence/diffs/proposals use scoped `LoadingState`;
- unavailable/incomplete/recovery-required/diff failures use scoped `ErrorState` with the existing remediation text preserved verbatim;
- proven zero-change uses a positive `EvidenceState`, not a generic empty-looking message;
- successful captured changes show a positive `EvidenceState` before diff details;
- no diff query is enabled until evidence status is `ready`, preserving the current fail-closed optimization.

- [ ] **Step 5: Update ownership tests away from legacy CSS selectors**

Change `work-action-ownership.spec.ts` and relevant `work-golden-path.spec.ts` assertions to roles/semantic container behavior. Keep the behavioral invariant: each phase exposes at most one primary action and Review does not render a second generic CTA.

- [ ] **Step 6: Run Work behavior tests**

```bash
pnpm --prefix apps/desktop exec playwright test e2e/work-design-system.spec.ts e2e/work-action-ownership.spec.ts e2e/work-golden-path.spec.ts --project=chromium
node --test scripts/*.test.mjs
```

Expected: PASS except CSS-deletion architecture assertions that intentionally remain for Task 4.

- [ ] **Step 7: Commit evidence/action convergence**

```bash
git add apps/desktop/src/features/work/WorkTab.tsx apps/desktop/src/features/work/ReviewPanel.tsx apps/desktop/e2e/work-action-ownership.spec.ts apps/desktop/e2e/work-golden-path.spec.ts
git commit -m "refactor(work): converge evidence and action ownership"
```

---

### Task 4: Remove Work visual generations and lower CSS debt

**Files:**
- Create: `apps/desktop/src/app/styles/work-hierarchy.css`
- Delete: `apps/desktop/src/app/styles/work-hierarchy-v3.css`
- Delete: `apps/desktop/src/features/work/work-focus-polish.css`
- Modify: `apps/desktop/src/features/work/work-route.css`
- Modify: `apps/desktop/src/features/work/work-visual-language.css` only to delete obsolete selectors; do not grow bytes.

**Interfaces:**
- Consumes: canonical `.work-workbench` class from Task 2 and semantic primitive CSS from Cut F slice 1.
- Produces: one non-versioned Work shell stylesheet and a smaller feature-local CSS footprint.

- [ ] **Step 1: Canonicalize the versioned workbench shell without adding a new generation**

Move the still-used structural rules from `work-hierarchy-v3.css` to `work-hierarchy.css`, changing `.work-workbench-v3` selectors to `.work-workbench`. Preserve layout/responsive behavior; remove rules that only supported deleted `work-tools-strip` or replaced legacy current-step markup.

- [ ] **Step 2: Remove the obsolete polish layer**

Delete `work-focus-polish.css`. Do not copy its dead/duplicated toolbar and focus overrides into another stylesheet. Retain only behavior proven necessary by Work E2E in existing canonical Work styles.

- [ ] **Step 3: Update route imports and stale selectors**

In `work-route.css`:

```css
/* remove */
@import "./work-focus-polish.css" layer(legacy);
@import "../../app/styles/work-hierarchy-v3.css" layer(workbench);

/* add */
@import "../../app/styles/work-hierarchy.css" layer(workbench);
```

Update remaining `.work-workbench-v3` media selectors to `.work-workbench`. Delete unused `work-tools-strip`, `.work-phase-error`, `.work-cta-row`, and other selectors whose consumers disappeared, but do not add replacement feature-local CSS.

- [ ] **Step 4: Run the architecture ratchet and Work E2E**

```bash
node --test scripts/*.test.mjs
BASE_SHA=$(git merge-base origin/main HEAD) node scripts/check-source-architecture.mjs
pnpm --prefix apps/desktop exec playwright test e2e/work-design-system.spec.ts e2e/work-action-ownership.spec.ts e2e/work-golden-path.spec.ts --project=chromium
```

Expected: all GREEN; the architecture output must show no feature CSS growth and no remaining Work versioned/polish ownership.

- [ ] **Step 5: Commit CSS debt removal**

```bash
git add -A apps/desktop/src/app/styles/work-hierarchy.css apps/desktop/src/app/styles/work-hierarchy-v3.css apps/desktop/src/features/work/work-focus-polish.css apps/desktop/src/features/work/work-route.css apps/desktop/src/features/work/work-visual-language.css
git commit -m "refactor(work): retire historical visual layers"
```

---

### Task 5: Full exact-head verification and PR finish

**Files:**
- Modify only if verification finds a real regression; any fix requires a new regression test first.

**Interfaces:**
- Consumes: completed Work migration.
- Produces: exact-head verification evidence suitable for squash merge.

- [ ] **Step 1: Run full architecture and frontend gates**

```bash
node --test scripts/*.test.mjs
BASE_SHA=$(git merge-base origin/main HEAD) node scripts/check-source-architecture.mjs
pnpm --prefix apps/desktop build
```

- [ ] **Step 2: Run repository Rust quality gates**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

- [ ] **Step 3: Run Playwright behavior suite used by CI**

```bash
pnpm --prefix apps/desktop exec playwright test --project=chromium
```

- [ ] **Step 4: Require native Tauri E2E on the same head**

Wait for the repository's `E2E (native)` workflow and confirm the Tauri/WebDriverIO smoke is GREEN on the exact PR head. Do not merge on an older green SHA.

- [ ] **Step 5: Self-review against the approved design**

Confirm:

- Work uses the shared semantic vocabulary and primitives;
- phase/evidence tone comes from exhaustive typed adapters;
- no new primary-action owner was introduced;
- Review remains fail-closed and does not fetch diffs when execution evidence is untrusted;
- feature CSS bytes did not grow;
- `work-focus-polish.css` and `work-hierarchy-v3.css` are gone;
- no Runs/Projects/Code migration leaked into the diff.

- [ ] **Step 6: Mark the PR ready and squash-merge only after exact-head green**

Use squash merge with a concise title such as:

```text
refactor(work): converge on semantic workbench primitives
```

The next Cut F slice after merge is Runs; do not start it inside this PR.
