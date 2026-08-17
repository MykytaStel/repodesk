# Code Design-System Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Converge the Code shell, typed engineering state, route states, explorer file status, and repository-intelligence evidence on the shared Cut F semantic primitives while preserving editor-first geometry and all Code domain behavior.

**Architecture:** `CodeTab.tsx` remains the orchestration owner but pure tab/session helpers move to `codeTabs.ts` so the route falls below 28 KiB. One exhaustive `codeSemantic.ts` adapter maps Code/domain enums into `SemanticState`; `CodeSemanticStrip.tsx`, `CodeWorkspaceTree.tsx`, `CodeTab.tsx`, and `RepositoryIntelligenceDrawer.tsx` consume the shared primitive boundary without redesigning CodeMirror or IDE chrome.

**Tech Stack:** React + TypeScript, TanStack Query, CodeMirror 6, Playwright, Node architecture ratchet, Tauri/Rust native E2E.

## Global Constraints

- Preserve the approved Cut F dependency direction: shared APIs -> Code-local typed adapter -> shared primitives -> Code surfaces.
- Do not change backend/Rust behavior, CodeMirror construction, LSP/live-language behavior, draft recovery, project search algorithms, or file mutation semantics.
- Keep the Code surface editor-like; do not wrap the editor in dashboard-card composition.
- Use only the existing `SemanticTone` vocabulary: `positive | attention | critical | neutral | info`.
- New typed variants must force an explicit mapping through exhaustive switches; no status substring inference and no `statusTone()` in migrated Code surfaces.
- No new feature-local CSS file, no feature CSS growth, no `*-vN`, no `*-polish.css`, no new raw TSX hex, and no new static inline layout geometry.
- Preserve existing data-driven inline geometry for Code tree virtualization and pointer-positioned editor previews.
- Rename the active route shell from `code-workspace-v0` to canonical `code-workspace`; do not create another visual generation.
- Bring `CodeTab.tsx` to at most 28 KiB.
- Keep `code-editor-polish.css` for the final Cut F historical CSS/dead-class cleanup unless this slice proves an active import/consumer.
- Preserve all Changes/Work/Runs/Projects architecture ratchets and the Work visual-debt cleanup contract.
- Exact-head Architecture Ratchet, full CI, Playwright, coverage/security/supply-chain, and native Tauri/WebDriverIO must be green before squash merge.

---

### Task 1: RED Code semantic and architecture contracts

**Files:**
- Create: `apps/desktop/e2e/code-design-system.spec.ts`
- Modify: `scripts/check-source-architecture.mjs`
- Modify: `scripts/design-system-ratchet.test.mjs`

**Interfaces:**
- Consumes: existing `evaluateTypedSemanticContract(...)`, current Code route and fixture commands.
- Produces: `evaluateCodeSemanticContract()` plus E2E expectations that define the required Code semantic boundary before production migration.

- [ ] **Step 1: Extend the architecture ratchet with a Code contract**

Add constants:

```js
const CODE_SEMANTIC_ADAPTER = "apps/desktop/src/features/code/codeSemantic.ts";
const CODE_TYPED_SURFACES = [
  "apps/desktop/src/features/code/CodeTab.tsx",
  "apps/desktop/src/features/code/CodeWorkspaceTree.tsx",
  "apps/desktop/src/features/code/CodeSemanticStrip.tsx",
  "apps/desktop/src/features/code/RepositoryIntelligenceDrawer.tsx",
];
```

Add:

```js
export function evaluateCodeSemanticContract() {
  const failures = evaluateTypedSemanticContract({
    label: "Code",
    adapterPath: CODE_SEMANTIC_ADAPTER,
    adapterImport: "./codeSemantic",
    typedSurfaces: CODE_TYPED_SURFACES,
  });

  const tab = readSource("apps/desktop/src/features/code/CodeTab.tsx");
  if (tab && /code-workspace-v\d+/i.test(tab)) {
    failures.push("apps/desktop/src/features/code/CodeTab.tsx: canonical Code shell must not use a versioned class name");
  }
  const tree = readSource("apps/desktop/src/features/code/CodeWorkspaceTree.tsx");
  if (tree && /\bstatusTone\s*\(/.test(tree)) {
    failures.push("apps/desktop/src/features/code/CodeWorkspaceTree.tsx: typed file state must use codeSemantic.ts instead of statusTone()");
  }
  return failures;
}
```

Call it from `runArchitectureRatchet()` after Projects.

- [ ] **Step 2: Add the ratchet unit assertion**

Import and assert:

```js
test("Code migration requires one typed adapter and the shared primitive boundary", () => {
  assert.deepEqual(evaluateCodeSemanticContract(), []);
});
```

This is intentionally RED because `codeSemantic.ts` and `CodeSemanticStrip.tsx` do not yet exist and current Code surfaces do not consume the primitive boundary.

- [ ] **Step 3: Add representative Playwright RED coverage**

Create `code-design-system.spec.ts` using `currentOnboardedFixtures` + `installMockIpc`. Cover these externally-visible contracts:

```ts
test("no project uses a surface-scoped semantic empty state", ...);
test("workspace loading and authority failure use semantic surface states", ...);
test("normal Code workspace uses canonical shell and typed index status", ...);
test("Explorer file status uses semantic tone", ...);
test("active-file scope and verification state use typed semantic badges", ...);
```

Use fixture overrides for `desktop_snapshot`, `get_active_project_config`, `code_workspace_snapshot`, `code_workspace_read`, and `work_engineering_intelligence`. Assert `data-semantic-tone`, roles/text, `.code-workspace`, and absence of `.code-workspace-v0`, not pixel values.

- [ ] **Step 4: Verify RED through the PR workflows**

Expected Architecture failure includes missing `codeSemantic.ts`, missing `CodeSemanticStrip.tsx`, primitive-boundary failures, and versioned Code shell. Expected Playwright failures show legacy route/semantic rendering.

- [ ] **Step 5: Commit and open a draft PR**

Commit message:

```text
test(code): define semantic convergence contracts
```

Draft PR title:

```text
refactor(ui): converge Code on semantic primitives
```

The PR body records the intentional RED state and links this plan/design.

---

### Task 2: Typed Code semantics and source-budget extraction

**Files:**
- Create: `apps/desktop/src/features/code/codeSemantic.ts`
- Create: `apps/desktop/src/features/code/codeTabs.ts`
- Create: `apps/desktop/src/features/code/CodeSemanticStrip.tsx`
- Modify: `apps/desktop/src/features/code/CodeTab.tsx`
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/CodeWorkspaceTree.tsx`

**Interfaces:**
- Consumes: `CodeWorkspaceFileStatus`, `ChangeFileScopeState`, `ChangeReviewState`, `ChangeVerificationState`, `SemanticOrigin`, `RepositoryEvidenceLevel`, and shared `SemanticState`.
- Produces: `codeFileStatusSemantic`, `codeScopeSemantic`, `codeReviewSemantic`, `codeVerificationSemantic`, `codeOriginSemantic`, `repositoryEvidenceSemantic`, `codeWorkspaceIndexSemantic`, `codeSaveSemantic`; pure tab/session helpers; `CodeSemanticStrip`.

- [ ] **Step 1: Implement exhaustive Code semantic mappings**

`codeSemantic.ts` must contain an `assertNever` guard and explicit switches. Required behavior:

```ts
clean       -> neutral / "Clean"
modified    -> attention / "M"
added       -> positive / "A"
deleted     -> critical / "D"
untracked   -> neutral / "U"
renamed     -> info / "R"
conflict    -> critical / "!"

allowed     -> positive / "In scope"
out_of_scope-> critical / "Out of scope"
protected   -> critical / "Protected"
ungoverned  -> attention / "Ungoverned"

accepted    -> positive / "Accepted"
rejected    -> critical / "Rejected"
proposed    -> attention / "Proposed"

passed + clean draft -> positive / "Verified"
passed + dirty draft -> attention / "Draft after verification"
failed      -> critical / "Verification failed"
running     -> info / "Verifying"
not_run     -> neutral / "Not verified"

repository evidence strong -> positive
bounded -> attention
unavailable -> neutral

workspace index complete -> positive / "Indexed"
workspace index truncated -> attention / "Index capped"
```

- [ ] **Step 2: Extract pure tab/session helpers from `CodeTab.tsx`**

Move `EditorTab`, `CachedCodeSession`, cache constants/map, ID helpers, clone/session-memory helpers, document-to-tab conversion, and `fileName()` into `codeTabs.ts`. Keep mutation/query/orchestration state in `CodeTab.tsx`.

Export only the helpers actually consumed by `CodeTab.tsx`:

```ts
export type EditorTab = ...;
export function workspaceTabId(...): string;
export function libraryTabId(...): string;
export function rememberCodeSession(...): void;
export function restoreCodeSession(project: string): { tabs: EditorTab[]; activeTabId: string | null } | null;
export function toWorkspaceTab(...): EditorTab;
export function toLibraryTab(...): EditorTab;
export function fileName(path: string): string;
```

The extraction must make `CodeTab.tsx` <= 28 KiB without changing behavior.

- [ ] **Step 3: Extract the semantic strip**

Create `CodeSemanticStrip.tsx`. It receives:

```ts
export function CodeSemanticStrip({
  semantic,
  dirty,
}: {
  semantic: SemanticFileState;
  dirty: boolean;
})
```

Use `StatusBadge` for typed scope, review, verification, origin, problem counts, and Git-state facts where present. Preserve concise editor density; no panels/cards. Remove the local `SemanticStrip` function from `SemanticCodeEditor.tsx` and render `<CodeSemanticStrip semantic={semantic} dirty={dirty} />`.

- [ ] **Step 4: Migrate Explorer typed file status**

Remove the local `STATUS_LABEL` and `statusTone()` from `CodeWorkspaceTree.tsx`. Use:

```tsx
const status = codeFileStatusSemantic(file.status);
...
<StatusBadge
  label={status.label}
  tone={status.tone}
  ariaLabel={status.detail ?? status.label}
  className="code-tree-status"
/>
```

Do not change virtualization, dynamic row indentation/height, blocked-file policy, context menu, or tree navigation.

- [ ] **Step 5: Run architecture and Code E2E verification**

Architecture should move from missing adapter/primitive failures toward only remaining shell/route-state failures. Existing editor/virtualization tests must remain compatible.

- [ ] **Step 6: Commit**

Commit message:

```text
refactor(code): add typed semantic boundary
```

---

### Task 3: Code shell, route states, and technical inspector migration

**Files:**
- Modify: `apps/desktop/src/features/code/CodeTab.tsx`
- Modify: `apps/desktop/src/features/code/code-workspace.css`
- Modify: `apps/desktop/src/features/code/RepositoryIntelligenceDrawer.tsx`

**Interfaces:**
- Consumes: Task 2 adapter/helpers and shared `StatusBadge`, `EvidenceState`, `LoadingState`, `EmptyState`, `ErrorState`.
- Produces: canonical `.code-workspace` shell and semantic route/local state presentation.

- [ ] **Step 1: Replace route authority states with shared states**

Use:

```tsx
if (!hasProject) {
  return <EmptyState scope="surface" message="Connect a project to open the Code workspace." />;
}
if (workspace.isLoading) {
  return <LoadingState scope="surface" message="Indexing repository files…" />;
}
if (workspace.isError || !workspace.data) {
  return <ErrorState scope="surface" title="Code workspace unavailable" detail={errorToMessage(workspace.error)} />;
}
```

- [ ] **Step 2: Canonicalize shell and toolbar semantic status**

Change `code-workspace-v0` -> `code-workspace` in TSX and `code-workspace.css`.

Use compact `StatusBadge` instances in the existing dense toolbar for:

- workspace index state from `codeWorkspaceIndexSemantic(workspace.data.truncated)`;
- unsaved state from `codeSaveSemantic(dirtyCount > 0 ? "dirty" : "saved")` only when it adds useful state, keeping file count as plain metadata.

Keep repository context/analyze/findings/review controls in the existing accessible icon toolbar as secondary actions.

- [ ] **Step 3: Migrate local errors/warnings without changing ownership**

`draftError` becomes an attention `EvidenceState` or equivalent compact semantic state with its dismiss action adjacent.

`workspaceError` becomes an inline `ErrorState`. Preserve `Reload from disk` when the exact conflict message indicates the file changed outside RepoDesk, plus Dismiss. Do not infer a semantic tone from the error string; the string check is only recovery-action eligibility for an existing backend error contract.

- [ ] **Step 4: Migrate document empty/diff states**

Inside `.code-document-stage`:

- no active tab -> inline `EmptyState` preserving the editor-budget hint;
- diff loading -> inline `LoadingState`;
- empty diff -> inline `EmptyState`.

Do not alter `DiffViewer` or `SemanticCodeEditor` geometry.

- [ ] **Step 5: Migrate repository-intelligence evidence states**

Use shared `LoadingState` and `ErrorState` for query lifecycle. Map `focus.graph_evidence.level` through `repositoryEvidenceSemantic(...)` and render `StatusBadge`. Preserve the existing drawer/section geometry and all navigation/data behavior.

- [ ] **Step 6: Verify CSS/source budgets**

`code-workspace.css` must stay flat or shrink. `CodeTab.tsx` must be <= 28 KiB. No new feature-local CSS files may exist.

- [ ] **Step 7: Commit**

Commit message:

```text
refactor(code): converge shell and states
```

---

### Task 4: Exact-head verification and squash merge

**Files:**
- Review all PR files; no new production scope unless verification reveals a concrete regression.

**Interfaces:**
- Consumes: completed Code semantic migration.
- Produces: verified Code Cut F slice on `main`.

- [ ] **Step 1: Run/fetch exact-head Architecture Ratchet**

Require `conclusion: success` for the exact PR head.

- [ ] **Step 2: Run/fetch exact-head full CI**

Require success for:

- frontend build;
- cargo fmt;
- Clippy;
- Rust tests;
- complete Playwright mock-IPC suite;
- coverage;
- cargo-deny;
- gitleaks;
- strict secret scan.

- [ ] **Step 3: Run/fetch exact-head native E2E**

Require release build + tauri-driver/WebDriverIO smoke success.

- [ ] **Step 4: Final diff review**

Verify:

- no backend/Rust changes;
- no CodeMirror/LSP/draft/file-mutation behavior rewrite;
- `CodeTab.tsx` <= 28 KiB;
- no `.code-workspace-v0` consumer;
- one `codeSemantic.ts` adapter;
- shared primitive boundary on contracted Code surfaces;
- no new CSS file or CSS growth;
- no unresolved review threads;
- `main` has not moved incompatibly.

- [ ] **Step 5: Update PR evidence and mark ready**

Record exact head SHA and all workflow run IDs in the PR body.

- [ ] **Step 6: Squash merge using the verified expected head SHA**

Merge method: `squash`. Commit title:

```text
refactor(ui): converge Code on semantic primitives
```

Stop after Code. The next Cut F slice is final historical CSS/dead-class cleanup and remaining ratchet-baseline removal.
