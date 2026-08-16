# Design-System Convergence — Changes Reference Slice Implementation Plan

> **Scope:** This plan implements the first executable slice of the approved Cut F design: semantic primitives, the Changes reference migration, non-regression ratchets, and Changes-focused visual/behavior coverage. Work, Runs, Projects, and Code remain follow-up slices under the same approved design.

**Goal:** Establish one typed semantic UI language in production, prove it on the evidence-heavy Changes surface, and freeze further visual debt before migrating the remaining four owning surfaces.

**Architecture:** Domain enums remain owned by existing feature/API models. A new feature-local Changes semantic adapter maps those enums exhaustively into a narrow shared `SemanticTone` vocabulary. Focused shared primitives render semantic state without parsing domain strings. Shared primitive styling lives in one non-versioned shared stylesheet; feature-only Changes geometry remains feature-local. Architecture checks freeze raw visual debt and prohibit new string-based domain-state inference.

**Tech stack:** React 18 + TypeScript, CSS, TanStack Query, Playwright, Node architecture checks, existing Tauri/Rust backend unchanged for this slice.

**Approved design:** `docs/superpowers/specs/2026-08-16-design-system-convergence-design.md`

---

## Non-negotiable invariants

- `StatusBadge` and `EvidenceState` never inspect RepoDesk domain strings.
- Changes domain state is mapped through exhaustive typed adapters; no silent `neutral` fallback.
- `manual` attribution renders neutral; exact-attribution project policy blockers render critical through the commit gate, not by changing the attribution fact.
- At most one primary action exists in one `ActionBar` instance.
- Critical blockers remain visible in the primary surface and are not hidden only in an inspector.
- No `design-v2`, `work-v4`, `new-ui`, new `*-vN`, or new route-wide polish layer.
- New static inline layout styles are forbidden. Data-driven inline values are allowed only where genuinely dynamic.
- Historical debt is grandfathered only through explicit deterministic baselines that can decrease but must not increase casually.
- This slice does not change backend trust policy or Safe Commit Manifest semantics.

## Task 1 — RED: lock the reference semantic contract

**Files:**
- Create: `apps/desktop/e2e/changes-design-system.spec.ts`
- Modify: `scripts/check-source-architecture.mjs`

**Test-first behavior:**

1. Add Playwright coverage for the Changes reference states using the existing mock-IPC fixture infrastructure:
   - exact isolated attribution is visibly `Exact` / positive and not described as merely recorded;
   - manual/legacy/weak attribution is not rendered as exact;
   - stale verification uses attention semantics and preserves stale-reason text;
   - scope violation is critical and exposes the existing override action without creating a second primary action;
   - commit-ready manifest is positive and has no blocker presentation;
   - Changes governance loading and error states use shared semantic state containers rather than ad-hoc text/notice markup.
2. Prefer semantic/accessibility assertions (`role`, visible state label, one-primary-action ownership, remediation text). Do not assert exact margin/pixel values.
3. Extend `scripts/check-source-architecture.mjs` with an initial RED contract that requires Changes to consume the new primitive boundary and a Changes semantic adapter. Keep the checks narrowly targeted so they fail for the missing architecture, not because historical UI debt already exists elsewhere.
4. Add a source invariant that Changes typed domain state must not call `statusTone()` or classify with `includes("ok")`, `includes("error")`, etc.

**RED verification:**

- `pnpm --dir apps/desktop exec playwright test e2e/changes-design-system.spec.ts`
- `node scripts/check-source-architecture.mjs`

Commit the test-only RED head and let branch CI prove the expected failures before production code is added. Record the failing job/run in the PR description.

## Task 2 — GREEN: establish focused semantic primitives

**Files:**
- Create: `apps/desktop/src/shared/ui/primitives/semantic.ts`
- Create: `apps/desktop/src/shared/ui/primitives/StatusBadge.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/EvidenceState.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/PanelHeader.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/EmptyState.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/LoadingState.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/ErrorState.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/InspectorSection.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/ActionBar.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/Metric.tsx`
- Create: `apps/desktop/src/shared/ui/primitives/index.ts`
- Create: `apps/desktop/src/shared/ui/primitives/primitives.css`

**Implementation:**

1. Define the fixed vocabulary:

   ```ts
   export type SemanticTone = "positive" | "attention" | "critical" | "neutral" | "info";
   ```

2. Keep primitive APIs intentionally narrow. `StatusBadge` receives a label + tone, not arbitrary domain data. `EvidenceState` receives label/state/tone/detail and optional technical-detail child/affordance. `PanelHeader` standardizes hierarchy. Empty/loading/error states accept `scope: "inline" | "surface"`.
3. `ActionBar` accepts explicit primary/secondary/destructive slots or equivalent typed props and renders predictable semantic grouping. Do not create a generic “card” primitive.
4. `Metric` remains a compact semantic readout, not a dashboard tile replacement.
5. Style semantic tones using existing foundation variables. Add token aliases to `foundation.css` only if an existing token cannot express the required semantic distinction; do not add raw status hex maps in TSX.
6. Keep `SharedComponents.tsx` intact except for migrations needed to remove a now-duplicated `EmptyState` consumer. Do not turn it into the new primitive barrel.

**Focused verification:**

- `pnpm --dir apps/desktop build`
- architecture script should still fail only on the yet-unmigrated Changes assertions, not on primitive construction.

## Task 3 — GREEN: add exhaustive Changes domain adapters

**Files:**
- Create: `apps/desktop/src/features/changes/changesSemantic.ts`
- Modify only if type exports are needed: `apps/desktop/src/shared/api/engineering.ts`

**Implementation:**

1. Add pure functions for the domain states currently rendered by Changes, including at minimum:
   - attribution;
   - Safe Commit state / gate;
   - review;
   - verification current/stale/failed/running/not-run;
   - scope and exceptional per-file scope;
   - acceptance criterion/result.
2. Each adapter returns an explicit small presentation object such as `{ label, tone, detail? }`.
3. Use exhaustive `switch` handling with a `never` assertion helper. Unknown future variants must cause TypeScript work, not silently become neutral.
4. Preserve factual distinctions. In particular:
   - exact attribution => `positive`;
   - derived/legacy => `attention`;
   - manual => `neutral`;
   - unattributed => `critical` when the attribution fact itself is missing;
   - project exact-attribution policy blocker remains a separate commit-gate critical state;
   - stale verification => `attention`, failed verification => `critical`.
5. Do not mutate backend/API enums and do not add a second state store.

**Focused verification:**

- `pnpm --dir apps/desktop build`

## Task 4 — GREEN: migrate ChangeSet evidence to semantic primitives

**Files:**
- Modify: `apps/desktop/src/features/changes/ChangeGovernancePanel.tsx`
- Modify: `apps/desktop/src/app/styles/changes-evidence.css`
- Modify: `apps/desktop/src/features/changes/changes-density.css` only for feature geometry that cannot move to primitives

**Implementation:**

1. Remove local tone/label helpers that duplicate adapter responsibilities: `safeStateTone`, `attributionMeta`, `criterionTone`, local `EvidenceCell`, and equivalent ad-hoc status rendering.
2. Replace the manifest header with `PanelHeader` + `StatusBadge`.
3. Replace the evidence grid cells with `EvidenceState` while preserving all trust-relevant detail: producer attribution, origin, baseline/run, scope, review, reviewed tree, verification, acceptance, HEAD/commit.
4. Render commit blockers as a primary-surface `ErrorState`/critical evidence block. Render compatibility warnings as attention/info without hiding blockers.
5. Render stale verification through typed semantic state with stale reason intact.
6. Convert verification and scope-override controls to `ActionBar`, preserving the existing one-primary-action invariant and mutation disabled/loading behavior.
7. Migrate Acceptance Evidence Matrix badges through typed adapters and `StatusBadge`; keep evidence-link controls feature-local.
8. Move only reusable semantic styling into `primitives.css`; keep Changes-specific matrix/grid/layout styles feature-local. Delete obsolete selectors as consumers disappear so this migration reduces or holds CSS debt rather than adding a second layer.

**Focused verification:**

- `pnpm --dir apps/desktop build`
- `pnpm --dir apps/desktop exec playwright test e2e/changes-design-system.spec.ts`

## Task 5 — GREEN: migrate the Changes workbench shell and local states

**Files:**
- Modify: `apps/desktop/src/features/changes/ChangesTab.tsx`
- Modify: `apps/desktop/src/features/changes/changes-density.css`
- Modify: `apps/desktop/src/features/changes/changes-route.css` only if a real route-level shell rule remains justified
- Modify/remove consumers from: `apps/desktop/src/shared/ui/SharedComponents.tsx`

**Implementation:**

1. Replace route header/action markup with the shared header/action vocabulary without making Changes look like a SaaS dashboard.
2. Map Git file status and exceptional scope states explicitly; use `StatusBadge` instead of raw `.pill ok|warn|danger` markup for semantic states.
3. Use the shared `EmptyState` for no-changes/no-selection conditions and shared `LoadingState` for preview loading where appropriate.
4. Preserve specialized diff/editor geometry and `DiffViewer`; semantic convergence must not flatten the actual engineering workspace into cards.
5. Normalize warning/error feedback using shared states when the feedback is a state, while retaining transient navigation notice semantics where a toast/notice is more appropriate.
6. Preserve all existing interactions: refresh, manifest toggle, Analyze/findings drawer, file selection, Diff/File switch, open in Code, double-click behavior, and query invalidation.
7. Remove any newly unnecessary Changes-specific semantic selectors; do not create a new route-wide style generation.

**Focused verification:**

- `pnpm --dir apps/desktop build`
- existing Changes/daily-loop Playwright tests relevant to file review/navigation
- `pnpm --dir apps/desktop exec playwright test e2e/changes-design-system.spec.ts`

## Task 6 — GREEN: install the debt-freeze architecture ratchet

**Files:**
- Modify: `scripts/check-source-architecture.mjs`
- Optionally create only if it keeps the main script focused: `scripts/design-system-baseline.mjs`

**Implementation:**

1. Measure the current post-Changes-migration frontend debt and encode reviewed grandfather baselines for remaining unmigrated surfaces.
2. Add deterministic checks that fail on regression:
   - new raw `#hex` in TSX beyond the recorded baseline/allowlist;
   - new static inline `style={{...}}` layout usage beyond baseline, with narrowly documented allowances for genuinely data-driven values;
   - any new visual filename/class matching `-v\d+`;
   - any new route-wide `*-polish.css` generation;
   - feature CSS byte growth above reviewed baselines;
   - new `statusTone()` consumers in typed domain feature code;
   - new string-substring domain-state classification (`includes("ok")`, `includes("error")`, etc.) in migrated Changes code.
3. Treat baseline lowering as success. Never require migration PRs to restore removed debt just to match an exact count.
4. Keep the architecture check cheap: source scanning only, no browser/build subprocess.
5. Include clear failure messages naming the violated contract and file.

**Verification:**

- `node scripts/check-source-architecture.mjs`
- intentionally perturb one rule in a throwaway/test-only commit or fixture if needed to prove the ratchet fails for a new violation, then revert that perturbation before the final implementation head.

## Task 7 — Reference visual/behavior regression coverage

**Files:**
- Modify: `apps/desktop/e2e/changes-design-system.spec.ts`
- Modify existing fixture helpers only if necessary: `apps/desktop/e2e/fixtures.ts` and/or `apps/desktop/e2e/current-fixtures.ts`
- Add screenshot baselines only through the repository's existing Playwright snapshot convention; do not invent a second screenshot framework.

**Implementation:**

1. Finish fixture coverage for normal/loading/empty/error and the five trust-critical Changes states.
2. Add screenshot regression for the Changes reference surface at the repository's standard deterministic viewport if the existing Playwright environment provides stable font/rendering. If cross-platform rendering makes pixel screenshots inherently unstable, use the existing CI-supported screenshot project/platform only; do not weaken functional assertions.
3. Assert accessible semantics where practical:
   - critical blockers are text-visible, not color-only;
   - loading/error state has appropriate role/live semantics;
   - controls retain names and disabled state;
   - one primary action remains visible for the relevant state.
4. Keep tests state-oriented rather than selector-class-oriented so future CSS cleanup does not invalidate behavior tests.

**Verification:**

- `pnpm --dir apps/desktop exec playwright test e2e/changes-design-system.spec.ts`
- `pnpm --dir apps/desktop e2e` if focused test is green.

## Task 8 — Self-review, exact-head verification, PR and merge

**Files:**
- Update PR description with RED/GREEN evidence and final architecture metrics.
- No production changes during verification unless a failing check exposes a real defect; if so, fix root cause and restart exact-head verification.

**Self-review checklist:**

1. Search the diff for a second semantic classifier, fallback string parsing, duplicated primitives, new versioned names, new inline static geometry, raw TSX status colors, and route-wide CSS growth.
2. Verify Changes still consumes the existing Safe Commit Manifest and does not introduce a second readiness decision.
3. Verify blocker/warning attribution semantics remain truthful and `manual` is not mislabeled exact/critical by itself.
4. Verify `ChangeGovernancePanel.tsx` and `ChangesTab.tsx` become simpler or at minimum do not gain new mixed responsibilities.
5. Verify new shared primitive files remain focused and individually understandable.

**Exact-head commands/checks:**

- `node scripts/check-source-architecture.mjs`
- `pnpm --dir apps/desktop build`
- `pnpm --dir apps/desktop e2e`
- repository full CI jobs (Rust fmt, Clippy, Rust tests, coverage, supply-chain/security, frontend budgets)
- native Tauri/WebDriverIO E2E

Use the exact branch head SHA for all final status claims. Only mark the PR ready and merge when required exact-head checks are green.

**Merge:** Squash merge this reference slice into `main`. Do not bundle Work/Runs/Projects/Code migrations into the same PR. After merge, create the next plan/slice for Work using the established primitives and lowered ratchet baselines.

---

## Definition of done for this slice

- Changes is the first production route using the new typed semantic language end-to-end.
- Changes has no substring-based domain status inference.
- Shared primitives exist as focused components rather than a new god-file.
- Changes blockers, evidence, actions, loading/error/empty states use those primitives consistently.
- Existing Changes engineering interactions and trust semantics are unchanged except for controlled presentation improvements.
- Architecture CI freezes new versioned visual generations, string inference, inline/raw-style growth, and CSS debt growth.
- Playwright covers exact/weak/stale/scope/ready plus loading/empty/error states.
- Full exact-head CI and native E2E are green before squash merge.
