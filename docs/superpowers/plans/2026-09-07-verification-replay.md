# Verification Replay Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Explain whether the active verification receipt is reusable against the current ChangeSet and make the next verification action explicit.

**Architecture:** Add a pure Rust replay projection beside existing Change governance. It compares canonical receipt facts with current Git/ChangeSet facts, returns explicit unknown/stale states, and is serialized through the existing engineering snapshot. The Changes UI renders the projection and reuses the existing verification mutation.

**Tech Stack:** Rust workspace, serde, existing workflow receipt/tree helpers, Tauri engineering snapshot, React/TypeScript, TanStack Query, Playwright.

**Spec:** `docs/superpowers/specs/2026-09-07-verification-replay-design.md`

## Global Constraints

- Replay is read-only and never executes a verification command.
- Exact tree and ChangeSet identity are required for a positive `current` state.
- Missing facts remain explicit; no inferred green state.
- Existing `work_engineering_intelligence` remains the only IPC boundary for this projection.
- Existing `work_verify` remains the only verification execution path.
- No new database, provider, state-management library, or top-level route.

---

### Task 1: Add the deterministic replay domain projection

**Files:**
- Create: `crates/repodesk-core/src/engineering/verification_replay.rs`
- Modify: `crates/repodesk-core/src/engineering/mod.rs`
- Modify: `crates/repodesk-core/src/engineering/change_governance.rs`

**Interfaces:**
- Consumes: `TaskRunReceipt`, current HEAD/index tree, current ChangeSet digest, and existing `ChangeVerificationEvidence`.
- Produces: `VerificationReplay` and `derive_verification_replay` for the Tauri snapshot and UI.

- [ ] **Step 1: Write failing tests** for current exact proof, stale tree, stale ChangeSet, missing receipt, and failed command evidence.
- [ ] **Step 2: Run the focused replay tests** and confirm they fail because the projection does not exist.
- [ ] **Step 3: Implement the enums, DTO, reason codes, and pure derivation function.** Keep reason codes stable snake_case and preserve optional identities.
- [ ] **Step 4: Add `verification_replay` to `ChangeGovernanceSnapshot` and derive it from the same receipt/tree inputs used by the live governance reconciliation.**
- [ ] **Step 5: Run focused Rust tests and formatting.**

### Task 2: Expose replay through the existing desktop snapshot

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/engineering.rs`
- Modify: `apps/desktop/src/shared/api/engineering.ts`
- Modify: `apps/desktop/e2e/changes-design-system.spec.ts`

**Interfaces:**
- Consumes: `ChangeGovernanceSnapshot.verification_replay`.
- Produces: typed frontend replay state and fixture coverage.

- [ ] **Step 1: Extend the TypeScript DTO and fixtures with replay fields.**
- [ ] **Step 2: Add Playwright assertions for current and stale replay evidence.**
- [ ] **Step 3: Run the focused Playwright scenarios and TypeScript build.**

### Task 3: Render the replay inspector in Changes

**Files:**
- Modify: `apps/desktop/src/features/changes/ChangeGovernancePanel.tsx`
- Modify: `apps/desktop/src/features/changes/changesSemantic.ts`
- Modify: `apps/desktop/src/features/changes/changes-route.css`

**Interfaces:**
- Consumes: typed replay state and existing `workVerify` mutation.
- Produces: compact accessible replay state, reason, identities, and rerun CTA.

- [ ] **Step 1: Add semantic mapping tests/fixture assertions for each replay state.**
- [ ] **Step 2: Render the replay section with no duplicated verification authority.**
- [ ] **Step 3: Reuse the existing Verify action for `can_rerun` and keep command execution explicit.**
- [ ] **Step 4: Run focused Playwright and frontend build checks.**

### Task 4: Full verification and integration

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Run Rust focused tests, workspace check, formatting, and targeted Playwright.**
- [ ] **Step 2: Run `./scripts/verify-all.sh` on the feature worktree.**
- [ ] **Step 3: Commit the implementation with `feat: explain verification receipt replay`.**
- [ ] **Step 4: Merge the verified branch into `main`, rerun the full gate on merged `main`, push `origin/main`, and clean up the feature worktree.**
