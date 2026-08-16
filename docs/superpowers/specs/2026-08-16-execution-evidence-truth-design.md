# Execution Evidence Truth Design

## Problem

A coding-agent process can launch, then fail while capturing its post-run changeset. The executor now preserves a durable `CodingAgentExecution`, but the orchestrator drops `execution_issues` and reduces an unavailable changeset to `changed_files = []`. Downstream receipts and Review therefore cannot distinguish “capture proved zero writes” from “capture failed, writes unknown”. This is a provenance bug: absence of evidence is being interpreted as evidence of absence.

## Invariants

1. `changed_files = []` may prove “no writes” only when changeset evidence is explicitly `complete`.
2. Missing/failed changeset capture is `unavailable`, never “none”.
3. Historical receipts/runs that predate the field deserialize as `legacy_unknown` and are not review-safe for write-capable steps.
4. Changeset evidence quality propagates executor → orchestration result → workflow receipt → execution-evidence state → Review UI.
5. Persistence recovery and evidence-content incompleteness are separate states. `RecoveryRequired` means persistence can potentially be replayed without rerunning an agent; `Incomplete` means the persisted content itself cannot prove what changed and normally requires rerun.
6. Review fails closed for every write-capable step whose changeset evidence is not `complete`.
7. `ExecutionReceipt::succeeded()` requires `Ok` plus complete changeset evidence for required write steps.
8. Execution issues persisted beyond the executor are bounded and secret-redacted; they are diagnostics, not authority. Changeset provenance is a typed field.
9. Dry runs remain `NotRequired`.
10. A single public execution-evidence boundary owns workflow receipt finalization. The raw runner may persist historical run JSON but must not silently create a second workflow receipt.

## Data model

Introduce `ChangeEvidenceStatus` with serialized values `complete`, `unavailable`, and `legacy_unknown`. Default deserialization is `legacy_unknown` so old JSON is conservative.

`CodingAgentExecution.change_evidence_status` is `complete` only when `capture_changeset` succeeds. It is `unavailable` when capture fails.

`SubAgentResult.change_evidence_status` defaults to `legacy_unknown`. Non-coding/read-only/provider results can stay `complete` only where no write provenance is required; write-capable coding-agent results must carry executor truth.

`SubAgentResult.execution_issues` carries bounded, already redacted diagnostic strings from the executor.

`StepReceipt.change_evidence_status` mirrors the result. `ExecutionEvidenceStatus` gains `Incomplete`.

## Evidence-state semantics

- `Ready`: exact persisted receipt matches the run and every required write step has complete change evidence.
- `RecoveryRequired`: exact workflow receipt is missing/corrupt/mismatched; a recovery payload may repair persistence without executing the agent again.
- `Incomplete`: receipt/run are structurally bound, but one or more required write steps has `unavailable` or `legacy_unknown` change evidence. Review must not accept/reject this run as if the changeset were complete.
- `NotRequired`: dry run.

## UX

Review must never render unavailable evidence as “No tracked file changes”. For incomplete evidence it should explain that RepoDesk cannot prove which paths changed and the execution must be rerun to obtain trustworthy changeset evidence. Persistence-recovery copy remains distinct and continues to say repair without rerun when a durable recovery payload exists.

## Compatibility

Serde defaults preserve loading old run/receipt JSON, but old write-capable evidence becomes `legacy_unknown` and therefore `Incomplete`. This intentionally trades convenience for provenance correctness.

## Verification

Add Rust unit tests for typed propagation, legacy JSON fallback, receipt success semantics, match semantics, and `Incomplete`; architecture ratchet assertions for the new provenance boundary; Playwright/native E2E coverage for user-visible incomplete-evidence copy. Run rustfmt, clippy, Rust tests, frontend build/tests, coverage, secret scans, cargo-deny, Playwright and native Tauri/WebDriverIO on the exact PR head before squash merge.