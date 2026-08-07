# RD2-11 — Focus Workspace + Engineering Knowledge

RepoDesk has accumulated useful engineering surfaces faster than its information hierarchy evolved. The problem is not feature count by itself. The problem is that too many features are visually present at the same time and therefore compete as if they were equally important.

This slice establishes two connected product rules:

1. RepoDesk uses a **focus-first IDE shell**.
2. RepoDesk learns through **reviewed Project Engineering Knowledge**, not opaque generic memory.

---

## 1. Focus-first workspace

### Problem

The previous shell could show all of these simultaneously:

```text
activity rail
+ contextual sidebar
+ titlebar
+ provider/token/status chips
+ persistent feedback strip
+ Work Item contract
+ phase workflow
+ right inspector
+ bottom panel
```

That produces many independent visual anchors. Even when every individual component is reasonable, the total screen becomes difficult to scan.

The redesign treats **attention as a budget**.

### Permanent surfaces

Only two things are permanently visible:

```text
Activity Rail | Active Engineering Canvas
```

The activity rail answers:

> Which primary engineering surface am I in?

The canvas answers:

> What am I working on right now?

### Secondary surfaces become deliberate drawers

The left workspace context and right inspector no longer consume permanent layout width.

```text
Cmd/Ctrl+B -> contextual workspace drawer
Inspector button -> evidence/context drawer
Cmd/Ctrl+J -> bottom execution panel
```

Opening a drawer should not resize the main engineering canvas.

### Surface responsibilities

#### Main canvas

Owns the active task/action.

Examples:

- Work: current lifecycle phase and next action;
- Code: source/file focus;
- Changes: current delta/review;
- Runs: selected execution evidence;
- Projects: repository workspace management;
- Knowledge: selected reusable engineering record.

#### Context drawer

Owns nearby navigation and workspace switching.

It must not become another complete menu tree.

#### Inspector drawer

Owns read-only evidence that helps explain the active canvas:

- context coverage;
- review state;
- verification state;
- commit gate;
- historical execution facts.

#### Bottom panel

Owns process-oriented output:

- Problems;
- Output;
- Terminal.

It remains closed unless the user asks for it or an action explicitly needs it.

### Navigation budget

Primary navigation remains exactly:

```text
Work
Code
Changes
Runs
Projects
```

Engineering Knowledge does **not** add a sixth activity-rail destination. During migration it replaces the product meaning of the old `memory` route and is reached through Work, contextual navigation, Inspector, or command palette.

Advanced/legacy surfaces such as Dashboard and Debug do not compete for normal contextual navigation.

### Visual hierarchy rules

RepoDesk should prefer:

```text
flat surface
+ separator
+ selected row
+ focused detail
```

over:

```text
card
inside card
inside panel
inside dashboard grid
```

Elevation is reserved for temporary overlays/drawers/modals.

Color is semantic:

- accent -> current selection/action;
- green -> verified/successful state;
- yellow -> caution/pending;
- red -> failure/blocker;
- neutral -> everything else.

Status telemetry should live close to the decision it affects. Provider/model/token telemetry belongs in Models & Cost or run evidence, not permanently in the global titlebar.

---

## 2. Engineering Knowledge v0

### Product boundary

Engineering Knowledge belongs to RepoDesk because it describes how a concrete repository should be engineered.

Examples:

```text
architecture boundaries
repository conventions
known hazards
verified commands
testing rules
decisions
performance constraints
tooling rules
```

It is not generic model/provider memory. Provider learning belongs to SubRadar.

### Storage

Project-local versioned artifact:

```text
engineering-knowledge.json
```

A record contains:

```text
id
project
category
title
content
status
origin
source Work Item
evidence refs
created/updated timestamps
```

### Lifecycle

```text
Evidence / Human observation
            |
            v
        Candidate
            |
      human review
       /          \
      v            v
  Accepted       Archived
      |
      v
eligible for future bounded context
```

Only `Accepted` knowledge can enter an agent context.

`Candidate` and `Archived` records are excluded by construction.

### Evidence-backed suggestions

A fresh canonical VerificationReceipt can produce suggestions from successful commands.

Example:

```text
cargo test -p repodesk-core
```

RepoDesk may suggest capturing this as project Testing knowledge.

It does **not** become accepted automatically.

The flow remains:

```text
successful verification command
    -> suggestion
    -> Capture candidate
    -> human Accept
    -> reusable Project Knowledge
```

This preserves the same epistemic rule introduced by Acceptance Evidence: machine evidence can justify a candidate, but a reusable project rule still requires explicit review.

### Context injection

The Context Builder now separates:

```text
Project Engineering Knowledge
Legacy Project Memory
```

Engineering Knowledge receives its own token budget and higher product priority. Legacy Memory Brain remains temporarily for compatibility with existing behavior, but with a smaller budget.

Context selection is deterministic:

1. only accepted records;
2. category priority;
3. lexical relevance to Work Item title + typed goal;
4. recency as a stable tie-breaker;
5. hard token budget.

There is no opaque AI relevance score in v0.

### Context telemetry

Engineering Knowledge is a separate context component:

```text
engineering_knowledge
```

Therefore Context Compactness can later answer:

- how much project knowledge was considered;
- how much was included;
- whether it was trimmed/reused;
- how often accepted project knowledge actually participates in engineering work.

Raw knowledge text is not copied into the engineering event ledger.

---

## 3. Performance contract

The focus redesign also removes unnecessary global work.

The application shell no longer polls model-health and token-usage data merely to keep global titlebar chips updated. Those queries belong to the surfaces that actually need them.

Knowledge context is bounded by token budget, and only accepted records are considered for injection.

UI state keeps:

- one selected Knowledge record;
- one filtered list;
- lightweight snapshot metadata.

No editor/run output is duplicated into React state as part of this slice.

---

## 4. Migration compatibility

The old `memory` route id remains temporarily because removing it would create unnecessary routing churn during the RepoDesk 2 migration.

Its visible product identity changes from generic **Memory** to **Knowledge** and renders the new Engineering Knowledge workspace.

The existing legacy Memory Brain implementation remains available internally until later migration work can prove which remaining consumers still need it.

This is intentional strangler-pattern migration, not two competing long-term memory systems.

---

## 5. Non-goals

This slice does not add:

- semantic embeddings/vector search;
- AI-generated acceptance of knowledge;
- cross-project knowledge sharing;
- provider/model learning;
- autonomous mutation of accepted knowledge;
- a sixth primary navigation item;
- a complete visual rewrite of every feature;
- LSP/editor changes.

---

## 6. Next design rule

Future RepoDesk features should answer this before adding UI:

> Does this need a new permanent surface, or is it context for an existing surface?

Default answer should be the latter.

A feature earns permanent navigation only when it represents a distinct, recurring engineering mode — not merely because it has enough data to fill a page.
