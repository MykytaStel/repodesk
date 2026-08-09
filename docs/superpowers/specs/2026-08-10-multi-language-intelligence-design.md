# Multi-language intelligence and language tools

## Goal

Replace the Rust-only live language session with a safe multi-language platform and present it through the approved RepoDesk Native UI. The first complete wave supports Rust, TypeScript/JavaScript, TOML, JSON, and YAML, including navigation into library declarations without making dependency files editable.

## Supported rollout

### Fully active profiles

- Rust: `rust-analyzer`.
- TypeScript and JavaScript: `typescript-language-server --stdio` with TypeScript available from the project or the managed installation.
- TOML: `taplo lsp stdio`.
- JSON: `vscode-json-language-server --stdio`.
- YAML: `yaml-language-server --stdio`.

Each active profile supports only the capabilities advertised by the initialized server. RepoDesk never shows hover, definition, reference, symbol, diagnostic, or completion affordances merely because the registry expects them.

### Discoverable profiles

Python, Go, C/C++, Java, Kotlin, Swift, and Shell remain visible in language-tool status. Their descriptors and installation availability are reported, but RepoDesk does not start them until a tested initialization profile is added. This avoids treating servers such as JDTLS, SourceKit-LSP, and Kotlin Language Server as interchangeable stdio processes.

## Architecture

### Registry

The language-server registry becomes the single source of truth for:

- stable server id and display label;
- supported RepoDesk language ids;
- executable name and argv;
- initialization profile id;
- declared capability ceiling;
- installation recipe id, if RepoDesk can install it;
- whether the profile is active or discovery-only.

Frontend and backend consume the same serialized descriptor. Rust-specific identity checks are removed from the session layer.

### Session manager

The manager owns sessions by `(project identity, server id)`. Multiple servers may run concurrently for one project, while documents for the same server share one process.

For every session the manager owns:

- canonical project root and workspace folder;
- exact executable and argv selected by the registry;
- process, stdin, stdout, and bounded stderr tail;
- initialization state and server-advertised capabilities;
- pending JSON-RPC requests and timeouts;
- open document versions and language ids;
- diagnostic routing and shutdown state.

Opening a document chooses the preferred available active profile for its language. Sync, hover, definition, references, symbols, diagnostics, and close operations route through that session. Switching projects shuts down every session belonging to the previous project. A crash affects only its own profile and exposes a retry action.

### Initialization profiles

The generic JSON-RPC transport remains shared. Small initialization profiles provide only server-specific protocol data:

- initialization options;
- workspace configuration responses;
- URI and language-id normalization;
- optional server-specific notifications;
- diagnostic quirks that cannot be expressed by the generic protocol.

The default profile is sufficient for Rust, TypeScript/JavaScript, JSON, and YAML. Taplo receives its explicit TOML profile. No profile can enable `workspace/applyEdit`; RepoDesk continues returning a rejection for all server-initiated workspace edits.

## TypeScript and library behavior

The TypeScript profile starts in the project root and respects the nearest `tsconfig.json` or `jsconfig.json`. It resolves project `node_modules`, package `exports`, source maps, `.d.ts` declarations, path aliases, and workspace packages through the language server rather than duplicating TypeScript resolution in RepoDesk.

When a definition URI points inside the repository, RepoDesk opens the normal workspace document. When it points to an approved dependency or toolchain root, RepoDesk opens a read-only `Library` tab. Library tabs:

- display the normalized package-relative or toolchain-relative path;
- cannot become dirty and expose no Save action;
- do not enter Git status, review, context-building, or AI payloads;
- can participate in hover, definition, references, and back/forward navigation;
- are discarded from the tab cache when the project changes.

Library reads accept only `file:` URIs returned by the active session. The backend additionally requires a safe text extension, enforces the editor byte limit, applies the existing sensitive-path denylist, and confines reads to derived dependency roots:

- repository workspace roots and workspace packages;
- project-local `node_modules` trees;
- Rust sysroot and Cargo registry source roots;
- RepoDesk's managed language-tool root.

Arbitrary absolute paths and non-file URIs are rejected. Rejection produces a bounded explanation rather than silently opening a file.

## Managed installation

### Storage and mutation boundary

RepoDesk-managed language tools live under `REPODESK_HOME/tools/language-servers`. Installation never changes the active repository, its manifests, lockfiles, project-local `node_modules`, or global package-manager state.

Installation is available only for recipes compiled into RepoDesk. The frontend sends a recipe id, never a command, executable, URL, package name, or arbitrary arguments. The backend resolves the exact allowlisted argv.

### Recipes

- TypeScript/JavaScript uses a pinned `typescript-language-server` plus compatible `typescript` package installed into the managed Node tool root.
- JSON uses the pinned `vscode-langservers-extracted` package in the managed Node tool root.
- YAML uses the pinned `yaml-language-server` package in the managed Node tool root.
- TOML uses a pinned `taplo-cli` installed into the managed Taplo root with Cargo's locked dependency resolution.
- Rust remains a rustup component and presents the existing `rustup component add rust-analyzer` recipe when missing.

Every recipe declares its required installer (`npm`, `cargo`, or `rustup`), pinned package version, destination, executable probe argv, and supported operating systems. RepoDesk does not install a missing package manager. It explains the prerequisite and offers a copyable command instead.

### Confirmation and execution

The confirmation dialog shows:

- server and language names;
- exact executable plus argv;
- pinned package/version and source ecosystem;
- destination directory;
- whether network access is required;
- files outside the repository that may be created;
- an explicit `Install language server` action.

Execution uses argv-only process spawning without a shell. Output is streamed as structured progress, bounded in size, and redacted before display. Cancellation terminates the installer process. Success requires both a zero exit status and a successful version probe. RepoDesk then refreshes discovery and starts the server without an application restart.

Failed or partial installations stay isolated in a staging directory. Only a verified installation is atomically promoted to the active recipe directory. Retrying replaces the staging directory, not the last working installation.

## RepoDesk Native UI

### Contextual pill

The editor shows a compact pill in its upper-right content area using the selected visual direction B. It contains the language label and one state:

- `Ready`;
- `Starting`;
- `Missing`;
- `Installing` with progress;
- `Error`.

The pill does not cover the scrollbar, gutter, current line, or modifier-hover target. It collapses to an icon and accessible label when editor width is constrained.

Activating the pill opens a compact popover showing server label, executable source, version, active capabilities, and the relevant action: `Install`, `Retry`, `Restart`, or `Show details`. Discovery-only profiles say that live support is not enabled yet instead of presenting a non-functional start action.

### Hover and navigation

The modifier-hover card uses a stronger information hierarchy:

1. symbol kind and signature;
2. package, module, or declaration origin;
3. documentation;
4. `Command/Control click to open definition` guidance.

Only a real definition response adds the source-link underline. A successful navigation preserves the existing transient exact-range reveal. Multiple definitions continue to use a compact result list. Library destinations carry a visible `Library` badge and read-only state.

### Error behavior

Normal editing never depends on a language server. A missing, starting, crashed, or unsupported server removes unavailable intelligence affordances but keeps syntax highlighting, scrolling, selection, editing, and save behavior intact.

Short errors appear in the pill or popover. Detailed stderr and lifecycle evidence stay behind `Show details`. Error output is bounded and redacted. One failed profile does not affect other active sessions.

## Data contracts

The language intelligence snapshot reports both discovery and runtime profile information. Runtime status events include project, server id, languages, state, pid when present, open-document count, advertised capabilities, executable source, version, progress when installing, and a bounded error summary.

Every document action includes its path or approved library handle, language id, text version, and position where relevant. Session selection occurs in the backend; the frontend does not choose an executable.

Installation commands accept a recipe id and explicit confirmation token bound to the previewed recipe revision. A stale confirmation cannot authorize a changed recipe.

## Testing

### Core and backend

- registry tests cover active versus discovery-only profiles and preferred-server selection;
- session-manager tests cover concurrent Rust and TypeScript sessions, document routing, project switching, crash isolation, shutdown, request timeout, and stale response rejection;
- protocol tests cover advertised capabilities and server-specific initialization profiles;
- installer tests cover every allowlisted argv, confirmation-token binding, missing installers, cancellation, staging cleanup, atomic promotion, version-probe failure, and output bounds/redaction;
- library tests cover workspace files, `node_modules`, Cargo registry/sysroot, path traversal, sensitive paths, non-file URIs, unsupported extensions, oversized files, and save rejection.

### Frontend and end to end

- pill and popover tests cover Ready, Starting, Missing, Installing, Error, unsupported, compact-width, keyboard, and screen-reader states;
- TypeScript tests cover project imports, path aliases, workspace packages, package declarations, and a read-only library tab;
- TOML, JSON, and YAML tests cover honest hover/definition affordances and missing-schema behavior;
- navigation tests preserve normal click behavior, modifier-hover cancellation, multiple definitions, exact target reveal, and back/forward history;
- installation UI tests verify the exact preview, confirmation, progress, cancellation, successful refresh, and failure recovery;
- existing CodeMirror gutter and single-scrollbar tests remain unchanged and green.

## Verification gates

- focused Rust unit and integration tests for the registry, session manager, installer, and library boundary;
- `cargo test --workspace`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- desktop TypeScript/Vite production build;
- complete Playwright frontend suite;
- `git diff --check` and scoped-file review.

## Out of scope

- automatic installation without confirmation;
- arbitrary user-authored install commands or custom executable arguments;
- editing dependency or toolchain files;
- server-initiated workspace edits;
- enabling unverified Python, Go, C/C++, Java, Kotlin, Swift, or Shell session profiles in the first wave;
- duplicating TypeScript module resolution or schema resolution in RepoDesk;
- completion UI, rename, formatting, and code actions in this slice even when a server advertises them.
