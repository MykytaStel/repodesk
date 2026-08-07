# RepoDesk Engineering Intelligence

## Purpose

Engineering Intelligence measures how software work is performed, not merely how many AI tokens were used.

The system should help answer:

- Was the task scoped well?
- Was context compact but sufficient?
- Were too many agents involved?
- Did several workers duplicate the same work?
- Did the workflow require avoidable retries?
- Did project knowledge reduce rediscovery?
- Did the changes respect scope and project rules?
- Did verification succeed on the first reviewed changeset?
- Did algorithmic complexity improve, regress, or remain uncertain?

Metrics must be derived from inspectable evidence. RepoDesk should prefer separate dimensions over one opaque score.

## Design rules

1. Facts first, scores later.
2. Every derived metric has an explanation and evidence references.
3. Unknown is a valid result.
4. A lower number is not automatically better: more context or more agents can be justified.
5. Human review decisions are first-class evidence.
6. Project-level baselines matter more than global arbitrary thresholds.
7. Metrics should compare similar Work Items where possible.
8. Raw prompts/responses do not need to be persisted to measure workflow efficiency.

## Event substrate

Engineering Intelligence should derive from an append-only engineering event ledger.

Minimum event shape:

```rust
struct EngineeringEvent {
    id: EventId,
    project_id: ProjectId,
    work_item_id: Option<WorkItemId>,
    execution_id: Option<ExecutionId>,
    event_type: EngineeringEventType,
    actor: ActorRef,
    occurred_at: DateTime<Utc>,
    evidence: Vec<EvidenceRef>,
    attributes: JsonValue,
}
```

Initial events:

```text
work_item_created
scope_changed
context_built
context_edited
execution_started
execution_finished
worker_handoff
changeset_created
changeset_reviewed
verification_started
verification_finished
commit_created
knowledge_injected
knowledge_proposed
knowledge_reviewed
human_override
```

## Level 0 — factual counters

These can be implemented without inference.

Per Work Item:

```text
elapsed_time
execution_count
worker_count
coding_agent_count
completion_model_count
handoff_count
context_build_count
context_token_total
unique_context_files
changed_file_count
changed_line_count
verification_run_count
verification_failure_count
changeset_count
accepted_changeset_count
rejected_changeset_count
scope_violation_count
knowledge_injected_count
knowledge_created_count
human_override_count
token_total
cost_total
```

These facts should be available before any composite metric is introduced.

## Context Compactness

### Goal

Determine whether workers received enough relevant information without repeatedly ingesting noise.

### Evidence

For each context pack record:

```text
included_files
included_symbols
knowledge_entries
context_tokens
worker
execution
```

After execution, collect:

```text
changed_files
files_named_in_worker_result
files_referenced_by_diagnostics
files_referenced_by_verification
files_human_marked_relevant
```

### Conservative useful-context set

A file may be considered evidence-backed useful when at least one of these is true:

- it was changed;
- it was explicitly referenced by the worker result;
- verification referenced it;
- the human marked it relevant;
- it contains a symbol directly referenced from a changed symbol, once symbol intelligence exists.

Absence from this set does **not** prove that a file was useless.

### Metrics

```text
Context Coverage
= useful_included_files / useful_files_observed_after_execution
```

```text
Context Precision Proxy
= evidence_backed_useful_included_files / included_files
```

```text
Context-to-Change Ratio
= context_tokens / max(changed_lines, 1)
```

```text
Repeated Context Ratio
= tokens_repeated_from_previous_worker_packs / total_context_tokens
```

The UI must display the components, not only the ratio.

## Agent Fan-out

### Goal

Describe decomposition cost.

Metrics:

```text
worker_count
agent_count
parallel_wave_count
handoff_count
max_dependency_depth
```

Suggested display:

```text
Workers: 4
Coding agents: 2
Check runners: 1
Human review: 1
Parallel waves: 2
Handoffs: 3
```

Do not flag fan-out only by count. Compare it to task complexity and outcome.

## Agent Redundancy

### Signals

- large overlap between context packs;
- overlapping file ownership without deliberate review role;
- repeated task instructions;
- repeated execution after no meaningful input change;
- overlapping changesets;
- duplicate independent reviews with identical evidence and purpose.

### Context overlap

For worker context file sets `A` and `B`:

```text
Jaccard(A, B) = |A ∩ B| / |A ∪ B|
```

This is a useful signal but not a verdict.

Example recommendation rule:

```text
IF
  same work item
  AND same worker role
  AND context-file Jaccard > 0.85
  AND neither worker produced distinct evidence/change ownership
THEN
  mark possible redundant execution
```

### Retry duplication

Two retries are near-duplicates when:

- goal unchanged;
- context hash unchanged or nearly unchanged;
- project tree/change state unchanged;
- worker role unchanged;
- previous failure evidence was not added.

RepoDesk can warn:

> Retry input is materially identical to the previous failed attempt. Add evidence, change scope, or reroute before spending another execution.

## Retry and Correction Cost

Track the full cost to reach an accepted result.

```text
first_attempt_success
attempts_to_accept
rejected_attempts
verification_failures_before_success
manual_fix_after_agent
reroutes
context_rebuilds
```

Useful derived metric:

```text
Correction Multiplier
= total_execution_cost / max(cost_of_accepted_execution_path, epsilon)
```

When monetary cost is unavailable, calculate token/time variants separately.

## Knowledge Reuse

### Facts

```text
knowledge_injected
knowledge_referenced
knowledge_from_previous_work_items
knowledge_created
knowledge_accepted
knowledge_rejected
```

### Metrics

```text
Knowledge Reuse Rate
= previously_existing_entries_used / injected_entries
```

```text
Knowledge Acceptance Rate
= accepted_proposals / reviewed_proposals
```

### Rediscovery candidate

A rediscovery event can be proposed when a new knowledge candidate is semantically equivalent to an already approved project entry that was not included in the worker context.

This feature should require high-confidence matching and remain advisory.

## Scope Adherence

Track whether changes remained inside Work Item scope.

Signals:

- changed file explicitly in scope;
- changed file under scoped directory;
- file required transitively and approved by human;
- unrelated out-of-scope file.

Metrics:

```text
Scope Adherence
= approved_in_scope_changed_files / changed_files
```

Record approved scope expansions separately so legitimate changes do not appear as violations.

## Verification Efficiency

Facts:

```text
checks_run
checks_passed
checks_failed
verification_duration
first_reviewed_changeset_passed
```

Metrics:

```text
First Verification Pass = true/false
```

```text
Verification Retry Count = verification_runs - 1
```

```text
Failure Concentration = failures_in_same_check / total_failures
```

Repeated failure concentration can suggest missing project knowledge or an inadequate pre-check.

## Evidence Density

The system should distinguish raw captured output from decision-relevant evidence.

Examples of high-value evidence:

- failing test names and assertions;
- compiler diagnostic with file/line;
- bounded diff;
- scope violation record;
- verification receipt;
- human rejection reason.

Potential metric:

```text
Evidence Density
= structured_decision_evidence_bytes / captured_output_bytes
```

This is primarily useful for trend comparison. Do not claim it measures correctness.

## Human Override Signal

Capture explicit human corrections:

```text
route_override
context_include
context_exclude
plan_edit
scope_expand
scope_reduce
changeset_accept
changeset_reject
manual_fix
knowledge_accept
knowledge_reject
```

A repeated override pattern should create an **Engineering Recommendation Candidate**, not silently change policy.

Example:

> In 6 Rust refactor tasks you manually added the nearest integration test after context preparation. Consider adding related integration tests to the default Rust refactor context policy.

## Algorithmic Profile

## Objective

Give the developer a practical, explainable approximation of time/space complexity and suspicious algorithmic structures.

It is not a formal verifier.

### Symbol-level input

```text
function/method symbol
AST/control-flow structure
resolved calls when available
collection operations
loop nesting
recursion hints
allocation hints
```

### First deterministic signals

#### Constant-time indicators

- direct indexing;
- hash lookup (document average-case assumption);
- fixed-size operations.

#### Linear indicators

- one full loop/iterator traversal over an input;
- one scan followed by constant-time operations.

#### N log N indicators

- sort followed by a linear pass;
- known standard sort calls when input size is the dominant variable.

#### Quadratic indicators

- nested loops over the same or similarly sized collections;
- repeated `contains/find/position` linear search inside an outer full traversal;
- pairwise comparison loops.

#### Space indicators

- collection allocated proportional to input;
- recursion depth proportional to input;
- cloning complete input collections;
- fixed auxiliary state.

### Output contract

```rust
struct AlgorithmicProfile {
    symbol: SymbolRef,
    time: ComplexityHint,
    space: ComplexityHint,
    confidence: Confidence,
    evidence: Vec<AlgorithmicEvidence>,
    assumptions: Vec<String>,
    warnings: Vec<String>,
}
```

```rust
enum ComplexityHint {
    Constant,
    LogN,
    Linear,
    NLogN,
    Quadratic,
    Polynomial { degree: u8 },
    Exponential,
    Unknown,
}
```

### Diff-aware comparison

The most useful product behavior is comparison around a changeset:

```text
Before: O(n)
After:  O(n²)
Confidence: medium
Reason: linear `contains()` search was introduced inside an existing full traversal.
```

Or:

```text
Before: unknown
After:  unknown
New signal: nested loop depth increased 1 -> 2
```

RepoDesk should not block a change solely from a heuristic Big-O estimate by default. It may warn and request review.

## Engineering Efficiency dimensions

A Work Item summary can eventually show:

```text
Task complexity        medium
Scope adherence        100%
First verification     pass
Attempts               2
Workers                3
Agent redundancy       low-confidence: 12%
Context coverage       94%
Context precision      71%
Knowledge reused       4 entries
Human overrides        1
Algorithmic change     no detected regression
Tokens                 18,420
Cost                   $0.18
Elapsed                 7m 14s
```

No overall `87/100 engineering score` in v0.

## Storage

Recommended separation:

```text
engineering_events       raw factual events
engineering_evidence     references to receipts/artifacts
engineering_metrics      cached derived snapshots
algorithmic_profiles     per symbol/tree state
```

Derived metrics should be rebuildable from events/evidence where practical.

## Baselines

RepoDesk should learn project baselines over time.

Examples:

- median context size for Rust bugfix tasks;
- typical number of changed files;
- first-pass verification rate;
- typical agent count;
- median retries;
- common check failures.

Recommendations should prefer project-relative anomalies:

> This task used 4.2× more context than the median of comparable Rust bugfix tasks.

rather than arbitrary universal thresholds.

## Privacy

Engineering Intelligence should work with metadata and structured evidence where possible.

Do not require storing full raw prompts/responses.

Useful persisted metadata includes:

- hashes;
- file/symbol references;
- token counts;
- worker IDs/types;
- timing;
- result status;
- changed paths;
- check names/results;
- human decisions;
- knowledge IDs.

## Initial implementation order

1. Event ledger.
2. Level-0 factual counters.
3. Work Item intelligence report.
4. Context compactness facts.
5. Agent fan-out/retry facts.
6. Knowledge reuse counters.
7. UI summary in Runs.
8. Algorithmic Profile for Rust.
9. Diff-aware complexity comparison.
10. Project-relative recommendations.

This order avoids building scores before the system has trustworthy evidence.