# Projects CI Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish PR #197 by repairing the two Playwright locator regressions without changing Projects production behavior, then verify and squash-merge the exact head.

**Architecture:** The Projects semantic migration is already implemented and its production state is correct. The only failure is test ownership: `No active project` is intentionally rendered both in app chrome and in the Projects workspace. Repair the tests by scoping the semantic assertion to the `main` landmark, preserving both user-visible facts and all production semantics.

**Tech Stack:** React, TypeScript, Playwright, GitHub Actions, Tauri/WebDriverIO.

## Global Constraints

- Do not change `projectsSemantic.ts` for this CI failure.
- Do not remove or rename the breadcrumb's `No active project` fact.
- Do not add production markup solely to satisfy Playwright.
- Do not modify backend/Rust behavior, project activation, exact-attribution policy authority, Knowledge, or Work templates.
- Preserve all Changes, Work, Runs, Projects and Work visual-debt architecture ratchets.
- Do not start Code convergence in this PR.
- Require exact-head Architecture Ratchet, full CI, Playwright, coverage/security/supply-chain, and native Tauri/WebDriverIO green before squash merge.

---

### Task 1: Repair Projects inactive-state Playwright ownership

**Files:**
- Modify: `apps/desktop/e2e/project-switching.spec.ts`
- Modify: `apps/desktop/e2e/projects-design-system.spec.ts`

**Interfaces:**
- Consumes: the existing Projects `StatusBadge` text `No active project` rendered inside the `main` landmark.
- Produces: tests that assert the Projects-owned inactive semantic state without assuming global text uniqueness.

- [ ] **Step 1: Preserve the existing failing evidence**

Use the already-recorded exact-head CI failure from head `aac71a442342ce9493c910fa9fd50244d2bfc8dc`, where Playwright strict mode resolves two `No active project` elements. No new RED production change is needed because the failing CI is the reproduction.

- [ ] **Step 2: Replace only the two global inactive-state locators**

In each failing assertion, replace the page-level text lookup:

```ts
page.getByText("No active project", { exact: true })
```

with the ownership-scoped lookup:

```ts
page.getByRole("main").getByText("No active project", { exact: true })
```

Do not change neighboring expectations, semantic tone assertions, production components, or fixture behavior.

- [ ] **Step 3: Commit the locator repair**

Commit only the two test-file edits with message:

```text
test(projects): scope inactive state to workspace
```

### Task 2: Exact-head verification and merge

**Files:**
- Modify: PR #197 metadata only after verification.

**Interfaces:**
- Consumes: Task 1 exact head.
- Produces: a verified, review-ready Projects migration merged into `main` by squash.

- [ ] **Step 1: Verify Architecture Ratchet**

Require the exact Task 1 head to have a completed successful `Architecture Ratchet` workflow.

- [ ] **Step 2: Verify full CI**

Require the exact Task 1 head to have a completed successful `CI` workflow, including frontend build, cargo fmt, Clippy, Rust tests, strict secret scan, complete Playwright mock-IPC suite, coverage, cargo-deny, and gitleaks.

- [ ] **Step 3: Verify native E2E**

Require the exact Task 1 head to have a completed successful `E2E (native)` workflow with the Tauri/WebDriverIO smoke path.

- [ ] **Step 4: Self-review final diff**

Confirm the only post-spec behavioral change is the two Playwright locator scopes. Confirm Projects production semantics, action ownership, attribution policy truth, lazy views, and architecture ratchets are unchanged.

- [ ] **Step 5: Update PR evidence and mark ready**

Update #197 with the root-cause evidence, exact verified head, and green workflow IDs. Mark it ready for review only after all three exact-head workflow families are green.

- [ ] **Step 6: Squash merge with expected head**

Squash-merge #197 into `main` using the exact verified head SHA as the expected head. Verify the PR reports `merged=true` and fetch `main` afterward to confirm the merge commit is current.
