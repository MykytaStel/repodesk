# RD2-13 — Problems Workbench

## Goal

RepoDesk needs one engineering-diagnostic surface that can receive findings from
multiple tools without turning the bottom panel into a copy of Output.

The contract for RD2-13 is:

> **Problems are actionable engineering diagnostics. Output is execution telemetry.**

A generic failed IPC call, network error, or runtime action log belongs in
Output. A compiler, check, verification, RepoPilot, or future LSP diagnostic may
belong in Problems when it carries engineering meaning.

## Diagnostic model

The desktop v0 model is intentionally small:

```ts
ProblemDiagnostic {
  id,
  source,
  severity,
  message,
  path,
  line,
  column,
  code,
  command,
}
```

Supported severities:

```text
error
warning
info
```

Supported sources in the contract:

```text
repopilot
check
verification
```

`verification` is reserved by the model for direct verification integration. In
this slice, check-like desktop actions and RepoPilot are the first adapters.

## Sources v0

### RepoPilot

A fresh RepoPilot review publishes its findings into the `repopilot` source
bucket.

Severity mapping:

```text
CRITICAL / HIGH -> error
MEDIUM          -> warning
LOW / INFO      -> info
```

Running a new review replaces the previous RepoPilot bucket. It does not append
unbounded historical findings.

### Check actions

Results returned through the shared `callCommand` transport for
`run_desktop_action` and `run_next_safe_step` are inspected only when the action
looks check-related.

Examples:

```text
checks
verify
verification
test
lint
clippy
cargo
build
tsc
eslint
```

The v0 parser recognises a deliberately bounded set of stable output shapes:

```text
Rust / Cargo / Clippy
error[E...]: message
  --> crates/foo/src/lib.rs:12:7

TypeScript
src/foo.ts(12,7): error TS1234: message

Colon style
src/foo.ts:12:7: error: message
```

Arbitrary stderr is not converted into a fake file diagnostic.

If a relevant check fails but no safe file location can be parsed, Problems may
show one non-navigable check failure. Generic API/runtime failures remain only
in Output.

## Bounded state

Problems is current engineering state, not an audit log.

Rules:

- maximum 500 current diagnostics;
- source buckets replace previous source state;
- duplicate diagnostics are collapsed by a stable identity;
- messages are bounded to 1,000 characters;
- no stdout/stderr corpus is retained in the Problems store;
- persisted action history is read only when the bottom panel is first opened;
- rehydration sorts actions by timestamp so the newest relevant check wins;
- switching projects clears Problems and panel history before lazy rehydrate.

Audit/evidence history remains owned by Runs, receipts, and the engineering event
ledger.

## Path safety

Problems never bypasses the Code Workspace security boundary.

The frontend parser accepts only repository-relative locations:

```text
src/lib.rs              allowed as a navigation candidate
./src/lib.rs            normalised to src/lib.rs
/path/to/file.rs         rejected as a navigation candidate
C:/repo/src/file.rs     rejected as a navigation candidate
../outside.rs           rejected as a navigation candidate
```

A click stores only a one-shot repository-relative path plus line/column.
`Code Workspace` then resolves and reads the file through its existing guarded
Rust API. Problems does not gain a direct filesystem read/write path.

## Code navigation

The existing Code hand-off now supports:

```text
path
line
column
```

Flow:

```text
Problem row
  -> one-shot path/location request
  -> open-code event
  -> Code surface
  -> guarded repository file lookup
  -> editor caret + scroll to location
```

Same-file navigation is supported. Clicking line 80 while the same file is
already open does not require closing or reopening the tab.

Changes can continue using the same hand-off with path only.

## Bottom panel

The dock now has clear responsibilities:

```text
Problems  -> typed engineering diagnostics
Output    -> actions + cheap IPC/runtime metadata
Terminal  -> live PTY
```

Problems v0 provides:

- total count in the tab;
- error / warning / info counts;
- All / Errors / Warnings filters;
- source and rule/code metadata;
- exact file:line:column where available;
- click-to-Code navigation;
- compact row layout rather than cards.

## Editor gutter correction

The screenshot-driven follow-up exposed a separate editor invariant.

The textarea can have a smaller `clientHeight` than the line-number gutter when
a horizontal scrollbar is present. Therefore:

```text
gutter.scrollTop = textarea.scrollTop
```

can clamp before the textarea reaches its final vertical position.

RepoDesk now preserves 1:1 line positioning and, only if the gutter clamps,
extends the gutter's bottom scroll range by the missing amount before retrying
the exact scroll position.

This avoids both failure modes:

- line numbers stopping before the end of the file;
- proportional scroll scaling that would introduce line drift in the middle.

The editor still uses one preformatted text node for all line numbers plus one
active-line overlay. It does not create one React element per source line.

## Screenshot-driven UI cleanup

### Project switcher

The project control is context selection, not project management.

For fewer than six connected projects:

- no search field;
- no visible stack/type badges;
- no keyboard-help footer;
- compact 30px rows;
- active-project checkmark only;
- footer contains `Open folder` and `Projects…`.

Search appears only at six or more projects. Keyboard navigation remains
available without permanently explaining it in the UI.

### Changes empty state

A clean repository now uses compact copy:

```text
No changes
This project has no uncommitted files.
```

The preview says only:

```text
Nothing to review
New edits will appear here after refresh.
```

Legacy flex/list distribution is explicitly disabled so these two lines stay
together at the top instead of spreading over the pane height.

### Project Knowledge

An empty filter no longer renders an empty 430px master/detail workspace.

Instead it renders one compact filter-specific state. The master/detail editor
exists only when records exist.

Copy is also shortened from product-policy prose to working language:

```text
Engineering knowledge
Reviewed rules, decisions and commands RepoDesk can reuse in future work.
```

## Performance contract

RD2-13 preserves previous runtime budgets:

- no Problems polling;
- diagnostics update through explicit source publication;
- Problems store is bounded;
- no raw command-output retention in Problems;
- no per-line React nodes in the editor gutter;
- project switcher adds no background polling;
- PTY remains mounted and survives panel hide/show.

## Future adapters

RD2-15 LSP should not create a second diagnostics UI.

It should adapt LSP diagnostics into the same model:

```text
LSP publishDiagnostics
       |
       v
ProblemDiagnostic[]
       |
       v
Problems Workbench
       |
       v
Code file:line:column
```

The same applies to richer compiler JSON, task runners, language-specific
linters, and verification receipts.

## Non-goals

RD2-13 does not add:

- LSP transport;
- compiler JSON/SARIF ingestion;
- persistent diagnostic history;
- automatic code fixes;
- Problems as a new primary Activity Rail destination;
- unrestricted filesystem access;
- a new editor engine.

Those can build on this contract without changing the bottom-panel information
architecture.
