# Design-System Convergence — Work Slice Implementation Plan

> **Scope:** Continue the approved Cut F strangler migration after Changes. This slice migrates the operational Work surface — phase hierarchy, current workflow state, execution packet/approvals, review evidence, loading/error states, and primary action ownership — onto the shared semantic primitive layer. Runs, Projects, and Code remain separate follow-up slices.

**Goal:** Make Work speak the same typed semantic UI language as Changes while preserving the canonical six-phase workflow and its one-primary-action invariant.

**Architecture:** Existing backend/API types remain authoritative. A focused `workSemantic.ts` adapter maps typed Work domain states into the shared `SemanticTone` vocabulary. Work components consume shared primitives directly; primitives remain domain-agnostic. Existing Work geometry is retained or simplified in existing stylesheets, with no new visual generation and no feature-CSS growth.

**Tech stack:** React 18 + TypeScript, TanStack Query, CSS, Playwright mock IPC, Node architecture ratchet, existing Tauri/Rust backend unchanged.

**Approved design:** `docs/superpowers/specs/2026-08-16-design-system-convergence-design.md`

---

## Non-negotiable invariants

- Preserve canonical workflow order: `Scope -> Prepare -> Execute -> Review -> Verify -> Finish`.
- Preserve backend `PhaseProgress`/CTA authority; UI must not invent readiness or phase transitions.
- A Work phase exposes at most one primary action.
- Review acceptance may be the primary action; reject is a distinct destructive/secondary decision, never a second primary.
- Finish commit is the single primary action for Finish.
- Manual handoff does not become semantically critical merely because it is manual.
- Execution evidence `incomplete` is critical; `recovery_required` is attention and explicitly says repair evidence without rerunning the agent; `not_required` is neutral.
- No string-substring inference of typed Work states.
- No new `*-vN`, `*-polish.css`, `new-ui`, or equivalent visual generation.
- Existing dynamic inline progress width in `WorkSurface` is an allowed data-driven inline value; do not add static inline geometry.
- Work feature CSS may stay flat or shrink only. Reusable semantic styling belongs in `shared/ui/primitives/primitives.css`.
- No backend trust/policy changes in this slice.

## Task 1 — RED: lock Work semantic and action contracts

**Files:**
- Create: `apps/desktop/e2e/work-design-system.spec.ts`
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`

**Test-first behavior:**

1. Add Work-focused Playwright fixtures/assertions for:
   - execute/current phase rendered as typed info state while completed phases are positive and locked phases neutral;
   - prepared execution packet is positive; rebuild-required packet is attention;
   - launch approvals distinguish `Ready`, `Action required`, and stale plan approval without string-derived tone;
   - phase loading uses shared accessible `LoadingState`;
   - phase authority failure uses shared surface `ErrorState` and preserves Retry/Open Runs remediation;
   - review `incomplete` evidence is critical and blocks diff presentation;
   - review `recovery_required` is attention and preserves “repair, do not rerun” remediation;
   - review complete/no tracked writes is an explicit neutral/positive empty result, not an error;
   - Review has exactly one primary Accept action and a separately classified Reject action;
   - Finish exposes exactly one primary commit action.
2. Extend the architecture contract with:
   - required `apps/desktop/src/features/work/workSemantic.ts`;
   - Work primary surfaces importing `../../shared/ui/primitives` and `./workSemantic` where typed state is rendered;
   - no `statusTone()` calls or status-text substring inference in migrated Work surfaces;
   - no new Work visual generation.
3. Commit the RED contract before production code and use branch CI to prove the intended failures.

**RED verification:**

- `node --test scripts/*.test.mjs`
- `pnpm --dir apps/desktop exec playwright test e2e/work-design-system.spec.ts`

## Task 2 — GREEN: add exhaustive Work semantic adapters

**Files:**
- Create: `apps/desktop/src/features/work/workSemantic.ts`

**Implementation:**

1. Reuse shared `SemanticPresentation`/`SemanticTone`; do not create a Work-specific tone vocabulary.
2. Add exhaustive typed mappings for:
   - `PhaseStatus`;
   - current/complete phase presentation;
   - `ExecutionEvidenceStatus`;
   - execution packet preparation state;
   - launch approval state (`ready | action_required | stale` as a local closed union derived from typed booleans, not display text).
3. Canonical mappings:
   - phase `done` -> positive;
   - phase `in_progress` -> info;
   - phase `available` -> attention;
   - phase `locked` -> neutral;
   - execution evidence `ready` -> positive;
   - `incomplete` -> critical;
   - `recovery_required` -> attention;
   - `not_required` -> neutral;
   - packet prepared -> positive; rebuild required -> attention;
   - approval ready -> positive; action required/stale -> attention.
4. Use exhaustive switches plus `assertNever` where the source is a discriminated union.
5. Keep adapter functions pure and O(1).

**Focused verification:**

- `pnpm --dir apps/desktop build`

## Task 3 — GREEN: migrate Work shell and phase hierarchy

**Files:**
- Modify: `apps/desktop/src/features/work/WorkSurface.tsx`
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`
- Modify: `apps/desktop/src/features/work/work-visual-language.css`
- Modify: `apps/desktop/src/features/work/work-focus-polish.css` only to delete obsolete consumers
- Modify: `apps/desktop/src/features/work/work-route.css` only to remove dead imports/selectors when proven unused

**Implementation:**

1. `WorkSurface`:
   - use `EvidenceState`/`StatusBadge` for current workflow position rather than ad-hoc state copy;
   - keep dynamic progress width inline because it is data-driven;
   - migrate inspector header hierarchy to `PanelHeader` or `InspectorSection` without changing inspector ownership;
   - preserve Contract/Context/Intelligence toggles and project/task identity.
2. `WorkTab`:
   - replace the local current-step header with `PanelHeader`;
   - render latest-run files/tokens/cost through compact `Metric` primitives only because they change operational understanding;
   - keep the six-phase rail, but attach adapter-derived semantic state and accessible text rather than CSS-only meaning;
   - preserve `PHASE_COPY` explanatory workflow text.
3. Do not create a new shell/card abstraction or rename the route into another versioned class generation.
4. Remove obsolete Work semantic selectors as primitive consumers replace them; every modified feature CSS file must be <= its base byte size.

**Focused verification:**

- `pnpm --dir apps/desktop build`
- existing `e2e/daily-loop.spec.ts`
- new `e2e/work-design-system.spec.ts`

## Task 4 — GREEN: make ActionBar the Work action owner

**Files:**
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`
- Modify: `apps/desktop/src/features/work/ReviewPanel.tsx` only where action grouping belongs to the review evidence surface

**Implementation:**

1. Prepare/Execute/Verify shared CTA path uses one `ActionBar.primary`.
2. Scope:
   - when no project, `Connect a project` is the route’s primary action;
   - when task selection is required, preserve `TaskSwitcher` ownership without adding a competing primary CTA.
3. Review:
   - `Accept & stage -> Verify` is the single primary action;
   - `Reject -> re-run` uses destructive or explicit secondary slot;
   - memory proposal actions inside `ReviewPanel` remain local secondary mutations and must not masquerade as the phase primary action.
4. Finish:
   - commit message + `Commit reviewed changes` are grouped under one `ActionBar` primary action;
   - preserve bounded commit semantics and Changes link.
5. Mutation pending/disabled states remain unchanged.
6. Convert mutation failure text to shared `ErrorState` with useful detail.

**Focused verification:**

- Playwright asserts at most one visible primary Work action in each representative phase.

## Task 5 — GREEN: migrate execution packet and approval evidence

**Files:**
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`
- Modify: `apps/desktop/src/features/work/ExecutionStrategyControls.tsx`
- Modify existing Work/strategy CSS only by shrinking or holding size

**Implementation:**

1. `ExecutionPreviewCompact`:
   - loading -> `LoadingState`;
   - preview error -> `ErrorState`;
   - prepared/rebuild state -> typed `StatusBadge`;
   - packet facts use `EvidenceState`/`Metric` where appropriate while preserving fingerprint, selected sources, workspace isolation, expected writes, tokens, and cost.
2. Launch approvals:
   - heading state uses adapter-derived `StatusBadge`;
   - stale approval copy remains explicit;
   - required capability labels remain visible and keyboard-accessible.
3. `ExecutionStrategyControls`:
   - strategy profile/plan-shape status uses shared `StatusBadge` rather than local color-only badge classes;
   - material AI-call/token/cost/context values may use `Metric` if it reduces duplicated presentation;
   - do not alter routing/strategy selection behavior or plan fingerprint semantics.
4. No new backend calls or state stores.

## Task 6 — GREEN: migrate review execution-evidence states

**Files:**
- Modify: `apps/desktop/src/features/work/ReviewPanel.tsx`

**Implementation:**

1. Replace ad-hoc muted paragraphs with shared semantic states:
   - no run -> `EmptyState`;
   - evidence/diff loading -> `LoadingState`;
   - evidence transport failure/diff failure -> `ErrorState`;
   - `incomplete` -> critical `EvidenceState`/`ErrorState` with rerun remediation;
   - `recovery_required` -> attention `EvidenceState` with “repair evidence; do not rerun” detail;
   - `not_required` -> neutral evidence state;
   - complete zero-change capture -> explicit non-error `EmptyState`/positive evidence result.
2. Diffs remain specialized `DiffViewer` content.
3. Memory proposals remain available even when review evidence is blocked, preserving the existing separation of evidence and metadata.
4. Do not weaken the fail-closed gate that prevents diff reads before execution evidence is ready.

## Task 7 — GREEN: lower Work visual-debt baselines

**Files:**
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`
- Modify/delete Work CSS only when consumers are removed

**Implementation:**

1. Expand the semantic-contract ratchet from Changes-only to Changes + Work without duplicating classifier logic in the script.
2. Require migrated Work typed surfaces to consume `workSemantic.ts` and shared primitives.
3. Keep historical CSS debt grandfathered but lower the actual baseline through selector/import deletion where this slice removes consumers.
4. Preserve the existing dynamic progress-width allowance by leaving its baseline flat; do not create a general escape hatch for static inline styles.
5. Ratchet tests must prove:
   - Work cannot reintroduce `statusTone`/substring inference;
   - Work adapter/surface boundary is required;
   - new Work `*-vN`/`*-polish.css` remains rejected;
   - CSS baselines only decrease or stay flat.

## Task 8 — Self-review, exact-head verification, PR and squash merge

**Self-review:**

1. Verify no second workflow/readiness authority was introduced.
2. Verify every Work semantic tone is derived from typed state or explicit closed booleans, never display strings.
3. Verify ActionBar primary ownership phase by phase.
4. Verify Review remains fail-closed for unavailable execution evidence.
5. Verify manual handoff and recovery semantics remain truthful.
6. Verify no new feature CSS file and no feature CSS growth.
7. Verify `WorkTab.tsx` does not cross the 28 KiB source limit and ideally shrinks through extraction into `workSemantic.ts`/focused components.

**Exact-head verification:**

- `node --test scripts/*.test.mjs`
- architecture ratchet workflow
- frontend build
- full Playwright E2E
- Rust fmt / Clippy / tests / coverage
- gitleaks / cargo-deny
- native Tauri/WebDriverIO E2E

Only mark the PR ready and squash merge when all exact-head checks are green.

---

## Definition of done

- Work is the second owning surface using the shared semantic primitive layer end-to-end for its operational state.
- Phase, execution packet, approval, review evidence, loading/error, and primary action semantics are typed and consistent with Changes.
- One-primary-action ownership is visible and test-protected in representative Work phases.
- Review evidence remains fail-closed and clearly distinguishes rerun vs evidence repair.
- Work visual debt is flat or lower; no new visual generation is introduced.
- Full exact-head CI and native E2E are green before squash merge.