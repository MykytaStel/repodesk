# Runs Design-System Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the Runs owning surface from ad-hoc string/class status presentation onto the shared typed semantic primitive layer without changing run evidence, observability, acceptance-linking, Outcomes, or Audit authority.

**Architecture:** `HistoryTab.tsx` remains the route owner, but presentation semantics move into a pure `runsSemantic.ts` adapter backed by typed API unions. The route consumes shared `StatusBadge`, `EvidenceState`, `PanelHeader`, `Metric`, `EmptyState`, `LoadingState`, `ErrorState`, and `ActionBar` primitives. Where the API currently exposes evidence state as plain `string`, tighten the TypeScript contract to the actual closed states already emitted by the backend before mapping tone; do not add a display-string classifier.

**Tech Stack:** React 18, TypeScript, TanStack Query, shared semantic primitives, Playwright mock IPC, Node architecture ratchet, existing Tauri/Rust backend unchanged.

## Global Constraints

- Approved Cut F migration order is `Changes -> Work -> Runs -> Projects -> Code`.
- Preserve Runs ownership: immutable execution/evidence inspection lives here; mutable review/commit policy remains in Work/Changes/Projects.
- Do not alter backend evidence generation, event journals, trust policy, routing, execution, or verification behavior.
- No generic `statusTone(value: string)` or substring/status-text inference.
- Unknown/legacy evidence must stay visibly neutral/attention, never be upgraded to positive by wording.
- Run status `completed` is positive, `partial` attention, `failed` critical, `dry_run` neutral.
- Worker status `ok` positive, `skipped` neutral, `blocked` critical, `failed` critical.
- Run disposition `complete` positive, `ready` info, `attention` attention, `blocked` critical.
- Verification command success/failure is typed Boolean evidence and may map directly to positive/critical.
- Acceptance `proven` positive, `unproven` attention, `failed` critical; stale evidence is attention/critical context and must remain explicit.
- Keep acceptance linking as a local secondary mutation; Runs must not gain a commit/review primary action.
- No new `*-vN`, `*-polish.css`, route-wide visual generation, static inline layout style, raw hex, or feature-CSS growth.
- Existing feature CSS may only stay flat or shrink; reusable styling belongs under `shared/ui/primitives`.
- Preserve existing run selection, refresh, lazy Outcomes/Audit tabs, and query cache behavior.
- Preserve the merged Work visual-debt cleanup ratchet from #196 while extending the generic semantic contract to Runs.
- Exact-head Architecture Ratchet, full CI, Playwright, security/supply-chain checks, coverage, and native E2E must be green before squash merge.

---

### Task 1: RED — lock typed Runs semantic contracts

**Files:**
- Create: `apps/desktop/e2e/runs-design-system.spec.ts`
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`

**Interfaces:**
- Consumes: existing `HistoryTab.tsx`, shared primitive barrel `../../shared/ui/primitives`.
- Produces: architecture requirement for `apps/desktop/src/features/history/runsSemantic.ts` and semantic Playwright expectations used by later tasks.

- [ ] Add failing architecture contract requiring the Runs adapter and shared primitive imports, while preserving Changes, Work, and Work visual cleanup contracts.
- [ ] Add Playwright RED cases for run/disposition/worker/verification/acceptance/commit states and accessible loading/error/empty states.
- [ ] Record RED evidence before production migration.

### Task 2: Tighten evidence state TypeScript contracts

**Files:**
- Modify: `apps/desktop/src/shared/api/engineering.ts`

- [ ] Add closed `RunReviewState` and `RunVerificationState` unions matching backend-emitted values.
- [ ] Replace plain string state fields without changing backend serialization.
- [ ] Build frontend; do not cast away compile failures.

### Task 3: GREEN — add exhaustive `runsSemantic.ts`

**Files:**
- Create: `apps/desktop/src/features/history/runsSemantic.ts`

- [ ] Add O(1), exhaustive semantic mappings for run, worker, review, verification, acceptance, disposition, commit, and command result states.
- [ ] Stale acceptance evidence overrides prior positive proof and renders attention.
- [ ] Unknown/legacy evidence never maps positive.

### Task 4: GREEN — migrate route/list states and shell

**Files:**
- Modify: `apps/desktop/src/features/history/HistoryTab.tsx`

- [ ] Remove local `statusTone(value: string)`.
- [ ] Migrate loading/error/empty states and route/detail headers to shared primitives.
- [ ] Remove the static inline list-header margin without adding feature CSS.
- [ ] Preserve Run evidence / Provider outcomes / Raw audit tabs, selection, refresh, lazy loading, and query keys.

### Task 5: GREEN — migrate immutable run evidence and observability semantics

**Files:**
- Modify: `apps/desktop/src/features/history/HistoryTab.tsx`

- [ ] Render disposition, run, worker, review, verification, command, acceptance, and commit semantics through typed adapter + shared primitives.
- [ ] Use shared `Metric` for operational counts/ratios while preserving timestamps, hashes, paths, evidence refs, and source labels.
- [ ] Keep empty technical evidence neutral, never successful-looking.

### Task 6: Preserve acceptance linking as secondary evidence maintenance

**Files:**
- Modify: `apps/desktop/src/features/history/HistoryTab.tsx`

- [ ] Keep `linkAcceptanceEvidenceBundle` mutation local to acceptance evidence.
- [ ] Surface link mutation failure without discarding the existing evidence snapshot.
- [ ] Keep stale verification fail-closed: no new proof linking until verification reruns.

### Task 7: Ratchet and exact-head verification

**Files:**
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`

- [ ] Extend the generic typed semantic contract to Runs while retaining #196 Work visual ownership checks.
- [ ] Ensure Runs cannot reintroduce `statusTone()` or display-string inference.
- [ ] Self-review source size, action ownership, unknown-state semantics, and CSS debt.
- [ ] Require exact-head Architecture Ratchet, frontend build, cargo fmt, Clippy, Rust tests, coverage, Playwright, gitleaks, cargo-deny, and native Tauri/WebDriverIO E2E.
- [ ] Update PR evidence, mark ready, and squash merge with expected head SHA.