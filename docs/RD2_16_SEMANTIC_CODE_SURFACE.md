# RD2-16 — Semantic Code Surface

## Status

Implementation slice for the RepoDesk 2 workspace.

## Product boundary

RepoDesk is not attempting to become another general-purpose VS Code distribution.

The Code surface exists inside a governed engineering lifecycle:

```text
Intent
  -> Work Item Contract
  -> bounded context
  -> human / worker execution
  -> ChangeSet
  -> review
  -> verification
  -> commit
  -> Engineering Knowledge
```

A conventional editor answers:

> What code is this?

The RepoDesk semantic surface must additionally answer:

> What engineering state is this code in?

CodeMirror is therefore used as a frontend editing primitive. It does not become the authority for repository state, filesystem access, verification, or ChangeSet governance.

## Authority remains in RepoDesk

The migration does **not** move these responsibilities into CodeMirror:

- project/file discovery;
- path traversal and sensitive-file guards;
- UTF-8/editable-size policy;
- optimistic save fingerprints;
- Git review semantics;
- Work Item scope;
- ChangeSet origin and review state;
- VerificationReceipt and commit gate;
- rust-analyzer process ownership;
- Problems aggregation.

The editor receives bounded projections of those systems and renders them.

## Semantic layers

The surface intentionally separates four visual layers.

### 1. Language syntax

Syntax color answers only language-semantic questions:

```text
keyword
function
type
variable/property
string
number/bool/null
comment
operator/punctuation
metadata/attributes
```

Syntax colors use RepoDesk-owned tokens rather than a third-party theme. The current theme families have dedicated palettes, including light, Midnight, Nord, and high-contrast modes.

Supported parser-backed highlighting in v0:

- Rust;
- JavaScript / JSX;
- TypeScript / TSX;
- JSON.

Other safe text files remain editable but may be plain until their language package is justified by usage/bundle cost.

### 2. Diagnostics

The existing Problems store remains canonical for advisory code diagnostics.

RepoDesk projects file-backed Problems into CodeMirror diagnostics:

```text
rust-analyzer / checks / RepoPilot
              |
              +-> Problems panel
              +-> editor squiggle / lint gutter
```

No second LSP-only diagnostic store is introduced.

A diagnostic is advisory engineering information. It is never VerificationReceipt evidence.

### 3. Git line state

For the active changed file RepoDesk projects the unified Git diff into a narrow gutter:

```text
added     -> green
modified  -> accent
removed   -> red marker anchored near the deletion
```

This is intentionally a visual hint; Changes remains the canonical review surface.

Git line markers are hidden while the active editor buffer is dirty. The on-disk Git diff cannot truthfully describe line locations in an unsaved draft.

### 4. RepoDesk engineering state

A compact semantic strip can surface:

- current Work Item;
- scope state;
- review state;
- verification state;
- ChangeSet origin;
- file problem counts.

Exceptional states are visually stronger than healthy states.

For example:

```text
Work auth-42 · Out of scope · Draft after verification · 2 errors
```

This layer is what differentiates RepoDesk from a generic code editor.

## Truthfulness boundary

RD2-16 does **not** claim per-line AI provenance.

The current event/governance model can prove ChangeSet-level worker origin, but that is not equivalent to proving that an individual line was authored by a specific worker.

Therefore:

```text
Git changed line       -> line-level marker is allowed
LSP diagnostic range   -> range-level marker is allowed
ChangeSet worker       -> file/ChangeSet-level label only
Work Item scope        -> file-level governance state
Verification           -> reviewed-tree/file-level state
```

Future line-level provenance requires first-class evidence at edit/patch application time.

## Editor engine

RD2-16 replaces the runtime Code edit view with modular CodeMirror 6 components rather than Monaco.

RepoDesk deliberately avoids the all-in-one/basic editor preset and opts into only the primitives it currently needs:

- state/view;
- history and keymaps;
- search;
- language parsing/highlighting;
- lint diagnostics;
- gutters/decorations.

This keeps editor capabilities subordinate to RepoDesk product architecture.

The previous `LightweightCodeEditor` source is retained temporarily as a migration fallback/reference until local parity smoke is complete. It is no longer the normal CodeTab edit renderer.

## Editing parity

The new surface retains the existing RepoDesk contracts:

- bounded open-file session;
- dirty drafts in process memory;
- Cmd/Ctrl+S save;
- optimistic concurrency / external-change conflict;
- exact `file:line:column` navigation;
- F12 definition;
- Shift+F12 references;
- Cmd/Ctrl+Shift+O document symbols;
- Alt+H hover;
- Code -> Diff switch;
- active Problems integration.

CodeMirror owns editor-local undo/history and selection. RepoDesk owns the actual file write.

## Performance contract

RD2-16 must remain quiet while idle.

- no new polling;
- CodeMirror mounts only for the active file in Edit mode;
- repository file content is still loaded on demand;
- the existing 512 KiB editable-file limit remains authoritative;
- engineering snapshot uses the shared query/cache contract;
- Git diff is requested only for the active changed file;
- Git gutter reconfiguration uses a CodeMirror `Compartment` and does not rebuild the editor history;
- line decorations are bounded by the active file diff;
- Problems are already bounded by the Problems model.

## Security

CodeMirror receives only the document text already admitted by Code Workspace.

It does not receive direct filesystem APIs or arbitrary process execution authority.

Language-server workspace edits remain rejected by the RD2-15b backend boundary.

Save continues through:

```text
CodeMirror buffer
  -> RepoDesk save request
  -> path/sensitive-file validation
  -> expected fingerprint check
  -> bounded atomic-ish write
```

## Why this is not a VS Code clone

RD2-16 does not add:

- an extension marketplace;
- arbitrary editor plugins;
- a generic command ecosystem;
- a second terminal/process authority;
- editor-owned Git operations;
- editor-owned LSP processes.

The editor is a rendering and interaction component over RepoDesk engineering state.

The long-term differentiator is the relationship between the same source code and:

```text
Work Item
Context Manifest
worker provenance
ChangeSet
Problems
verification evidence
Engineering Knowledge
Repository Intelligence
```

## Next: RD2-17 Repository Intelligence

The next product slice should make the semantic surface useful beyond IDE parity by deriving a bounded repository graph from signals RepoDesk already owns:

```text
files
 + symbols/references
 + imports/dependencies
 + tests
 + Git co-change/history
 + Work Items
 + Engineering Knowledge
       -> Repository Intelligence Graph
```

That graph should serve both human understanding and bounded AI context selection.

## Local verification

```bash
git fetch origin
git checkout feat/rd2-semantic-code-surface
git pull

pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop build

cargo fmt --all -- --check
cargo clippy \
  -p repodesk-core \
  -p repodesk-desktop \
  --all-targets \
  --all-features \
  -- -D warnings

cargo test -p repodesk-core code_workspace
cargo test -p repodesk-desktop language_server
```

CI is not part of this slice's implementation workflow and should not be polled for validation.

## Manual smoke

1. Open a Rust file and confirm syntax roles are visually distinct.
2. Open `.ts`, `.tsx`, `.js`, `.jsx`, and `.json` samples and confirm parser-backed highlighting.
3. Open Markdown/TOML and confirm safe editing still works even where parser highlighting is not yet enabled.
4. Verify Cmd/Ctrl+F, undo/redo, selection, Tab indentation, and Cmd/Ctrl+S.
5. Verify stale fingerprint save protection still rejects an externally changed file.
6. Introduce a Rust error and confirm the same diagnostic appears in Problems and inline in Code.
7. Verify F12, Shift+F12, Cmd/Ctrl+Shift+O, and Alt+H remain functional.
8. Modify/save a tracked file and confirm Git gutter markers appear after cache invalidation/refetch.
9. Create an unsaved draft and confirm stale on-disk Git line markers disappear until save.
10. With an active Work Item, inspect scope/review/verification state in the semantic strip.
11. Confirm an out-of-scope/protected file receives an exceptional engineering-state treatment without changing commit-gate authority.
12. Verify a dirty draft after previously passed verification says `Draft after verification` rather than implying that the draft itself is verified.
13. Switch projects and confirm drafts/semantic state stay project-scoped.
14. Smoke all RepoDesk themes, especially Hermes Light and High Contrast.
15. Confirm Changes remains the canonical diff/review view and Work Verify remains the only source of canonical verification evidence.
