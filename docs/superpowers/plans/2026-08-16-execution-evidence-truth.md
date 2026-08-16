# Execution Evidence Truth Implementation Plan

> **Approved design:** `docs/superpowers/specs/2026-08-16-execution-evidence-truth-design.md`

## Goal

Make RepoDesk's execution evidence truthful end-to-end. An empty `changed_files` list must mean “proven zero writes” only when changeset capture completed. Failed or historical/unknown capture must remain explicitly incomplete through executor output, orchestration run JSON, workflow receipts, autonomous-loop classification, Tauri IPC, and Review UI.

## Architecture

Introduce one neutral, shared changeset-provenance type in `repodesk-core`, then propagate it without re-deriving meaning at downstream boundaries. The public `orchestrator::execution_evidence` module remains the sole owner of workflow-receipt finalization; the raw runner persists run history only. Persistence failure (`RecoveryRequired`) remains separate from content/provenance failure (`Incomplete`). Review and autonomous execution fail closed when evidence is incomplete.

## Tech stack

Rust (`serde`, existing core/orchestrator/workflow modules), Tauri command bridge, React + TypeScript + TanStack Query, Playwright mock-IPC E2E, existing WebDriverIO native E2E, Node architecture ratchet, GitHub Actions.

## Global constraints

- Preserve backward-compatible deserialization of historical run/receipt JSON.
- Historical write-capable evidence missing the new field must deserialize as `legacy_unknown` and must not be review-safe.
- Do not persist raw secrets in diagnostic issue text. Redact with the existing secret detector and bound issue count/length.
- Do not weaken secret scanners, source-size budgets, review gates, or human approval gates.
- Do not collapse `RecoveryRequired` and `Incomplete`; only the former may be repaired without rerunning the agent.
- Keep `changed_files = []` valid proof of no writes only with `ChangeEvidenceStatus::Complete`.
- Do not add a second receipt writer to the raw runner.
- New behavior is test-first. Production edits follow a failing regression test.

## File map

- Create: `crates/repodesk-core/src/change_evidence.rs` — shared `ChangeEvidenceStatus` contract.
- Modify: `crates/repodesk-core/src/lib.rs` — export neutral provenance module.
- Modify: `crates/repodesk-core/src/executors.rs` — add provenance to `CodingAgentExecution`.
- Modify: `crates/repodesk-core/src/executors/runtime.rs` — set `complete` vs `unavailable`; sanitize/bound execution issues.
- Modify: `crates/repodesk-core/src/executors/tests.rs` — executor truth regressions.
- Modify: `crates/repodesk-core/src/orchestrator/types.rs` — propagate provenance/issues with conservative serde defaults.
- Modify: `crates/repodesk-core/src/orchestrator/runner.rs` — preserve executor truth, truthful notes, remove duplicate workflow-receipt writer.
- Modify: `crates/repodesk-core/src/workflow/receipt.rs` — persist step provenance and require complete write evidence for success.
- Modify: `crates/repodesk-core/src/orchestrator/execution_evidence.rs` — add `Incomplete`, classify structurally matching-but-untrustworthy receipts, fail Review closed.
- Modify: `crates/repodesk-core/src/orchestrator/auto_loop.rs` — never classify incomplete evidence as success; retry/terminate according to evidence semantics without confusing it with persistence repair.
- Modify: `apps/desktop/src-tauri/src/commands/orchestrate.rs` — expose evidence state through a bounded read-only IPC command.
- Modify: `apps/desktop/src-tauri/src/lib.rs` — register the evidence-state command.
- Modify: `apps/desktop/src/shared/api/orchestrate.ts` — typed provenance/evidence-state API.
- Modify: `apps/desktop/src/features/work/ReviewPanel.tsx` — fail-closed evidence UX and explicit truthful zero-change copy.
- Modify: `apps/desktop/e2e/fixtures.ts` — ready/incomplete/recovery evidence fixtures.
- Modify: `apps/desktop/e2e/work-golden-path.spec.ts` — Review evidence UX regressions.
- Modify: `scripts/check-source-architecture.test.mjs` and/or a focused source-boundary check only if needed to ratchet the single-owner/provenance invariant without growing production god-files.

## Task 1 — Shared provenance type and executor truth

- [ ] **RED:** extend `executors/tests.rs` so successful Git changeset capture expects `ChangeEvidenceStatus::Complete`, and post-launch changeset failure expects `Unavailable` rather than an untyped empty list.
- [ ] **RED:** add a serialization/default test showing missing provenance deserializes to `LegacyUnknown`.
- [ ] Run focused executor tests and confirm they fail because the type/field does not exist yet.
- [ ] Create `change_evidence.rs` with `Complete | Unavailable | LegacyUnknown`, `snake_case` serde, `Default = LegacyUnknown`, and a small `is_complete()` helper.
- [ ] Export the module from core `lib.rs` and add the field to `CodingAgentExecution`.
- [ ] In `runtime.rs`, set `Complete` only after `capture_changeset` succeeds; set `Unavailable` on capture error.
- [ ] Sanitize issue strings with existing `security::redact_secrets`, cap each persisted issue, cap issue count, then sort/deduplicate.
- [ ] Re-run focused executor tests and make them green.

## Task 2 — Orchestration propagation and truthful notes

- [ ] **RED:** add `orchestrator/types.rs` tests for legacy JSON defaulting to `LegacyUnknown` and bounded `execution_issues` serialization shape.
- [ ] **RED:** add/extend runner-focused tests so `Unavailable + []` never emits “no writes detected”, while `Complete + []` may emit an explicit complete-capture zero-write note.
- [ ] Run focused orchestrator tests and confirm RED.
- [ ] Add `change_evidence_status` and `execution_issues` to `SubAgentResult` with serde defaults.
- [ ] Make non-write base results evidence-complete where no write provenance is required; keep write-capable prelaunch/legacy results conservative.
- [ ] Propagate coding-agent provenance/issues into the result and gate zero-write notes on `Complete`.
- [ ] Re-run focused tests.

## Task 3 — Receipt contract and single workflow-receipt owner

- [ ] **RED:** extend `workflow/receipt.rs` tests so a write step with `Ok + Unavailable/LegacyUnknown` does not make `ExecutionReceipt::succeeded()` true; `Ok + Complete + []` does.
- [ ] **RED:** add a source-boundary regression that rejects a raw-runner `write_execution_receipt` owner if a focused architecture test is practical.
- [ ] Run focused tests and confirm RED.
- [ ] Add `change_evidence_status` to `StepReceipt` with conservative default.
- [ ] Update `ExecutionReceipt::succeeded()` to require complete evidence for successful required write steps.
- [ ] Update the public evidence receipt builder/matcher to copy and compare provenance.
- [ ] Remove the raw runner's best-effort workflow receipt write and delete its now-dead writer helper/imports. Raw runner continues persisting run JSON and outcome ledger records.
- [ ] Re-run receipt/orchestrator tests.

## Task 4 — Run-level evidence state and autonomous-loop correctness

- [ ] **RED:** add `execution_evidence.rs` tests for structurally matching complete receipt → `Ready`, matching unavailable/legacy write evidence → `Incomplete`, mismatch/missing semantics remaining recovery-oriented, and Review blocking incomplete evidence with rerun-specific copy.
- [ ] **RED:** extend `auto_loop.rs` classifier tests so an otherwise completed run with incomplete evidence cannot become `LoopStatus::Succeeded` and is not mislabeled as persistence recovery.
- [ ] Run focused tests and confirm RED.
- [ ] Add `ExecutionEvidenceStatus::Incomplete` and a pure classification helper separating structural match from content completeness.
- [ ] Update `evidence_state_for_run`, `finalize_execution_evidence`, `repair_execution_evidence`, and `require_review_evidence_ready` to preserve the distinction.
- [ ] Update autonomous-loop classification to treat incomplete evidence as rerun-required/non-success, while `RecoveryRequired` remains terminal repair-without-rerun.
- [ ] Re-run focused tests.

## Task 5 — Tauri contract and Review UI/UX

- [ ] **RED:** add Playwright fixture/test for Review with `incomplete` evidence asserting visible “cannot prove which paths changed / rerun execution” copy and asserting the old zero-change sentence is absent.
- [ ] **RED:** add a recovery fixture/test asserting persistence-repair copy is distinct from incomplete/rerun copy.
- [ ] Run the focused Playwright spec and confirm RED because the command/UI state does not exist.
- [ ] Add `orchestrate_evidence_state(run_id)` Tauri command and register it.
- [ ] Add TypeScript evidence/provenance types and `orchestrateEvidenceState()` API.
- [ ] In `ReviewPanel`, query evidence state alongside diffs and fail closed on query errors, `Incomplete`, `RecoveryRequired`, and `NotRequired`.
- [ ] For `Ready + []`, render explicit truth-preserving copy such as “Changeset capture is complete; no tracked file changes were produced.”
- [ ] Use an accessible status/alert surface; keep memory proposals independent but never imply the run is review-safe when evidence is not ready.
- [ ] Re-run focused Playwright spec.

## Task 6 — Architecture ratchet and compatibility sweep

- [ ] Add a focused architecture/source-boundary assertion that the public evidence module owns receipt finalization and that the raw runner contains no receipt writer.
- [ ] Add compatibility tests for historical run/receipt JSON missing the new field.
- [ ] Update all Rust/TS fixtures and constructors required by the new fields; prefer serde defaults over brittle migrations.
- [ ] Inspect compiler failures for every `SubAgentResult`, `StepReceipt`, and `CodingAgentExecution` constructor and fix explicitly rather than broad search/replace.
- [ ] Run architecture ratchet unit test and enforcement locally in CI through the PR.

## Task 7 — Full verification, review, PR, exact-head CI, squash merge

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run Clippy with workspace/all-target warnings-as-errors equivalent to CI.
- [ ] Run Rust workspace tests, including focused executor/orchestrator/workflow tests.
- [ ] Run desktop frontend typecheck/build/test commands used by CI.
- [ ] Run strict basic secret scan, gitleaks, cargo-deny, coverage gate, and source architecture ratchet.
- [ ] Run Playwright mock-IPC E2E including the new incomplete/recovery Review tests.
- [ ] Run the existing native Tauri/WebDriverIO E2E suite on the exact head; command registration/native startup must remain green.
- [ ] Self-review the final diff against this plan: no duplicated receipt owner, no secret-bearing diagnostics, no unknown→none collapse, no recovery/incomplete wording conflation.
- [ ] Open/update PR with exact scope and verification evidence.
- [ ] Wait for exact-head GitHub Actions. Inspect any failure by job logs; fix root cause and repeat exact-head verification.
- [ ] Re-read `main` and PR head immediately before merge; require mergeability and exact expected head SHA.
- [ ] Squash merge to `main`, then verify `main` points at the returned merge SHA.
