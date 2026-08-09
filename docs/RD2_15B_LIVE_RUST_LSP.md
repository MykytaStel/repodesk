# RD2-15b — Live Rust language intelligence

## Purpose

RD2-15 introduced language-server discovery and protocol-facing types without starting background processes. RD2-15b turns that foundation into the first live language service while preserving RepoDesk's existing Code, Problems, verification, and security boundaries.

The first live implementation is intentionally Rust-only and backend-owned:

```text
Code (.rs)
  -> RepoDesk Tauri transport
  -> one rust-analyzer process
  -> LSP JSON-RPC over stdio
  -> diagnostics / hover / navigation / symbols
  -> existing Code + Problems surfaces
```

No new permanent navigation surface is introduced.

## Product contract

Live language intelligence is advisory developer feedback. It is not workflow authority.

```text
rust-analyzer diagnostic != VerificationReceipt
hover                    != evidence receipt
go-to-definition         != reviewed change
```

The Commit Gate continues to trust only the canonical review and verification pipeline.

## Session ownership

RepoDesk owns the language-server process in Rust/Tauri. The frontend never sends an executable path or arbitrary shell command.

The live Rust session is selected from the static backend registry created in RD2-15:

```text
server id:   rust-analyzer
language:    rust
cwd:         active project root
transport:   stdin/stdout JSON-RPC
```

V0 allows at most one live language-server session. Opening a Rust document for another project replaces the stale project session.

## Lifecycle

```text
open supported .rs file
  -> discovery confirms rust-analyzer is available
  -> lazy process start
  -> initialize
  -> initialized
  -> didOpen(version = 1)

edit document
  -> 350 ms debounce
  -> full-text didChange(version++)

switch document
  -> didClose(old)
  -> didOpen(new)

leave editor surface
  -> StrictMode-safe grace period
  -> shutdown
  -> exit
  -> process cleanup
```

React development StrictMode intentionally mounts and cleans up effects twice. RepoDesk therefore uses a small reference-counted owner with a 250 ms stop grace period instead of killing the server on the first cleanup callback.

## Performance budget

RD2-15b deliberately avoids a high-frequency bridge.

```text
idle polling                  0
language-server processes     max 1
change debounce               350 ms
editable document size        <= 512 KiB
request timeout               8 s
initialize timeout            20 s
cached LSP diagnostics        <= 500 total
LSP diagnostic files          <= 64
locations returned/request    <= 128
symbols returned/request      <= 500
hover text                    <= 12,000 chars
```

Document changes use full-text synchronization in v0. Incremental edits can be introduced later only if measurements justify the additional editor/protocol complexity.

## JSON-RPC transport

The backend implements standard LSP framing:

```text
Content-Length: <bytes>\r\n
\r\n
<JSON payload>
```

Requests use monotonically increasing numeric IDs and a bounded pending-request map. On timeout RepoDesk removes the pending request and sends:

```text
$/cancelRequest
```

The stdout reader is the protocol authority. stderr is drained so the child cannot block, but routine rust-analyzer logging is not treated as a session failure.

## Server-to-client requests

Rust-analyzer can issue requests to the client while an ordinary RepoDesk request is pending. The reader thread therefore handles server requests independently instead of treating stdout as response-only.

Supported v0 responses include:

```text
workspace/configuration
workspace/workspaceFolders
client/registerCapability
client/unregisterCapability
window/workDoneProgress/create
window/showMessageRequest
workspace/semanticTokens/refresh
workspace/inlayHint/refresh
workspace/codeLens/refresh
workspace/diagnostic/refresh
```

Unknown server requests receive JSON-RPC `Method not found` rather than hanging indefinitely.

## Workspace mutation rule

A language server does not gain write authority.

Every `workspace/applyEdit` request is rejected:

```json
{
  "applied": false,
  "failureReason": "RepoDesk does not allow language servers to mutate the workspace"
}
```

Future rename/code-action support must route proposed edits through an explicit RepoDesk review/change boundary rather than silently enabling `workspace/applyEdit`.

## Path security

Every editor-originated document path is revalidated in Rust through the existing Code Workspace boundary before it becomes an LSP URI.

Server-returned locations are accepted only when their canonical file path stays under the active project root. External/sysroot/virtual locations are ignored in v0 rather than bypassing repository guards.

This means go-to-definition is intentionally project-scoped in the first slice.

## Coordinate contract

The protocol remains zero-based:

```text
LSP line 0 / character 0
```

RepoDesk Code navigation remains one-based:

```text
Ln 1 / Col 1
```

Protocol diagnostics are converted to one-based coordinates only when adapted into the Problems UI. Navigation results returned by the backend are already one-based.

The existing textarea cursor offsets are JavaScript UTF-16 code-unit offsets, matching the default LSP UTF-16 position encoding used by rust-analyzer.

## Diagnostics

Rust-analyzer `textDocument/publishDiagnostics` notifications are emitted from Tauri as:

```text
language-diagnostics
```

A publish event replaces diagnostics for one document URI, not the whole project. The frontend therefore keeps bounded per-file buckets and publishes their aggregate into the existing RD2-13 Problems store.

```text
rust-analyzer
  -> publishDiagnostics(file A)
  -> publishDiagnostics(file B)
  -> bounded LSP file buckets
  -> Problems source: lsp
```

An empty publish event clears only that file's bucket.

No second diagnostics panel is introduced.

## Status

For a supported Rust file the existing Code status bar moves from discovery-only wording:

```text
LS rust-analyzer found
```

to live lifecycle state:

```text
RA starting
RA ready
RA error
```

The tooltip can expose PID/open-document/error context without adding another card or inspector section.

## IDE actions

RD2-15b adds keyboard-first language actions without adding permanent toolbar controls.

```text
F12                  Go to definition
Shift+F12            Find references
Cmd/Ctrl+Shift+O     Document symbols
Alt+H                Hover information
Escape               Close language overlay
```

Single definitions navigate immediately. Multiple definitions, references, symbols, and hover content use one compact editor overlay.

Hover content is rendered as text/preformatted content. RepoDesk does not inject language-server markdown as raw HTML.

## Multiplexed Tauri transport

To avoid expanding the global Tauri handler registry with seven near-identical commands, RD2-15b reuses the already registered `language_intelligence_snapshot` entrypoint as a compatibility bridge.

Without an action it returns the same discovery snapshot introduced in RD2-15.

With a tagged action it performs one live operation:

```text
status
sync_document
close_document
hover
definition
references
document_symbols
stop
```

Blocking process/JSON-RPC work runs through Tauri's blocking runtime rather than the UI thread.

This transport can later be split into dedicated commands if the language subsystem grows enough to justify it; the frontend and core protocol types do not depend on that implementation detail.

## Non-goals

This slice does not add:

- TypeScript live sessions;
- completion UI;
- rename;
- formatting through LSP;
- code actions;
- semantic highlighting;
- workspace edits;
- external/sysroot source browsing;
- language-server auto-install;
- language-server network downloads;
- verification evidence from LSP.

## Next slice

The next language-intelligence slice should be selected from measured value rather than feature count. Likely candidates:

```text
A. completion + signature help for Rust
B. reviewed rename/code-action proposal flow
C. TypeScript live session using the same manager boundary
D. repository symbol index feeding Repo Intelligence
```

Before expanding protocols, measure rust-analyzer startup time, resident memory, full-text sync cost, diagnostic latency, and session restart behavior on representative repositories.
