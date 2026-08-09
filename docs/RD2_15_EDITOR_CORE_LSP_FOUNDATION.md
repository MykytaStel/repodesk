# RD2-15 — Editor Core and LSP Foundation

## Goal

RD2-15 establishes two foundations that the Code workspace needs before live language-server sessions:

1. deterministic editor/gutter geometry with one authoritative scroll surface;
2. a typed, backend-owned language-intelligence boundary that can later host LSP lifecycle safely.

This slice intentionally does **not** start language servers. Discovery and protocol contracts land first so later lifecycle code has stable ownership, coordinates, capabilities, and Problems integration.

---

## 1. Editor scroll invariant

### The bug

The lightweight editor historically used two independently scrollable elements:

```text
textarea                pre gutter
   |                        |
scrollHeight A          scrollHeight B
   |                        |
scrollTop ---------> gutter.scrollTop
```

Even with identical font metrics, padding, and attempted scrollbar compensation, the browser/WebView is free to calculate different effective scroll ranges for the textarea and the `<pre>`.

Near end-of-file the gutter could clamp before the source document. The visible symptom was that text kept scrolling while line numbers stopped several rows early.

Repeated offset/padding fixes could not guarantee correctness because the architecture still contained two scroll models.

### New invariant

The textarea is the **only scroll surface**:

```text
textarea.scrollTop
       |
       v
--editor-scroll-top
       |
       v
translateY(line-number visual layer)
```

The gutter is now:

- non-scrollable;
- clipped by its shell;
- naturally tall enough for all line numbers;
- translated by the textarea's exact `scrollTop`;
- independent of its own `scrollHeight` because it has no scroll state.

Critical metrics are shared explicitly:

```text
editor top padding     12px
editor line height     20px
editor font size       12.5px
editor bottom padding  0px
```

The active-line marker uses the same top padding and line-height constants.

### Performance contract

Scrolling must not update React state.

The scroll handler writes one CSS custom property directly:

```text
--editor-scroll-top
```

The line-number layer moves with a compositor-friendly `translate3d` transform.

This avoids a React render on every scroll event while still keeping exact visual alignment.

### Forbidden regression

Do not reintroduce:

- a scrollable gutter;
- `gutter.scrollTop = editor.scrollTop`;
- proportional gutter scroll mapping;
- artificial source `padding-bottom` to compensate a scrollbar;
- a second independently measured editor line height.

If the lightweight editor is later replaced by CodeMirror, Monaco, or another engine, that engine must own text and gutter geometry internally rather than rebuilding this synchronization externally.

---

## 2. Language intelligence boundary

RepoDesk needs language intelligence, but `LSP` is not a single UI feature. It is a long-lived protocol/runtime subsystem.

The intended architecture is:

```text
Code document
    |
    v
Language Intelligence API
    |
    +--> server discovery
    +--> capability selection
    +--> future session manager
              |
              v
          LSP JSON-RPC
              |
              +--> diagnostics ------> Problems
              +--> definition --------> Code navigation
              +--> hover -------------> Code overlay
              +--> references ---------> Code/search surface
```

The UI never owns server process spawning.

---

## 3. Discovery vs session lifecycle

### This slice: discovery

Discovery is side-effect free.

RepoDesk checks whether known server executables exist:

1. project-local `node_modules/.bin` for Node-based tools where applicable;
2. current process `PATH`.

No process is started.
No network request is made.
No package manager is invoked.
No server is installed automatically.
No polling loop runs.

The Code editor caches discovery through React Query with a 60-second stale window and no focus refetch.

### Next slice: live sessions

The future backend-owned lifecycle should be:

```text
discover
  -> start one project-scoped server
  -> initialize
  -> initialized
  -> didOpen(document, version=1)
  -> didChange(version++)
  -> publishDiagnostics
  -> Problems
  -> didClose
  -> shutdown
  -> exit
```

A project switch or application shutdown must terminate owned child processes cleanly.

---

## 4. Supported server registry v0

The registry is static and backend-owned.

| Language | Server | Launch arguments |
|---|---|---|
| Rust | `rust-analyzer` | none |
| TypeScript / JavaScript | `typescript-language-server` | `--stdio` |
| Python | `pyright-langserver` | `--stdio` |
| Go | `gopls` | none |
| C / C++ | `clangd` | none |
| Java | `jdtls` | none |
| Kotlin | `kotlin-language-server` | none |
| Swift | `sourcekit-lsp` | none |
| Shell | `bash-language-server` | `start` |
| JSON | `vscode-json-language-server` | `--stdio` |
| YAML | `yaml-language-server` | `--stdio` |
| TOML | `taplo` | `lsp stdio` |

Availability states:

```text
available
missing
```

Sources:

```text
project_local
path
```

Discovery does not expose an arbitrary executable supplied by the frontend.

---

## 5. Capability model

The typed descriptor carries a stable capability shape:

```text
diagnostics
definition
hover
references
completion
rename
formatting
document_symbols
```

The first live milestone should only depend on:

```text
diagnostics
definition
hover
references
```

Completion, rename, formatting, and richer symbol UI can follow after session correctness is proven.

The current capability values describe the intended server integration contract, not an active negotiated LSP `ServerCapabilities` response. A live session must replace assumptions with the capabilities actually returned by `initialize` before enabling a feature.

---

## 6. Protocol coordinate contract

LSP positions are zero-based:

```text
LSP line 0, character 0
```

RepoDesk's human editor/status/navigation is one-based:

```text
Ln 1, Col 1
```

Core protocol types therefore remain explicitly zero-based:

```rust
LspPosition {
    line,
    character,
}

LspRange {
    start,
    end,
}
```

Conversion happens only at the frontend Problems/navigation boundary:

```text
Problem.line   = LSP.start.line + 1
Problem.column = LSP.start.character + 1
```

Do not store one-based coordinates in future JSON-RPC session state.

---

## 7. Diagnostics reuse the Problems model

LSP does not get another diagnostics panel.

The existing source union becomes:

```text
repopilot
check
verification
lsp
```

Future `textDocument/publishDiagnostics` flow:

```text
LanguageDiagnostic[]
       |
       v
captureLanguageDiagnostics()
       |
       v
Problems source bucket: lsp
       |
       v
Problems row
       |
       v
requestCodeWorkspaceOpen(path, line, column)
```

This preserves the RD2-13 principle: one user-facing Problems surface, many evidence sources.

Language diagnostics must still pass the existing relative-path normalization before becoming navigable.

---

## 8. Code status UI

The active editor may show a compact discovery state:

```text
LS rust-analyzer found
LS TypeScript Language Server missing
```

This is deliberately status-bar metadata, not a new card, drawer, or primary navigation item.

The label is width-bounded and ellipsized to protect editor space.

A `found` label means only that the executable was discovered. It does **not** mean:

- a server process is running;
- initialization succeeded;
- diagnostics are current;
- a workspace is indexed.

The tooltip states that live sessions are not started in this slice.

---

## 9. Security boundary for live LSP

Future LSP execution must preserve these rules:

1. frontend sends a static `server_id`, never a shell command;
2. backend resolves that ID through the built-in registry;
3. no `sh -c` / `cmd /C` wrapper for server launch;
4. working directory is the active project root;
5. server lifetime is project-scoped;
6. stdout is protocol transport, not an unbounded log buffer;
7. stderr logging is bounded;
8. child processes are killed/reaped on failure and shutdown;
9. server responses cannot bypass Code Workspace path guards for file edits;
10. LSP diagnostics are advisory and never satisfy a VerificationReceipt or Commit Gate.

Language-server code intelligence must not become an alternate unrestricted file-write channel.

---

## 10. Live session performance constraints

The next slice should target:

```text
one server process per project/language family
not one process per file
```

Document changes should be debounced before `didChange`, approximately 100–200 ms for the first implementation.

Required bounds:

- finite pending-request map;
- request timeout/cancellation;
- bounded stderr;
- bounded diagnostics per document/project;
- no copy of entire repository into frontend memory;
- no repeated executable discovery per keystroke;
- no global polling loop.

For Rust, `rust-analyzer` should start lazily only when a Rust document needs live language intelligence.

---

## 11. JSON-RPC transport requirements for the next slice

LSP over stdio uses framed messages with headers such as:

```text
Content-Length: <bytes>\r\n
\r\n
<JSON payload>
```

The parser must operate on bytes, not newline-delimited JSON assumptions.

The session manager will need:

```text
request id allocation
pending request map
notifications
server requests
response matching
Content-Length framing
partial-read buffering
shutdown lifecycle
```

Do not parse stdout as ordinary process logs.

---

## 12. Non-goals of this slice

RD2-15 foundation does not include:

- starting `rust-analyzer`;
- starting TypeScript/Python/etc. servers;
- LSP auto-install;
- package-manager mutation;
- completion UI;
- rename UI;
- hover popovers;
- references UI;
- go-to-definition execution;
- semantic tokens;
- workspace edits;
- formatting through LSP;
- a new LSP navigation destination;
- using a manual language-server run as verification evidence.

No new frontend editor dependency is added in this slice.

---

## 13. Acceptance for this foundation

Editor:

- the gutter has no independent scroll state;
- a file can reach absolute bottom without line numbers clamping early;
- horizontal overflow does not require gutter scroll-range compensation;
- navigating away and back cannot restore legacy `28px` source padding;
- scrolling does not cause React rerenders solely to move line numbers.

Language intelligence:

- discovery is backend-owned and side-effect free;
- project-local Node server binaries are preferred over PATH;
- missing supported servers are represented explicitly;
- protocol ranges are zero-based;
- frontend diagnostics adapter converts to one-based navigation once;
- Problems recognizes Language Server as a source;
- no server process starts merely by opening Code.

---

## 14. Follow-up: RD2-15b live Rust LSP

Recommended next vertical slice:

```text
Rust file opened
   -> resolve rust-analyzer registry entry
   -> lazily start project-scoped session
   -> initialize capabilities
   -> didOpen
   -> debounced didChange
   -> publishDiagnostics
   -> Problems
```

Then add:

```text
hover
go to definition
find references
```

Only after the Rust lifecycle is stable should TypeScript be attached to the same generic session boundary.
