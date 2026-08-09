# RepoDesk 2 Runtime Performance Budget

RepoDesk is a persistent engineering workspace. A feature that is cheap once can
still become expensive when it stays mounted for hours, polls the filesystem,
serializes large IPC payloads, or invalidates unrelated query families.

This document defines the first runtime budget for RepoDesk 2.

## Core rule

> Stable workspace state should be quiet.

When the user is reading code, evidence, or a completed run, RepoDesk should not
continuously wake Rust, Git, SQLite, the filesystem, or model/provider probes
unless there is a live operation whose state can actually change.

## Background work classes

### Allowed persistent work

- the application shell;
- an explicitly created PTY session;
- cheap in-memory UI state;
- bounded command timing/status metadata;
- listeners required to preserve an active native session.

### Conditional work

Polling is acceptable only while a state is genuinely live, for example:

- an executor is running;
- verification is running;
- an external process is expected to mutate a receipt before the current IPC
  returns;
- a user explicitly enables a monitor.

Once the state becomes terminal, polling must stop.

### On-demand work

The following should normally run on mount, explicit refresh, focus after a
stale window, or domain mutation invalidation:

- Git workspace snapshots;
- persisted run history;
- engineering event-ledger projections;
- repository indexing;
- RepoPilot history;
- SQLite diagnostics;
- model/provider health;
- token/cost analytics.

## Query invalidation

A mutation should invalidate the smallest domain that can have changed.

Current domain families:

```text
workspace
work
git
code
runs
providers
system
```

Avoid `queryClient.invalidateQueries()` without a key for normal workflow
mutations. Global invalidation is reserved for explicit user-level operations
such as `Refresh workspace` or a project switch where every project-scoped
projection can legitimately be stale.

## IPC instrumentation

Every `callCommand()` may emit cheap metadata:

```text
command
status
duration
timestamp
```

Result payload previews are opt-in and are only produced while an explicit
Debug consumer is mounted.

Debug previews are bounded by:

- string length;
- array item count;
- object key count;
- recursion depth;
- sensitive-key omission.

Keys matching content/source/prompt/secrets/passwords/authorization/API keys or
tokens are not copied into Debug previews.

This is both a performance and local-data hygiene rule.

## Shell / bottom panel budget

The bottom panel remains mounted because PTY sessions must survive hide/show.
That does **not** mean all of its data sources should eagerly load.

Rules:

- historical action output loads only after the panel is first opened;
- panel log state is capped at 150 records;
- historical stdout is not retained in React state;
- error excerpts are bounded;
- Debug owns rich payload previews, Output owns lightweight operational metadata.

## Workspace identity budget

Normal `useWorkspace()` consumers need project/task identity, not database
health diagnostics.

`db_status` is opt-in and currently requested by Debug only.

## Git budget

Git workspace state is shared through one TanStack Query key. A short stale
window coalesces repeated consumers/focus events so opening several surfaces does
not immediately spawn duplicate Git snapshots.

Filesystem/repository-wide indexing belongs to Code Workspace and must not be
used just to answer "which files changed?". Changed-file surfaces reuse the
canonical Git snapshot.

## Runs budget

Persisted orchestration runs have terminal `RunStatus` values:

```text
completed
partial
failed
dry_run
```

Therefore completed run history must not poll.

Before this slice, the Runs surface performed:

```text
orchestration_runs    every 5s
run evidence detail  every 5s
```

That is 24 periodic IPC calls per minute while simply reading historical
evidence.

Runtime Budget v0 removes both intervals. Runs now refresh through:

- query staleness;
- window focus;
- explicit Refresh;
- mutations that update the selected evidence cache.

## Inspector budget

Inspector is an on-demand drawer. It must not continuously project the
engineering event ledger while the user is only reading it.

Before Runtime Budget v0 it refreshed the engineering aggregate every 6 seconds
(10 reads/minute while open). The interval is removed; normal workflow mutations
invalidate the shared engineering query.

## Memory limits introduced/retained

```text
Debug command traces          100
Bottom-panel logs             150
Debug string preview          2,000 chars per string
Debug array sample            16 items
Debug object sample           24 keys
Debug traversal depth         3
Code open files               8
Code file size                512 KiB
```

These are product safety rails, not benchmark targets. They should be adjusted
only from profiling evidence.

## Review checklist for future PRs

Before adding a query, listener, watcher, or retained cache, answer:

1. Is this state live or terminal?
2. Why must it poll instead of invalidate/refetch on demand?
3. What native work does one refresh trigger: Git, filesystem, SQLite, event
   ledger, process launch, or network?
4. Is the result duplicated or serialized for a hidden surface?
5. What bounds its retained memory?
6. Can a shared query serve multiple surfaces?
7. Does this mutation really require global cache invalidation?
8. What happens after the app remains open for eight hours?

## Next profiling pass

Runtime Budget v0 removes known structural waste without claiming benchmark
numbers that have not been measured.

The next optimization pass should add measured baselines for:

- cold desktop startup;
- idle IPC calls/minute per primary surface;
- Git processes/minute;
- engineering ledger reads/minute;
- Code repository-index latency at 1k / 10k / 50k files;
- frontend JS heap after opening/closing primary surfaces repeatedly;
- PTY memory after long-running sessions;
- frontend bundle/chunk sizes;
- Code editor engine comparison before adopting Monaco/CodeMirror.
