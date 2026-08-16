# Design-System Convergence Design

Date: 2026-08-16
Status: approved design, implementation pending
Roadmap: Cut F — Design-system convergence

## Goal

Converge RepoDesk onto one durable UI language without turning the work into a product-wide redesign. Preserve the product's engineering-workbench identity while improving hierarchy, density, consistency, accessibility, and evidence readability across the five owning surfaces:

- Work
- Code
- Changes
- Runs
- Projects

The implementation must reduce historical visual debt rather than introduce another visual generation.

## Product direction

Use a **controlled refresh**, not a cosmetic-only cleanup and not a full redesign.

The target is a compact engineering workbench:

- dense enough for developer workflows;
- clear primary/secondary information hierarchy;
- evidence and blockers visible without decorative dashboard chrome;
- consistent spacing, actions, status, empty/loading/error states;
- editor surfaces remain editor-like rather than being forced into card layouts.

Do not create `design-v2`, `work-v4`, `new-ui`, or equivalent replacement layers.

## Current problems

The current frontend has a useful shared UI base but still contains multiple historical visual dialects:

- route-specific panel/header/action structures;
- versioned stylesheets such as `ai-strategy-v1.css`, `command-palette-v2.css`, `context-evidence-v2.css`, and `execute-packet-v2.css`;
- local visual generations and route-wide polish styles;
- static inline layout styles;
- semantic state inferred from text via helpers such as substring-based `statusTone()`;
- broad shared files that risk becoming god-files;
- repeated local loading/error/empty/action patterns;
- status/evidence presentation that is not consistently typed by domain state.

This debt is concentrated enough to migrate incrementally. A frontend rewrite is explicitly out of scope.

## Architecture

Target dependency direction:

```text
foundation / tokens
  -> semantic primitives
  -> shell / workbench patterns
  -> domain adapters and domain components
  -> Work / Code / Changes / Runs / Projects
```

Dependencies must flow downward only. A primitive must not know about RepoDesk domain enums such as attribution, verification, or scope states.

### 1. Foundation and tokens

`foundation.css` remains the canonical token/foundation owner. Consolidate stable values for:

- semantic colors;
- surfaces and borders;
- typography;
- spacing rhythm;
- radii;
- focus treatment;
- density primitives;
- motion only where it conveys state.

Raw visual values in route TSX are not part of the target architecture.

### 2. Semantic primitives

Create focused primitives under a narrow shared boundary, for example:

```text
apps/desktop/src/shared/ui/primitives/
```

Do not add them all to `SharedComponents.tsx`.

Canonical primitives:

- `StatusBadge`
- `EvidenceState`
- `PanelHeader`
- `EmptyState`
- `LoadingState`
- `ErrorState`
- `InspectorSection`
- `ActionBar`
- `Metric`

Each primitive owns one semantic responsibility and exposes a typed API.

### 3. Shell / workbench patterns

Shared layout patterns may compose primitives for:

- route headers;
- section stacks;
- compact evidence grids;
- inspector regions;
- primary content plus technical detail;
- action placement.

These patterns may own geometry, but must not own domain policy.

### 4. Domain adapters

Domain state must be mapped explicitly into semantic UI state.

Example:

```text
backend/domain enum
  -> exhaustive feature adapter
  -> SemanticState / SemanticTone
  -> StatusBadge or EvidenceState
```

Examples of adapters:

- `mapAttributionState(...)`
- `mapVerificationState(...)`
- `mapScopeState(...)`
- `mapReviewState(...)`
- `mapExecutionEvidenceState(...)`

Adapters use exhaustive switches. New backend/domain variants must force a compile-time UI decision whenever possible.

There must be no silent fallback that converts an unknown domain state to `neutral`.

## Semantic language

Define one shared semantic tone vocabulary independent of raw colors:

```ts
export type SemanticTone =
  | "positive"
  | "attention"
  | "critical"
  | "neutral"
  | "info";
```

Exact names may be adjusted during implementation if existing conventions make a nearby naming set materially cleaner, but the meaning must remain explicit and typed.

Illustrative mappings:

```text
exact_isolated       -> positive
legacy_unknown       -> attention
unattributed         -> critical
verification_stale   -> attention
verification_passed  -> positive
scope_violation      -> critical
manual_handoff       -> neutral/info depending on context
```

Semantic components choose presentation from `SemanticTone`; feature code chooses the tone from domain truth.

## Primitive contracts

### StatusBadge

Answers only: **what is the current state?**

Examples:

- Passed
- Stale
- Blocked
- Exact
- Manual
- Unavailable

Contract:

- short label;
- typed semantic tone;
- optional accessible label when visible copy is intentionally terse;
- no explanatory paragraph;
- no domain enum parsing inside the primitive.

### EvidenceState

Answers: **what evidence exists, and how trustworthy/current is it?**

May contain:

- evidence label;
- state label;
- semantic tone;
- concise detail/provenance;
- optional technical-detail affordance.

It is not a generic card abstraction.

### PanelHeader

Standardizes:

- optional eyebrow/section identity;
- title;
- optional description;
- optional trailing secondary action.

It replaces local `panel-title-row`, `header-row`, and equivalent variants as migrated routes lose those consumers.

### EmptyState / LoadingState / ErrorState

Support two explicit scopes:

- `inline`: a section, list, or subpanel;
- `surface`: most/all of the current route.

A local failure must not visually imply that the whole route failed.

Errors must preserve useful user-facing remediation detail and accessibility semantics.

### InspectorSection

Owns secondary technical evidence such as:

- SHA / IDs;
- worktree identity;
- raw evidence metadata;
- timestamps;
- bounded diagnostic details.

Critical blockers and required decisions must never be hidden only inside the inspector.

### ActionBar

Contract:

```text
0..1 primary action
0..N secondary actions
0..1 destructive action
```

This extends the existing one-primary-action workflow invariant rather than competing with it.

A route or phase may have zero primary actions when the current state is informational or blocked.

### Metric

Use only when the number changes a decision or communicates material engineering state.

Good examples:

- `3 / 3 checks passed`
- `2 files out of scope`
- token/cost usage when it affects routing/budget decisions

Do not rebuild generic KPI-dashboard cards.

## Controlled visual refresh

The migration may intentionally change appearance where it improves information hierarchy.

Desired changes:

- reduce decorative panel-within-panel nesting;
- establish a consistent vertical rhythm;
- separate state from evidence from action;
- make blocking/critical states visually immediate;
- demote technical metadata without hiding it;
- normalize heading hierarchy;
- normalize button/action placement;
- normalize empty/loading/error geometry;
- reduce visual noise and oversized whitespace;
- keep developer-tool density rather than SaaS-dashboard spacing.

The visual identity of RepoDesk must remain recognizable as an engineering workbench.

## Migration strategy

Use a strangler migration rather than a big-bang rewrite.

Order:

```text
1. Changes
2. Work
3. Runs
4. Projects
5. Code
```

### Changes — reference implementation

Changes is the first migrated surface because it already owns the richest typed trust model:

- producer attribution;
- scope state;
- acceptance evidence;
- review state;
- verification state/freshness;
- Safe Commit Manifest readiness.

Changes establishes the reference semantic language and component composition for evidence-heavy product UI.

Required reference states include:

- exact attribution;
- weak/unknown attribution;
- stale verification;
- scope violation;
- commit-ready;
- loading/empty/error.

### Work

Migrate operational workflow hierarchy:

- phase header;
- preparation/execution/review/verify/finish state;
- contextual evidence;
- `ActionBar` ownership.

Preserve the existing invariant that a phase has at most one primary action.

### Runs

Make Runs the most technical primary surface:

- immutable execution evidence;
- executor/provider/model identity;
- context/worktree provenance;
- bounded logs and diagnostics;
- cost/usage;
- recovery state.

Prefer dense evidence presentation over decorative cards.

### Projects

Unify registry/configuration/Knowledge states and mutation feedback with the same shared language.

Project policy remains domain-owned; UI primitives only present its state.

### Code

Migrate Code last because Monaco/editor geometry is a specialized work surface.

Code inherits:

- shell/header language;
- actions;
- status/evidence states;
- empty/loading/error states;
- inspector patterns.

Do not force the editor into dashboard panel composition.

## CSS migration contract

The work must delete consumers before deleting historical CSS. Never rename old generations into new versioned generations.

Forbidden new patterns after the first reference migration:

- new `*-vN` classes or stylesheets;
- new `*-polish.css` route-wide generations;
- new `new-ui`, `design-v2`, or equivalent visual replacement layers;
- new raw hex values in TSX outside explicit narrowly-reviewed exceptions;
- new static inline layout styles;
- unreviewed feature CSS growth above recorded baselines.

Dynamic inline values that are inherently data-driven, such as a calculated progress width, may remain inline when converting them to classes would reduce clarity. Static geometry around them must live in CSS.

## Ratchet strategy

Do not fail CI on all historical debt immediately.

After the Changes reference migration:

1. record current debt baselines;
2. freeze growth;
3. make new violations fail architecture checks;
4. lower baselines as each route migrates;
5. remove the baseline when the debt category reaches zero.

Ratchets must be deterministic and cheap enough for normal CI.

Expected checks include:

- no new raw `#hex` in TSX outside an allowlist/token boundary;
- no new static inline layout style objects;
- no new visual `*-vN` naming;
- no new route-wide polish stylesheets;
- feature CSS bytes may not exceed reviewed baseline;
- optional architecture checks preventing reintroduction of substring-based domain status inference.

The ratchet must not be weakened merely to merge a migration PR.

## Removal of substring status inference

The existing `statusTone()`-style string heuristics are not a valid long-term domain-state contract.

Migration rules:

- do not add new consumers;
- typed product/domain states must use explicit adapters;
- generic text-only utility surfaces may temporarily keep a compatibility helper only where no typed source exists;
- remove the helper once all legitimate consumers are migrated;
- a new domain state must never be classified through `includes("error")`, `includes("ok")`, or equivalent text parsing.

## Accessibility

The convergence must improve, not regress, accessibility:

- semantic tone cannot be communicated by color alone;
- badges/evidence states expose readable text;
- loading/error/status regions use appropriate roles where meaningful;
- keyboard focus remains visible and tokenized;
- ActionBar ordering follows DOM/tab order;
- inspector detail does not trap keyboard navigation;
- destructive actions remain clearly distinguishable by text and semantics, not only color.

## Performance and bundle constraints

This is a convergence cut, not a frontend framework expansion.

Requirements:

- no new UI framework solely for this migration;
- no utility-CSS framework migration;
- no runtime CSS-in-JS dependency;
- primitives must remain lightweight;
- avoid new global rerender/state ownership;
- do not add extra backend calls for presentation-only state;
- derive semantic presentation from already-fetched domain data;
- preserve existing lazy route/error-boundary behavior.

## Error handling

Shared states must preserve failure ownership.

Rules:

- inline failure -> inline ErrorState;
- route authority failure -> surface ErrorState;
- mutation failures remain visible until superseded or retried;
- a failed secondary inspector fetch must not replace usable primary workflow content;
- status adapters must not hide unknown/unhandled state behind a safe-looking neutral label.

## Testing contract

### Component / TypeScript

Cover:

- primitive rendering contracts;
- semantic tone mapping;
- exhaustive domain adapters;
- ActionBar action-count invariants where practical;
- accessibility semantics for important states.

### Playwright behavior

Each owning route needs representative fixtures for:

- normal;
- loading;
- empty;
- error;
- important warning/blocked state.

Changes additionally covers:

- exact attribution;
- weak/unknown attribution;
- stale verification;
- scope violation;
- commit-ready.

Tests should assert semantics and behavior, not individual pixel values or arbitrary margin constants.

### Visual regression

Maintain visual regression coverage for:

- Work;
- Code;
- Changes;
- Runs;
- Projects.

Reference screenshots should focus on stable route geometry and semantic hierarchy. Avoid snapshotting highly volatile timestamps/log text when it creates noise rather than protection.

### Architecture ratchet

CI must prove that migrated code cannot silently reintroduce the banned visual generations/debt categories.

## Delivery slicing

Cut F should land as several coherent PRs rather than one frontend-wide rewrite.

Recommended sequence:

1. semantic primitives + typed semantic vocabulary + Changes reference migration + initial ratchet;
2. Work migration + lower Work/CSS debt baseline;
3. Runs migration + technical evidence density cleanup;
4. Projects migration;
5. Code shell/state migration;
6. final historical CSS/dead-class cleanup and ratchet baseline removal where possible.

A later slice may be combined with an adjacent one only if the diff remains reviewable and the architecture ratchet proves no debt growth.

## Non-goals

This cut does **not** include:

- a new dashboard;
- another top-level route;
- new AI chat/model surfaces;
- a full brand redesign;
- replacement of Monaco/editor UX;
- migration to Tailwind, Material UI, Chakra, styled-components, or another new UI system;
- broad animation work;
- redesigning backend/domain policy to fit UI components;
- changing trust semantics established by prior Cuts C/G.

## Acceptance criteria

Cut F is complete when:

1. Work, Code, Changes, Runs, and Projects share one semantic status/evidence language.
2. Typed domain state reaches UI through explicit adapters rather than string heuristics.
3. `StatusBadge`, `EvidenceState`, `PanelHeader`, `EmptyState`, `LoadingState`, `ErrorState`, `InspectorSection`, `ActionBar`, and decision-relevant `Metric` have canonical shared implementations.
4. Changes serves as the reference evidence-heavy route and does not invent local status/card/action systems.
5. No new versioned visual generation exists.
6. New raw TSX hex, static inline layout styles, versioned classes, route-wide polish files, and accidental CSS growth are ratcheted.
7. Historical CSS/classes are removed as their consumers disappear; no replacement `vNext` layer is created.
8. Loading/error/empty/action behavior is not rebuilt ad hoc per primary feature.
9. Each primary route has behavior and visual regression coverage for representative states.
10. Code preserves editor-specific geometry while adopting the shared shell/state language.
11. The resulting UI is denser and clearer, not more dashboard-like.
12. A new evidence state can be added through a domain adapter and shared primitives without creating a new local badge/card/error/layout dialect.

## Success metric for the architecture

A developer implementing a new typed engineering state should normally need to:

1. add or receive the domain enum/state;
2. map it exhaustively in the owning feature adapter;
3. render it with an existing semantic primitive;
4. add behavior/visual coverage;

They should **not** need to invent a color map, badge implementation, status parser, panel header, empty/error state, action layout, or route-wide stylesheet generation.