# Multi-language Intelligence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Rust-only live language session with safe, installable Rust, TypeScript/JavaScript, TOML, JSON, and YAML intelligence presented through the RepoDesk Native language-tool UI.

**Architecture:** A registry-owned profile contract selects a generic per-project, per-server JSON-RPC session. Read-only library documents and managed tool installation remain separate security boundaries. The frontend consumes capability-driven status through a generic hook and focused pill, popover, preview, and library-tab components.

**Tech Stack:** Rust 2024, Tauri 2, serde/JSON-RPC over stdio, React 18, TypeScript, CodeMirror 6, TanStack Query, Playwright.

## Global Constraints

- Work on the current `main` branch as requested.
- Finish every task with focused tests, one scoped commit, and `git push origin main` before starting the next task.
- Preserve unrelated work and never rewrite or force-push history.
- Language-server and installer processes are argv-only; never use `sh -c`.
- Never allow `workspace/applyEdit` or writable library documents.
- Managed tools live below `REPODESK_HOME/tools/language-servers` and never mutate the active repository or global package-manager state.
- Fully active first-wave profiles are Rust, TypeScript/JavaScript, TOML, JSON, and YAML. Other registered profiles remain discovery-only.
- Use TDD: write and observe each focused test failing before production implementation.

---

### Task 0: Publish the completed editor-navigation baseline

**Files:**
- Commit: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Commit: `apps/desktop/src/features/code/semantic-code-editor.css`
- Commit: `apps/desktop/src/features/code/useLiveRustLanguage.tsx`
- Commit: `apps/desktop/src/shared/api/codeWorkspace.ts`
- Commit: `apps/desktop/e2e/editor-ui.spec.ts`
- Commit: `apps/desktop/e2e/mock-ipc.ts`
- Commit: `docs/superpowers/specs/2026-08-09-editor-definition-navigation-design.md`
- Commit: `docs/superpowers/plans/2026-08-09-editor-definition-navigation.md`

**Interfaces:**
- Produces: range-aware `CodeWorkspaceLocation`, modifier-hover preview, modifier-click definition navigation, and transient target reveal.
- Consumes: existing Rust-only language session; no multi-language contracts yet.

- [ ] **Step 1: Re-run the completed feature gate**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts
pnpm --dir apps/desktop run build
git diff --check
```

Expected: editor tests and build pass; diff check produces no output.

- [ ] **Step 2: Commit and push only the editor-navigation baseline**

```bash
git add apps/desktop/src/features/code/SemanticCodeEditor.tsx \
  apps/desktop/src/features/code/semantic-code-editor.css \
  apps/desktop/src/features/code/useLiveRustLanguage.tsx \
  apps/desktop/src/shared/api/codeWorkspace.ts \
  apps/desktop/e2e/editor-ui.spec.ts apps/desktop/e2e/mock-ipc.ts \
  docs/superpowers/specs/2026-08-09-editor-definition-navigation-design.md \
  docs/superpowers/plans/2026-08-09-editor-definition-navigation.md
git commit -m "feat(editor): add IDE-style definition navigation"
git push origin main
```

Expected: one pushed commit; the new multi-language spec and plan remain uncommitted.

### Task 1: Publish and lock the multi-language contract

**Files:**
- Commit: `docs/superpowers/specs/2026-08-10-multi-language-intelligence-design.md`
- Commit: `docs/superpowers/plans/2026-08-10-multi-language-intelligence.md`

**Interfaces:**
- Produces: approved architecture, security boundaries, commit sequence, and gates.
- Consumes: Task 0's published editor baseline.

- [ ] **Step 1: Validate, commit, and push the documents**

```bash
rg -n "[T]BD|[T]ODO|[i]mplement later|[f]ill in details" \
  docs/superpowers/specs/2026-08-10-multi-language-intelligence-design.md \
  docs/superpowers/plans/2026-08-10-multi-language-intelligence.md
git diff --check
git add docs/superpowers/specs/2026-08-10-multi-language-intelligence-design.md \
  docs/superpowers/plans/2026-08-10-multi-language-intelligence.md
git commit -m "docs(language): specify multi-language intelligence"
git push origin main
```

Expected: placeholder scan has no matches, diff check passes, and the documentation commit is pushed.

### Task 2: Make the registry profile-driven

**Files:**
- Modify: `crates/repodesk-core/src/language_intelligence.rs`
- Test: unit tests in `crates/repodesk-core/src/language_intelligence.rs`

**Interfaces:**
- Produces: `LanguageServerProfileState::{Active, DiscoveryOnly}`, `LanguageServerInitializationProfile::{Default, Taplo}`, and optional `install_recipe_id`.
- Produces: active profiles for `rust-analyzer`, `typescript-language-server`, `taplo`, `json-language-server`, and `yaml-language-server`.
- Consumes: executable discovery and `preferred_server_for_language`.

- [ ] **Step 1: Write failing literal registry tests**

Add tests asserting the five exact active ids, discovery-only ids, Taplo's initialization profile, TypeScript's two language ids, and recipe ids.

- [ ] **Step 2: Verify RED**

```bash
cargo test -p repodesk-core language_intelligence::tests::registry_marks_first_wave_profiles_active -- --exact
```

Expected: FAIL because profile state and initialization profile do not exist.

- [ ] **Step 3: Add the serialized profile contract**

Extend `ServerSpec` and `LanguageServerDescriptor` with concrete enums and recipe ids. Executable and argv remain backend-owned.

- [ ] **Step 4: Verify, commit, and push**

```bash
cargo test -p repodesk-core language_intelligence
cargo clippy -p repodesk-core --all-targets --all-features -- -D warnings
git diff --check
git add crates/repodesk-core/src/language_intelligence.rs
git commit -m "feat(language): add server execution profiles"
git push origin main
```

Expected: focused tests and Clippy pass before push.

### Task 3: Generalize the language-session manager

**Files:**
- Create: `apps/desktop/src-tauri/src/language_server/{mod.rs,session.rs,protocol.rs,profiles.rs,errors.rs}`
- Modify: `apps/desktop/src-tauri/src/commands/language_intelligence.rs`
- Remove after migration: `apps/desktop/src-tauri/src/language_server.rs`
- Test: focused unit tests in the new module files

**Interfaces:**
- Produces: sessions keyed by `SessionKey { project, server_id }`.
- Produces: generic `sync_document`, `hover`, `definition`, `references`, `symbols`, `close_document`, `status`, `restart`, and `stop_all`.
- Consumes: Task 2's profile, executable, argv, and initialization profile.

- [ ] **Step 1: Write failing manager tests**

Use fake stdio fixtures to prove Rust and TypeScript coexist, two TypeScript documents reuse one process, closing one preserves the other, project changes stop stale sessions, and one crash leaves the other ready.

- [ ] **Step 2: Verify RED**

```bash
cargo test -p repodesk-desktop language_server::tests::keeps_sessions_isolated_by_server -- --exact
```

Expected: FAIL because the manager owns one Rust session.

- [ ] **Step 3: Extract generic transport and profile initialization**

Move framing, pending requests, bounded stderr, diagnostics, URI conversion, and workspace-edit rejection into focused modules. Replace Rust-specific validation and error text with descriptor-driven values.

- [ ] **Step 4: Route first-wave document actions through the manager**

Session selection derives from document language in the backend. The frontend cannot select executable or argv.

- [ ] **Step 5: Verify, commit, and push**

```bash
cargo test -p repodesk-desktop language_server
cargo clippy -p repodesk-desktop --all-targets --all-features -- -D warnings
git diff --check
git add apps/desktop/src-tauri/src/language_server \
  apps/desktop/src-tauri/src/commands/language_intelligence.rs \
  apps/desktop/src-tauri/src/language_server.rs
git commit -m "feat(language): support concurrent server sessions"
git push origin main
```

Expected: session tests and Clippy pass; the old monolithic module is removed in the same commit.

### Task 4: Make the frontend hook generic

**Files:**
- Create: `apps/desktop/src/features/code/useLiveLanguage.tsx`
- Create: `apps/desktop/src/features/code/definitionNavigation.ts`
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/shared/api/languageIntelligence.ts`
- Remove: `apps/desktop/src/features/code/useLiveRustLanguage.tsx`
- Modify: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Produces: `useLiveLanguage` with capability-driven enablement.
- Produces: focused CodeMirror definition-link and target-reveal extensions.
- Consumes: Task 3 runtime status and advertised capabilities.

- [ ] **Step 1: Add failing first-wave frontend tests**

Assert modifier-hover, modifier-click, F12, exact reveal, normal-click isolation, and capability-disabled behavior for TypeScript and TOML fixtures. Add JSON and YAML fixtures that prove the generic hook activates only the capabilities advertised by their server responses.

- [ ] **Step 2: Verify RED**

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "TypeScript intelligence|TOML intelligence"
```

Expected: FAIL because the hook still requires Rust.

- [ ] **Step 3: Generalize the hook and extract navigation state**

Enable actions only when the selected active profile advertises the capability. Preserve cancellation and exact-range reveal.

- [ ] **Step 4: Verify, commit, and push**

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts
pnpm --dir apps/desktop run build
git diff --check
git add apps/desktop/src/features/code/useLiveLanguage.tsx \
  apps/desktop/src/features/code/definitionNavigation.ts \
  apps/desktop/src/features/code/SemanticCodeEditor.tsx \
  apps/desktop/src/features/code/useLiveRustLanguage.tsx \
  apps/desktop/src/shared/api/languageIntelligence.ts apps/desktop/e2e/editor-ui.spec.ts
git commit -m "feat(editor): enable multi-language navigation"
git push origin main
```

Expected: editor tests and production build pass before push.

### Task 5: Add confined read-only library documents

**Files:**
- Create: `crates/repodesk-core/src/code_library.rs`
- Modify: `crates/repodesk-core/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/code_workspace.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/shared/api/codeWorkspace.ts`
- Modify: `apps/desktop/src/features/code/CodeTab.tsx`
- Create: `apps/desktop/src/features/code/LibraryTabBadge.tsx`
- Test: `crates/repodesk-core/tests/code_library_security.rs`
- Test: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Produces: opaque `LibraryDocumentHandle`, read-only `CodeLibraryDocument`, and `code_library_read(handle)`.
- Consumes: definition URIs from an active session and derived dependency roots.

- [ ] **Step 1: Write failing confinement tests**

Cover allowed declarations and Cargo sources plus traversal, sensitive paths, non-file URIs, unsupported extensions, oversized files, arbitrary home files, expired handles, and save rejection.

- [ ] **Step 2: Verify RED**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test code_library_security
```

Expected: FAIL because no library-handle boundary exists.

- [ ] **Step 3: Implement backend-issued handles and read-only tabs**

Bind handles to project, session, canonical path, and expiry. Library tabs never enter dirty/save/Git/review/context flows.

- [ ] **Step 4: Verify, commit, and push**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test code_library_security
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "Library"
pnpm --dir apps/desktop run build
git diff --check
git add crates/repodesk-core/src/code_library.rs crates/repodesk-core/src/lib.rs \
  crates/repodesk-core/tests/code_library_security.rs \
  apps/desktop/src-tauri/src/code_workspace.rs apps/desktop/src-tauri/src/lib.rs \
  apps/desktop/src/shared/api/codeWorkspace.ts apps/desktop/src/features/code/CodeTab.tsx \
  apps/desktop/src/features/code/LibraryTabBadge.tsx apps/desktop/e2e/editor-ui.spec.ts
git commit -m "feat(editor): open library definitions read-only"
git push origin main
```

Expected: confinement, library UI, and build gates pass before push.

### Task 6: Add managed language-tool installation

**Files:**
- Create: `crates/repodesk-core/src/language_tools.rs`
- Modify: `crates/repodesk-core/src/lib.rs`
- Create: `apps/desktop/src-tauri/src/commands/language_tools.rs`
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/shared/api/languageIntelligence.ts`
- Test: `crates/repodesk-core/tests/language_tools_security.rs`

**Interfaces:**
- Produces: `language_tool_install_preview(recipe_id) -> InstallPreview` with revision-bound confirmation token.
- Produces: confirm, progress, cancellation, staging, version probe, and atomic promotion.
- Consumes: Task 2 recipe ids; never frontend-provided argv.

- [ ] **Step 1: Write failing installer-security tests**

Use fake installers to assert exact argv, confinement, stale-token rejection, cancellation, redaction, rollback, promotion, and no repository writes.

- [ ] **Step 2: Verify RED**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_tools_security
```

Expected: FAIL because the preview/confirmation boundary does not exist.

- [ ] **Step 3: Implement preview, confirmation, and staged execution**

Bind tokens to project, recipe id, version, destination, argv digest, and expiry. Promote only after a successful probe.

- [ ] **Step 4: Verify, commit, and push**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_tools_security
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
git add crates/repodesk-core/src/language_tools.rs crates/repodesk-core/src/lib.rs \
  crates/repodesk-core/tests/language_tools_security.rs \
  apps/desktop/src-tauri/src/commands/language_tools.rs \
  apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs \
  apps/desktop/src/shared/api/languageIntelligence.ts
git commit -m "feat(language): install managed servers safely"
git push origin main
```

Expected: security tests and workspace Clippy pass before push.

### Task 7: Build the RepoDesk Native language-tool UI

**Files:**
- Create: `apps/desktop/src/features/code/LanguageToolPill.tsx`
- Create: `apps/desktop/src/features/code/LanguageToolPopover.tsx`
- Create: `apps/desktop/src/features/code/LanguageInstallDialog.tsx`
- Create: `apps/desktop/src/features/code/language-tools.css`
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/live-language.css`
- Modify: `apps/desktop/e2e/editor-ui.spec.ts`
- Modify: `apps/desktop/e2e/mock-ipc.ts`

**Interfaces:**
- Produces: Ready, Starting, Missing, Installing, Error, and discovery-only states.
- Produces: accessible popover, exact install confirmation, progress/cancel/retry, and capability list.
- Consumes: runtime, capability, preview, and installation contracts.

- [ ] **Step 1: Write failing UI-state and installation tests**

Cover every pill state, narrow layout, keyboard/focus, exact preview, token forwarding, progress, cancellation, details, refresh, and no gutter/scrollbar overlap.

- [ ] **Step 2: Verify RED**

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "language tool"
```

Expected: FAIL because the UI does not exist.

- [ ] **Step 3: Implement approved visual direction B**

Clamp pill and popover to the editor content, use theme tokens, preserve non-focus-stealing hover, and keep the single right scrollbar.

- [ ] **Step 4: Verify, commit, and push**

```bash
pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts
pnpm --dir apps/desktop run build
git diff --check
git add apps/desktop/src/features/code/LanguageToolPill.tsx \
  apps/desktop/src/features/code/LanguageToolPopover.tsx \
  apps/desktop/src/features/code/LanguageInstallDialog.tsx \
  apps/desktop/src/features/code/language-tools.css \
  apps/desktop/src/features/code/SemanticCodeEditor.tsx \
  apps/desktop/src/features/code/live-language.css \
  apps/desktop/e2e/editor-ui.spec.ts apps/desktop/e2e/mock-ipc.ts
git commit -m "feat(editor): add native language-tool controls"
git push origin main
```

Expected: editor tests and frontend build pass before push.

### Task 8: Run the complete release gate

**Files:**
- Verify only; any scoped failure returns to the task that introduced it before the release gate is rerun.

**Interfaces:**
- Consumes: every previous task's pushed commit.
- Produces: fresh evidence that the complete pushed sequence is releasable.

- [ ] **Step 1: Run all repository gates**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
pnpm --dir apps/desktop run build
pnpm --dir apps/desktop exec playwright test
git diff --check
```

Expected: every command exits zero and `git status --short` is empty. Existing unrelated rustfmt drift is reported instead of reformatted wholesale; a feature-caused failure is fixed and committed within its owning task, then this complete gate is rerun from the beginning.
