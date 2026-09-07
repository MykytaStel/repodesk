# Phase 2: Measured Verification Catalog

Status: design approved by product owner; implementation pending spec review

## Intent

Make RepoDesk's verification advisor useful with evidence from the repository's
own check executions. RepoDesk should be able to answer:

> What checks exist, what do they prove, how long do they usually take, and
> what is the smallest defensible check to run for this change?

The advisor remains a decision surface. It recommends and records a choice; it
does not silently execute, skip, or claim that a check is cheap.

## Problem statement

The execution layer already produces per-command facts in `CheckCommandResult`,
including duration, status, exit code, tree identity, timestamps, and a log
reference. The current verification event only retains aggregate success and
command count, so the advisor can count attempts but cannot learn the cost or
reliability of an individual check.

The current project configuration also stores checks as bare strings. That
makes check IDs dependent on array position and leaves no durable place for
requiredness, path relevance, timeout, or an operator-facing title.

## Goals

1. Give every configured check a stable identity that survives reordering.
2. Preserve existing project configurations and existing verification events.
3. Record per-check execution facts in the canonical append-only event ledger.
4. Derive explainable per-check history from those events.
5. Use measured history to estimate wall-clock time and confidence.
6. Keep AI/token cost separate from local verification time; do not fabricate a
   cost value.
7. Make an empty check catalog explicit and recoverable in the desktop UX.
8. Keep execution explicit: recording a recommendation must never run a check.

## Non-goals

- Automatically running or skipping checks based on the recommendation.
- Inferring relevant paths from command text or from a guessed test framework.
- Parsing arbitrary test output to invent test counts.
- Introducing a machine-specific monetary or token cost model for local checks.
- Replacing the existing receipt-bound Verify/Finish safety gates.
- Building a global productivity or quality score.

## Proposed domain model

### Project check descriptor

Replace the runtime assumption that a check is only a string with a normalized
descriptor:

- `id`: stable operator-facing identifier.
- `title`: short human-readable name; command is the fallback.
- `command`: existing allowlisted command string.
- `kind`: `test`, `lint`, `typecheck`, `security`, or `check`.
- `required`: whether this check is mandatory before acceptance.
- `relevant_paths`: explicit repository-relative paths or directory prefixes.
- `timeout_secs`: bounded execution timeout, defaulting to the current 120s.

There is deliberately no cost field in this descriptor. Local duration is an
execution-time fact, not an AI spend measurement.

### Configuration compatibility

Existing TOML such as:

```toml
checks = ["cargo test --all"]
```

must continue to load. Bare strings are normalized into descriptors with:

- a generated stable ID from the normalized command;
- `title = command`;
- an inferred kind using the existing conservative classifier;
- `required = false`;
- empty `relevant_paths`;
- `timeout_secs = 120`.

Structured entries may add the descriptor fields without forcing an immediate
rewrite of every existing project file. Serialization should not silently
change a legacy project unless the operator edits or explicitly adopts the
catalog.

Generated IDs must not depend on array position. The preferred form is
`check-<short-sha256-of-normalized-command>`. An explicitly configured ID wins,
is validated as non-empty, and remains stable when the command is reordered.
Changing a command without retaining the explicit ID intentionally creates a
new history identity; the old history must not be presented as evidence for a
different command.

### Default project types and empty catalogs

`rust-tauri` must be treated like the existing Rust desktop project types when
new projects receive defaults. Existing projects with `checks = []` must not
be mutated silently. The desktop project surface should instead show an
explicit empty-catalog state with a safe action to add the recommended checks
for the detected project type, plus the ability to add an allowlisted command
manually.

## Execution facts and event contract

The existing `VerificationFinished` event remains the aggregate verification
boundary. Its existing attributes remain available for older readers:

- `success`;
- `command_count`;
- existing verification evidence references;
- optional `error`.

Add an optional `check_results` attribute containing one bounded record per
executed check:

- `check_id`;
- `command`;
- `status` (`passed`, `failed`, or `timeout`);
- `exit_code` when the process supplied one;
- `duration_ms`;
- `started_at` and `finished_at`;
- `tree_identity` when available;
- `log_evidence_ref` when available;
- `tests_observed` only when a trusted parser supplies it; otherwise null.

Do not store full stdout/stderr in the event. Existing bounded logs and summary
files remain evidence locations, and the event stores references to them.

Older `VerificationFinished` events without `check_results` remain valid. They
contribute to aggregate attempt/failure counts but cannot be represented as
measured per-check history.

## History projection

Build a deterministic projection from all replayed `VerificationFinished`
events. For each stable check ID expose:

- total measured runs;
- failed runs;
- latest status and timestamp;
- latest tree identity when present;
- median duration for non-timeout runs;
- whether the estimate is `unknown`, `provisional`, or `calibrated`.

Rules:

- `unknown`: no per-check duration is available;
- `provisional`: one or two usable duration observations;
- `calibrated`: at least three usable observations;
- timeout durations are retained as operational facts but are excluded from the
  normal duration estimate because they are censored at the timeout boundary;
- duplicate records for the same verification/check identity must not inflate
  the projection;
- history is evidence for the command identity, not proof that the current tree
  will pass.

The projection must remain rebuildable from the canonical ledger; no second
history database is introduced.

## Advisor behavior

The advisor consumes descriptor metadata and the history projection:

- `required` checks still take precedence;
- explicit `relevant_paths` enable targeted recommendations;
- a selected check's estimated wall-clock time is its measured median;
- the total recommendation estimate is the sum of selected check estimates;
- if any selected check has no estimate, the total remains unknown;
- confidence is `unknown` if a selected check has no history, `provisional` if
  its history is thin, and `calibrated` only when all selected checks meet the
  three-observation threshold;
- `estimated_cost_units` remains null unless a future, separately specified
  cost source exists;
- repeated failures may still result in `ask_for_approval`, but the rationale
  should identify whether the failures are measured per-check or only aggregate.

The decision receipt continues to record the selected stable IDs, deferred
debt, wall-clock estimate, uncertainty, and policy version. It must not imply
that a recorded recommendation has executed anything.

## Desktop UX

The Work control surface should make the source quality visible:

- show measured duration and sample count alongside each actionable check;
- distinguish `Unknown`, `Provisional`, and `Calibrated` rather than treating
  all non-unknown values as equally trustworthy;
- label local verification time separately from AI spend;
- show the empty-catalog explanation and setup action instead of a dead-end
  `No checks` recommendation;
- retain an evidence-inspection path to the verification summary/log;
- preserve the existing explicit decision recording flow.

## Implementation slices

1. Normalize project checks and add stable IDs without changing execution
   behavior.
2. Extend verification telemetry and event serialization with bounded
   per-check facts.
3. Add the deterministic history projection and advisor integration.
4. Add project check setup for empty catalogs and `rust-tauri` defaults.
5. Update the Work UI and Runs evidence presentation.

Each slice must keep legacy string configs and old events readable.

## Verification requirements

### Core tests

- legacy string configuration normalizes to the same stable ID regardless of
  array position;
- structured descriptors validate IDs, commands, paths, and timeout bounds;
- `rust-tauri` receives the expected defaults for newly created projects;
- per-check telemetry round-trips through the typed event payload;
- history derives correct run/failure counts, latest status, median, and
  confidence thresholds;
- timeout samples do not become false duration estimates;
- duplicate verification/check records do not inflate history;
- older aggregate-only events remain readable and produce unknown per-check
  history;
- advisor sums measured wall-clock estimates while leaving cost unknown.

### Desktop tests

- the advisor renders measured, provisional, calibrated, and unknown states;
- selected and deferred check IDs remain stable after reordering;
- an empty catalog presents setup guidance;
- recording a decision does not invoke verification execution.

### Gates

Run the focused core and desktop tests first, then the repository's full
`./scripts/verify-all.sh` gate. Perform a real desktop visual check of the Work
surface and verify that the active RepoDesk project can add and run at least
one configured check before calling this phase complete.

## Acceptance criteria

Phase 2 is complete when a configured project can run a check twice, RepoDesk
can show that check's stable identity, latest status, measured duration, and
sample count in Work/Runs, and the Advisor can use that history to produce a
time estimate without claiming an AI cost. A project with no checks receives a
clear setup path rather than a recommendation that pretends verification is
available.
