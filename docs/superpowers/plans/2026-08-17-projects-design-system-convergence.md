# Projects Design-System Convergence Implementation Plan

**Goal:** Migrate Projects onto the shared semantic primitive language while preserving project registry/configuration, Knowledge, Work templates, activation, and exact-attribution policy authority.

**Architecture:** `ProjectsTab.tsx` remains the route owner. One pure `projectsSemantic.ts` adapter maps typed project activity, attribution-policy, and project-mutation notice states into shared semantic tones. `useProjectSetup.ts` keeps mutation/business ownership and exposes a typed notice contract; primitives only present it.

## Constraints

- Cut F order remains Changes → Work → Runs → Projects → Code.
- No backend/Rust behavior changes.
- Project policy remains domain-owned; UI only presents it.
- `require_exact_change_attribution=true` is an informational policy fact, not a success state.
- Active project is positive operational state; no active project is neutral.
- setup/activation pending feedback is attention, success positive, failure critical.
- Registry loading/error/empty states use shared accessible primitives.
- Keep Registry / Knowledge / Work templates ownership and lazy boundaries.
- Preserve query keys, cache invalidation, add/activate behavior, and exact-attribution mutation semantics.
- Use `ActionBar` so each local context has at most one primary action.
- No new feature CSS, visual `vN`, polish layer, raw TSX hex, static inline layout, or status-string inference.
- Preserve all Changes/Work/Runs architecture ratchets already in `main`.
- Exact-head Architecture, full CI, Playwright, coverage/security/supply-chain, and native E2E must be green before squash merge.

## Task 1 — RED semantic contracts

- Add `projects-design-system.spec.ts` covering active/no-active state, attribution policy, registry loading/error/empty, setup failure, and one-primary registry-card ownership.
- Extend generic architecture ratchet to require `projectsSemantic.ts` + shared primitive boundary.
- Record RED before production migration.

## Task 2 — Typed semantic adapter

- Export the existing project notice tone union from `useProjectSetup.ts`.
- Add exhaustive `projectsSemantic.ts` mappings for:
  - active / inactive workspace state;
  - exact-required / informational attribution policy;
  - setup/activation notice tone.
- No display-string parsing.

## Task 3 — Route and registry migration

- Migrate route header to `PanelHeader` + `StatusBadge`.
- Migrate Suspense, registry loading/error/empty states to shared primitives.
- Migrate setup and registry section headers to `PanelHeader`.
- Render attribution policy as explicit `EvidenceState`.
- Render mutation feedback as semantic evidence/error state.
- Use `ActionBar` for route, setup, and project-card actions; active project cards have no fake primary action.

## Task 4 — Verification

- Ensure no feature CSS growth is needed.
- Self-review action ownership, policy truth, accessible status roles, and lazy views.
- Require exact-head Architecture Ratchet, frontend build, fmt, Clippy, Rust tests, Playwright, coverage, cargo-deny, gitleaks, strict secret scan, and native Tauri/WebDriverIO.
- Update PR evidence, mark ready, squash merge with expected head SHA.
