# RD2-14 — Task Runner and Structured Checks

Status: implementation slice

## Product intent

RepoDesk needs a fast way to run the current project's ordinary engineering checks without turning the UI into an unrestricted shell or conflating utility checks with governed Work Item verification.

The Task Runner is therefore a secondary workbench tool:

```text
Problems | Tasks | Output | Terminal
```

It is deliberately not a sixth permanent Activity Rail destination.

The core boundary is:

```text
Project task
    -> explicit configured check
    -> allowlist validation
    -> bounded execution
    -> structured result
    -> Problems adapter
```

not:

```text
text field
    -> arbitrary shell
```

## Tasks vs Terminal vs Work Verification

These three surfaces have different authority.

### Tasks

Tasks are convenient, human-triggered project checks such as:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
pnpm typecheck
pnpm test
```

They answer:

> What do the project's configured engineering checks say right now?

A Task result is useful runtime evidence for the human, but is not commit-gate evidence.

### Terminal

Terminal is the user's interactive PTY. It remains a general manual tool and does not become an implicit source of verification truth.

### Work Verification

Work → Verify is the governed receipt-bound operation. It answers:

> Was the exact accepted ChangeSet verified against the exact reviewed tree/index state required by the Work Item contract?

Only that flow may satisfy the commit gate.

Therefore:

```text
Task passed       != VerificationReceipt
Terminal command  != VerificationReceipt
Work Verify       -> VerificationReceipt
```

This separation must remain explicit in future UI and API work.

## Source of tasks

RD2-14 does not invent a second task configuration system.

The task list is projected from:

```rust
ProjectConfig.checks: Vec<String>
```

This preserves one existing source of truth for project checks.

The current project type already seeds useful defaults. Examples include Rust format/lint/test checks and TypeScript/Python checks.

Future Settings as Code work may provide a better UI + JSON editor for managing these checks, but Task Runner v0 only consumes the existing project configuration.

## Core model

`crates/repodesk-core/src/task_runner.rs`

### ProjectTaskKind

```text
format
lint
typecheck
test
security
check
```

The kind is presentation metadata inferred deterministically from the configured command. It does not affect execution authority.

### ProjectTask

```text
id
label
command
kind
runnable
validation_error
```

### TaskRunStatus

```text
passed
failed
timeout
blocked
```

### TaskRunResult

```text
project
task_id
label
kind
command
status
exit_code
duration_ms
started_at
finished_at
stdout
stderr
stdout_truncated
stderr_truncated
```

### TaskRunBatch

`Run all` returns one batch with aggregate counts and every individual task result.

This matters for Problems: diagnostics from multiple failing tasks are published together rather than allowing the final command to erase earlier failures.

## Security contract

The frontend never sends an executable command to the task-run endpoint.

Single-task execution accepts only:

```text
task_id
```

The Rust backend then:

1. loads the active project;
2. rebuilds the task snapshot from the current `ProjectConfig.checks`;
3. resolves the requested task id in that current snapshot;
4. rejects an unknown or stale id;
5. executes the stored command through `checks::run_validated_check`.

`run_validated_check` remains the execution authority and reuses the existing check-command restrictions.

No frontend-provided command string is trusted.

## Stale task identity

A simple positional id such as `check-1` is insufficient because an external config edit could reorder checks between rendering and clicking Run.

Task ids therefore include both position and a deterministic command fingerprint:

```text
check-<index>-<fingerprint>
```

The fingerprint is a stable FNV-1a identity guard. It is not a cryptographic security primitive; allowlist validation remains the security boundary.

If project checks change after the UI snapshot, the old task id is rejected with a refresh instruction instead of silently running the new command at that index.

## Bounds

Task Runner v0 has explicit limits:

```text
max tasks                  64
per-command timeout        120 seconds
returned stdout tail       64 KiB
returned stderr tail       64 KiB
```

The output bound controls IPC/UI retention. The existing process runner still owns process execution and capture semantics.

Recent output is retained rather than the prefix because compiler/linter summaries and final diagnostics frequently appear near the end.

## Tauri transport

Commands:

```text
task_runner_snapshot
task_runner_run
task_runner_run_all
```

Long-running task execution is dispatched with `tauri::async_runtime::spawn_blocking` so the desktop async command path does not directly block on synchronous process execution.

## Bottom-panel UX

Task Runner is a compact dock, not a dashboard.

Typical layout:

```text
Project tasks   repodesk                         Refresh  Run all
───────────────────────────────────────────────────────────────
FORMAT  Format check   cargo fmt ...                  Not run  Run
LINT    Lint           cargo clippy ...               Failed   Run
TEST    Tests          cargo test ...                 Passed   Run
───────────────────────────────────────────────────────────────
Lint                                      Failed  3.2s  exit 101
STDERR
...
```

The list emphasizes:

- category;
- human label;
- exact configured command;
- latest in-memory result;
- duration;
- explicit Run action.

Detailed output appears only for the selected completed task.

The panel stays mounted while switching Problems / Tasks / Output / Terminal so recent task results are not destroyed by tab navigation. The task query itself remains lazy and is enabled only when Tasks is active.

Project changes clear local task results.

## Structured Problems ingestion

Task Runner results feed the RD2-13 Problems model directly.

This is more precise than reconstructing diagnostics from generic action history because the frontend receives the exact structured result for the exact project task that ran.

Supported v0 compiler shapes remain:

### Rust / Cargo / Clippy

```text
error[E0382]: borrow of moved value
  --> crates/foo/src/lib.rs:42:7
```

### TypeScript

```text
src/foo.ts(20,12): error TS2322: ...
```

### Colon form

```text
src/foo.ts:20:12: error: ...
```

A single task run replaces the current `check` Problems bucket with that task's diagnostics.

`Run all` parses every result and publishes the aggregate once:

```text
format diagnostics
+ lint diagnostics
+ typecheck diagnostics
+ test diagnostics
= current check Problems
```

A failed / timed-out / blocked task with no file-backed diagnostic still produces one non-navigable Problem so failure cannot disappear from the Problems surface.

A passing task with no warnings/errors clears its single-task check diagnostics.

## Workbench navigation

Bottom-panel navigation is expressed through a small intent event rather than lifting all bottom-panel tab state into `App.tsx`:

```text
requestBottomPanelTab("tasks")
```

The WorkbenchBottomPanel remains the owner of its tab lifetime.

This provides a clean path for command palette or contextual buttons to open Tasks / Problems later without creating another global navigation store.

## Editor end-of-file correction

The same slice contains the screenshot-driven final gutter correction.

The previous editor implementation had:

```css
padding-bottom: 28px;
```

on both textarea and gutter. That padding was part of the scrollable source area and therefore created artificial blank content after the last real line.

It also led to progressively more complicated gutter compensation code.

RD2-14 removes that fake source padding entirely:

```text
textarea bottom padding = 0
gutter bottom padding   = actual horizontal scrollbar height only
gutter scrollTop        = textarea scrollTop 1:1
```

When a classic horizontal scrollbar consumes textarea viewport height, the gutter receives exactly that physical height as invisible scroll-range compensation. With overlay scrollbars the value is zero.

This preserves exact line alignment while allowing the last real line to reach the actual bottom of the editor viewport.

There is no proportional scroll mapping and no accumulated compensation.

## Performance contract

Task Runner adds no polling.

- task discovery is lazy;
- task list is capped;
- result output sent to React is bounded;
- recent task results are in-memory only;
- Tasks remains inside the existing bottom dock;
- Problems keeps its existing 500-diagnostic bound;
- Code gutter remains one `<pre>` string rather than one React node per source line.

## Non-goals

RD2-14 intentionally does not add:

- arbitrary custom shell command entry;
- package.json script discovery beyond existing RepoDesk project checks;
- task dependency graphs;
- parallel task execution;
- persisted task-run history;
- streaming stdout/stderr;
- cancellation UI;
- watch tasks;
- automatic task execution on save;
- a new permanent Activity Rail destination;
- LSP diagnostics;
- any rule allowing manual Task results to satisfy Work Verification or Commit Gate requirements.

Those should be separate slices with explicit authority and performance contracts.

## Follow-up

The natural next step is RD2-15 LSP.

LSP should publish diagnostics into the existing Problems model and reuse exact Code location navigation rather than build another diagnostic surface.

Later Settings as Code can own editable task/check configuration while preserving this runner's backend allowlist and task-id revalidation.
