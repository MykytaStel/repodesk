# RD2-10 — Runs & Evidence

RepoDesk Runs is an evidence workspace, not a chat history and not a provider leaderboard.

Its job is to reconstruct what happened during one engineering run and distinguish facts from missing or stale evidence.

## Evidence chain

```text
Work Item
  -> Context
  -> Workers
  -> ChangeSet
  -> Human Review
  -> Verification
  -> Acceptance Evidence
  -> Commit
```

Each projection must expose its source. Canonical workflow receipts take precedence over historical event reconstruction when both exist.

## Authority hierarchy

### Persisted orchestration run

Authoritative for execution facts:

- run identity;
- worker steps;
- provider/model attribution;
- token/cost telemetry;
- worker-reported changed-file set.

### TaskRunReceipt

Canonical post-execution evidence for the current run:

- reviewed path set;
- review decision;
- ChangeSet digest;
- verification commands and outcomes;
- verification HEAD/index binding;
- bounded commit receipt.

### Engineering Event Ledger

Historical fallback for runs whose canonical TaskRunReceipt has already been superseded.

Event-derived data must be labeled as such. Missing command-level evidence must remain missing rather than being reconstructed from prose.

## Acceptance Evidence v0

Acceptance criteria come from the typed Engineering Contract.

A criterion can be:

```text
PROVEN
FAILED
UNPROVEN
```

RepoDesk does not infer `PROVEN` from agent output, model confidence, a summary, or a generic "done" claim.

In v0 a human explicitly links a criterion to one concrete command in the canonical VerificationReceipt. The criterion status then follows the recorded command result:

```text
linked command passed -> PROVEN
linked command failed -> FAILED
no current evidence    -> UNPROVEN
```

This link is an explicit human evidence mapping. Later versions may add richer typed proof sources such as test-case IDs, diagnostics, benchmarks, CI checks, or externally signed receipts.

## Freshness

Before commit, verification evidence is current only for the exact:

```text
run id
+ ChangeSet digest
+ HEAD
+ staged index tree
```

A different HEAD or staged tree makes linked acceptance evidence stale.

A normal RepoDesk bounded commit necessarily moves HEAD. After that commit, evidence remains valid only if the tree of the recorded FinishReceipt commit is exactly the tree that was verified before commit.

This preserves proof across the expected commit transition without allowing proof to drift onto different code.

## Historical runs

Historical Runs are forensic views.

If the current canonical receipt no longer belongs to the selected run, RepoDesk may use run-scoped engineering events as fallback for review, verification, and commit facts. It must not invent command-level verification details that were not durably recorded.

Acceptance bindings are filtered by `run_id`, so one run cannot inherit another run's proof for an identically worded criterion.

## Performance contract

The Runs list loads lightweight persisted summaries only.

Detailed evidence is fetched only for the selected run. Work/Inspector polling does not load run details by default, and the selected run projection reuses the engineering event slice already read by the aggregate snapshot.

This keeps history size from scaling React memory and JSONL I/O linearly with the number of runs visible in the sidebar.

## Non-goals for v0

- AI-generated semantic proof;
- automatic claim that a broad test suite proves every criterion;
- permanent storage of every stdout/stderr stream in React state;
- replacing canonical workflow receipts with telemetry events;
- provider scoring as the primary Runs experience.

## Next evidence sources

Future slices can extend the same acceptance model with typed sources:

- named test cases;
- compiler/linter diagnostics;
- benchmark thresholds;
- security/architecture checks;
- CI checks;
- screenshots or UI test artifacts;
- external issue/requirement references.

The invariant remains the same: a result is only as strong as the inspectable evidence attached to it.
