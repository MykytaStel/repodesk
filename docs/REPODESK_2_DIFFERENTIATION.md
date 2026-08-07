# RepoDesk 2 — Product Differentiation

## The wedge

RepoDesk should not compete on the claim that an AI agent can edit code, run a terminal, or work in parallel. Those capabilities are becoming baseline IDE infrastructure.

RepoDesk should compete on a different promise:

> **Every meaningful software change has an explicit engineering contract and a reviewable chain of evidence from intent to commit.**

The product is an **evidence-first, agent-agnostic engineering environment**.

AI is a worker. RepoDesk owns the engineering process around that worker.

## The evidence chain

The durable product model is:

```text
Work Item Contract
      │
      ├── goal
      ├── allowed scope
      ├── protected areas
      └── acceptance criteria
      │
      ▼
Bounded Context
      │
      ├── included files + reasons
      ├── token footprint
      ├── engineering knowledge
      └── context provenance
      │
      ▼
Execution
      │
      ├── human / agent / script / CI
      ├── worker identity
      ├── isolation
      └── cost + token evidence
      │
      ▼
ChangeSet
      │
      ├── exact files
      ├── diff
      ├── scope compliance
      └── attribution
      │
      ▼
Verification
      │
      ├── compiler / tests / linter / security
      ├── command receipts
      ├── Problems
      └── acceptance evidence
      │
      ▼
Human Review
      │
      ├── accept / reject / override
      └── rationale
      │
      ▼
Commit + Engineering Knowledge
```

This chain is the product. Editor, terminal, agents, LSP, MCP, models, and plugins exist to make the chain fast and pleasant.

## What RepoDesk must be able to answer

For any change, RepoDesk should eventually answer without reconstructing a chat transcript:

1. What was the intended outcome?
2. What was the agent or human allowed to touch?
3. What context was actually supplied, and why?
4. Which worker produced each changeset?
5. Did the changes stay inside the contract?
6. Which checks ran against which tree?
7. Which acceptance criteria have evidence?
8. What did the human approve, reject, or override?
9. What was committed?
10. What reusable engineering knowledge was learned from the result?

## Product principles

### Work Items, not chats

Chat is an interaction surface, not the source of truth. A Work Item survives model/provider changes and multiple execution sessions.

### Agent-agnostic

Codex, Claude, local models, scripts, humans, CI, and future agents are workers behind the same engineering contract.

RepoDesk should not require one model vendor to retain project history or prove how a change was produced.

### Deterministic before AI

Use Git, ASTs, manifests, compiler output, test results, file hashes, and event replay for facts that do not require inference.

AI may explain evidence, rank possibilities, or propose work. It should not manufacture facts RepoDesk can derive deterministically.

### Bounded context by default

A larger prompt is not automatically a better prompt. RepoDesk should know what entered context, why it entered, how large it was, and whether later changes were represented in that context.

### Proof before completion

A green-looking agent response is not completion. Completion is a reviewed changeset with verification evidence bound to the current tree.

### Human overrides are explicit evidence

The user must always be able to override a gate, but the override should be recorded as a deliberate engineering decision rather than silently weakening policy.

### Local-first control

Repository inspection, Git state, context construction, terminal execution, verification, and project knowledge should stay local whenever practical.

Cloud AI is optional execution capacity, not the owner of the engineering state.

## UX rule

RepoDesk should be dense in capability but quiet in presentation.

The default workspace should show only:

- what am I working on;
- what is the next engineering action;
- is anything violating the contract;
- what changed;
- what evidence exists;
- what needs my decision.

Everything else uses progressive disclosure through Inspector, Problems, Runs, command palette, or dedicated deep surfaces.

Avoid duplicating the same metrics in multiple panels. Avoid keeping large logs or agent transcripts in React state when a bounded native/read-model representation is sufficient.

## Roadmap filter

A proposed core feature should strengthen at least one of these capabilities:

```text
Contract
Context
Execution attribution
Change governance
Verification
Human review
Engineering knowledge
Engineering intelligence
```

If it does not, it is probably platform infrastructure, an extension, or outside the RepoDesk product boundary.

## Near-term product sequence

1. **Work Item Contract + scope compliance**
   - typed goal, allowed paths, protected paths, acceptance criteria
   - deterministic changeset comparison

2. **Changes as evidence**
   - changeset identity, attribution, scope violations, review decisions

3. **Runs + Verification receipts**
   - one timeline tying worker execution to checks and current tree state

4. **Acceptance evidence**
   - map verification receipts to explicit acceptance criteria

5. **Engineering Knowledge**
   - promote reviewed outcomes into scoped project knowledge with provenance

6. **Code + Problems + LSP**
   - make the editor fast enough for normal work while preserving the evidence chain

7. **Agent coordination**
   - parallel workers, handoffs, budgets, isolation, and conflict detection behind Work Items

8. **SubRadar bridge / plugins / MCP**
   - external execution and extensibility without moving engineering ownership out of RepoDesk

## The long-term moat

The moat is not a particular model or chat UX.

It is the accumulated, structured engineering history that connects:

```text
intent -> context -> worker -> change -> check -> decision -> outcome -> knowledge
```

As that history grows, RepoDesk can make increasingly useful deterministic recommendations about context selection, related tests, risky files, likely checks, effective worker patterns, and project-specific hazards while remaining explainable and reviewable.
