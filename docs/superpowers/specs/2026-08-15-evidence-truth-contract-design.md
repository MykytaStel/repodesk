# Change Evidence Truth Contract

Date: 2026-08-15
Base: `main@2870301a5520112bc1918745fe3c02491b59a9c4`

## Problem

RepoDesk now preserves a `CodingAgentExecution` after post-launch evidence failures, including `execution_issues`. That fixes the earlier failure mode where a launched agent could disappear behind a generic executor error.

The next trust failure is downstream: orchestration currently converts the executor receipt into `SubAgentResult` without preserving `execution_issues`, and an empty `changed_files` vector is rendered as `changed files: none (no writes detected)`. The same empty vector can mean two materially different things:

1. Git provenance completed and proved that the agent produced zero file changes.
2. Change provenance failed or was unavailable, so RepoDesk does not know which files changed.

That ambiguity survives into `StepReceipt` and the Review UI. A loss of evidence can therefore become false certainty.

There is a second boundary problem: evidence-readiness checks exist at workflow level, but `review_run()` itself is a mutating core API and does not currently enforce that gate before applying/staging/restoring paths. A direct Review command must not be able to bypass evidence quality checks.

## Goal

Make change provenance a first-class typed contract from executor runtime through orchestration, durable workflow receipts, desktop IPC, and Review UI.

RepoDesk must never infer “no writes” from missing evidence, and no Review mutation may occur before the core review boundary proves the execution evidence is ready.

## Non-goals

- Redesign every executor diagnostic into one generic evidence ledger.
- Change provider routing, approval policy, worktree creation, or token/cost accounting.
- Make incomplete evidence recoverable by reconstructing a past Git delta after the fact.
- Require a unified textual diff for every changed file; binary/untracked files can legitimately have path evidence without a unified diff.
- Migrate historical run JSON on disk eagerly.

## Chosen approach

Add an explicit `ChangeEvidenceStatus` and propagate it end-to-end.

The public states are:

- `complete`: Git pre/post provenance completed. `changed_files=[]` is now a proven “no writes detected”.
- `unavailable`: the current run expected change provenance but RepoDesk could not establish it. The result must carry an explanatory issue and cannot be reviewed as a trustworthy changeset.
- `legacy_unknown`: a persisted pre-contract result/receipt lacks evidence-quality metadata. For a write-capable step this is deliberately not trusted.
- `not_applicable`: no filesystem-change evidence is expected for the step, e.g. a normal completion-provider step. This prevents old/non-writing steps from being mislabeled as degraded evidence.

`not_applicable` is an internal completeness state added to the previously approved three-state write-evidence model. Review gating only cares about steps whose change evidence is expected.

## Architecture

### 1. Executor runtime owns provenance truth

`CodingAgentExecution` gains `change_evidence_status`.

For new coding-agent executions the runtime may emit only:

- `complete`
- `unavailable`

`legacy_unknown` is deserialization compatibility only; `not_applicable` is for non-coding-agent orchestration results.

A Git pre-status snapshot and a successful post-status delta are the proof boundary. If the pre-snapshot is absent because the working directory is not a Git work tree, or post-run provenance capture errors, change evidence is `unavailable` rather than an empty successful changeset.

Post-launch provenance failure still returns the executor receipt, appends an `execution_issues` entry, and forces execution status to `failed`. The agent execution is historical fact; evidence quality is a separate typed fact.

A missing persisted `.diff` file does not by itself make path provenance unavailable. The path set is the authority for change attribution; textual diff persistence remains a presentation warning because new/binary files may also lack a normal unified diff.

### 2. Orchestration preserves evidence instead of reconstructing it

`SubAgentResult` gains:

- `change_evidence_status`
- `execution_issues`

New non-coding-agent results explicitly use `not_applicable`.

Coding-agent finalization copies both fields from `CodingAgentExecution`. It must not derive evidence quality from `changed_files.is_empty()` or parse free-form notes.

Human notes remain useful presentation metadata, but the wording becomes source-aware:

- `complete + []` -> `changed files: none (proven by Git provenance)`
- `complete + files` -> existing changed-files summary
- `unavailable` -> `change evidence unavailable; no no-write claim can be made`
- `legacy_unknown` -> `change evidence quality unknown for legacy run`

Generic `execution_issues` are preserved as structured data and may also be summarized in notes for existing surfaces.

### 3. Durable receipts record evidence quality

`StepReceipt` gains, with serde-compatible defaults:

- `change_evidence_status`
- `execution_issues`

Existing receipt JSON without these fields deserializes as `legacy_unknown` plus an empty issue list.

When building a new execution receipt, RepoDesk copies the exact `SubAgentResult` evidence state. `execution_receipt_matches_run` compares evidence status and structured issues in addition to status and changed paths so a receipt cannot silently disagree with the persisted run.

`changeset_digest` keeps its current meaning: identity of a known non-empty path set. A complete zero-change run can validly have no digest. An unavailable/legacy-unknown write step can also have no digest, but is distinguished by evidence status and is never treated as reviewable.

### 4. Execution evidence state distinguishes persistence failure from incomplete provenance

`ExecutionEvidenceStatus` gains `Incomplete`.

Semantics:

- `Ready`: receipt exists, matches the run, and every write-capable step has `change_evidence_status=complete`.
- `Incomplete`: the run/receipt exists, but at least one write-capable step has `unavailable` or `legacy_unknown` change evidence.
- `RecoveryRequired`: execution evidence persistence is missing/corrupt/mismatched and may be repaired from the existing durable recovery payload.
- `NotRequired`: dry run.

This distinction matters operationally. `RecoveryRequired` is a persistence problem that RepoDesk may repair without rerunning the agent. `Incomplete` is an observation-quality problem; this slice does not pretend it can reconstruct the exact historical delta, so safe remediation is a new execution.

`require_review_evidence_ready` accepts only `Ready` and fails closed for `Incomplete`, `RecoveryRequired`, and `NotRequired`, with remediation-specific messages.

Crucially, `review_run()` must call this gate at the top of the core mutation boundary, before resolving review paths or mutating either the active checkout or an isolated worktree. The Tauri `orchestrate_review` command therefore inherits the invariant automatically instead of duplicating it in UI/IPC code. Receipt-recording paths must also refuse to persist a new Review decision for an execution that is not evidence-ready.

### 5. Desktop IPC exposes evidence quality

`RunDiff` gains:

- `change_evidence_status`
- `execution_issues`

`orchestrate_run_diffs` no longer filters an evidence-bearing coding-agent result out merely because `changed_files` and `diff_path` are empty. A row with `unavailable` or `legacy_unknown` must reach the UI.

`CheckProof` step data also carries change-evidence status so diagnostic/proof views cannot collapse unknown into zero changes.

No secret values are added to IPC. Evidence issues are infrastructure diagnostics already persisted in the run record; they are not derived from provider credential values.

### 6. Review UI is provenance-aware

Review must render three materially different outcomes:

- Complete with files: show the recorded file list/diff as today.
- Complete with zero files: show an affirmative but qualified message such as `No writes detected — Git provenance complete.`
- Unavailable/legacy unknown: show a blocking warning such as `Change evidence unavailable. RepoDesk cannot prove what the agent changed; rerun execution before Review.`

The current blanket empty state `No tracked file changes captured for this run.` must not be used to describe evidence failure.

Review actions remain governed by the backend fail-closed gate; UI disabling/messaging is defense in depth, not the security boundary.

## Backward compatibility

Historical run and receipt JSON must continue to deserialize.

Missing `change_evidence_status` becomes `legacy_unknown`, never `complete`. This intentionally makes historical write-capable executions non-reviewable under the new stronger contract unless they already completed their historical workflow before this version. Completed receipts remain historical records; the new gate applies when an operator attempts a new Review action.

Non-writing steps created by current code explicitly serialize `not_applicable`, so they do not poison execution evidence readiness.

## Error handling and invariants

1. Empty `changed_files` is never sufficient evidence for “no writes”.
2. Only `change_evidence_status=complete` may support that claim.
3. A current coding-agent run with unavailable Git provenance is not successful automation, even if the child process exits zero.
4. Receipt persistence failure and change-provenance incompleteness remain separate states with separate remediation.
5. `review_run()` cannot mutate the active checkout or isolated workspace unless execution evidence is `Ready`.
6. A new Review receipt cannot be recorded unless execution evidence is `Ready`.
7. Free-form notes are never parsed to determine trust state.
8. Legacy missing metadata defaults toward uncertainty, never success.

## Testing strategy

### Core/unit

- Runtime/change-capture contract: unavailable provenance cannot produce a `complete` empty changeset.
- Coding-agent -> `SubAgentResult` propagation preserves `change_evidence_status` and `execution_issues`.
- A complete coding-agent result with zero changed files remains `complete` and may state proven no writes.
- Receipt serialization/deserialization defaults old missing status to `legacy_unknown`.
- Receipt matching rejects evidence-status/issue disagreement.
- `ExecutionEvidenceStatus::Ready` requires every write-capable step to have complete change evidence.
- `unavailable` and `legacy_unknown` write steps produce `Incomplete`, not `RecoveryRequired`.
- Non-writing `not_applicable` steps do not block readiness.
- `review_run()` rejects `Incomplete` before any checkout/worktree mutation.
- Review receipt recording rejects an incomplete execution.

### Desktop command

- `orchestrate_run_diffs` returns an unavailable/legacy evidence row even with no paths/diff receipt.
- Complete zero-change evidence is distinguishable from unavailable evidence.
- Check proof exposes the typed evidence state.

### Frontend / Playwright

- Complete zero-change run shows `No writes detected — Git provenance complete.`
- Unavailable evidence shows a blocking provenance warning and never the ordinary no-change empty state.
- Legacy unknown shows an explicit legacy/unknown warning.

### Regression / architecture ratchet

Add a source invariant preventing the old unqualified `changed files: none (no writes detected)` claim from returning in the coding-agent finalization path, require the typed evidence field in executor/orchestration/receipt contracts, and require the core review mutation path to call the evidence-readiness gate.

## Verification contract

Before merge, the exact PR head must pass:

- Architecture Ratchet
- frontend production build
- `cargo fmt --all -- --check`
- Clippy with warnings denied
- full Rust tests
- strict basic secret scan
- gitleaks
- cargo-deny
- coverage job
- Playwright mock-IPC E2E
- native Tauri/WebDriverIO E2E

The PR is squash-merged only after those checks are green on the exact final head and the base still matches current `main`.