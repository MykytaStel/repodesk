# Projects CI Completion Design

## Context

PR #197 already contains the approved Projects Cut F semantic migration. Its current exact head keeps Architecture Ratchet and native Tauri/WebDriverIO green, while the full CI fails only in Playwright mock-IPC coverage.

The failing assertions both target `No active project` with an unscoped page-level text locator. The migrated UI legitimately renders that same user-facing fact in two ownership regions:

- the application chrome breadcrumb (`Current workspace location`), and
- the Projects semantic status inside the main workspace.

Playwright strict mode therefore resolves two elements and fails before it can inspect `data-semantic-tone`.

## Root cause

This is a test-locator ownership regression, not a Projects domain or semantic-state regression.

`projectsSemantic.ts` maps the inactive workspace state to `{ label: "No active project", tone: "neutral" }`, and `ProjectsTab.tsx` renders that state through the shared `StatusBadge`. The duplicate text in the shell is expected and should remain.

## Design

Keep production behavior unchanged. Repair the two failing Playwright assertions by scoping the semantic-state lookup to the `main` workspace region, where Projects owns the `StatusBadge`.

Preferred locator shape:

```ts
page.getByRole("main").getByText("No active project", { exact: true })
```

This preserves the user-visible duplicate fact across chrome and route content while making the test assert the correct ownership boundary instead of relying on global text uniqueness.

No test-only IDs, CSS selectors, production wrappers, status-label renames, or route behavior changes are required.

## Scope constraints

- Do not change `projectsSemantic.ts` for this CI failure.
- Do not remove or rename the breadcrumb's `No active project` fact.
- Do not add production markup solely to satisfy Playwright.
- Do not modify backend/Rust behavior, project activation, exact-attribution policy authority, Knowledge, or Work templates.
- Preserve all Changes, Work, Runs, Projects and Work visual-debt architecture ratchets.
- Do not start Code convergence in this PR.

## Verification and merge gate

After the locator repair:

1. Require exact-head Architecture Ratchet green.
2. Require exact-head full CI green, including the complete Playwright mock-IPC suite, frontend build, fmt, Clippy, Rust tests, coverage, cargo-deny, gitleaks and strict secret scan.
3. Require exact-head native Tauri/WebDriverIO E2E green.
4. Review the final PR diff for scope creep and semantic/action-ownership regressions.
5. Mark #197 ready and squash-merge with the verified expected head SHA.

Projects is complete only after these exact-head gates pass. Code remains the next separate Cut F slice.
