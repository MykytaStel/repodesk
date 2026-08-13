# RepoDesk Trust Polish Design

## Goal

Make RepoDesk's first-run, confirmations, project loading, and initial bundle tell one trustworthy story: RepoDesk is a local-first engineering workspace for bounded, reviewable software change.

## Product contract

- The visible identity is an engineering workspace, not an AI provider cockpit.
- The primary navigation vocabulary is `Work`, `Code`, `Changes`, `Runs`, and `Projects`.
- Empty data and failed data are distinct states. A failed project-registry read must never render as an empty registry.
- Destructive or capability-bearing actions use RepoDesk dialogs, never browser-native `alert()` or `confirm()`.
- Modal surfaces expose dialog semantics, close on Escape where safe, contain keyboard focus, and restore focus to the opener.
- Heavy editor and terminal dependencies load only when their surface is requested.

## UI structure

`shared/ui/Dialog.tsx` owns modal accessibility and focus behavior. `shared/ui/useDecisionDialog.tsx` owns promise-based confirmation state and renders through that primitive. About, artifact viewing, Code decisions, and Orchestrate decisions share these boundaries while retaining feature-specific content.

Project registry reads continue to use the shared React Query key, but errors propagate into a visible retry state. Successful empty arrays remain the only way to show an empty registry.

The bottom panel stays mounted so evidence and PTY state survive hide/show. Xterm is imported only after the user first selects Terminal; after that first activation, the terminal remains mounted.

## Verification

- Playwright covers product copy, dialog semantics, Escape/focus restoration, registry failure/retry, and RepoDesk-owned Orchestrate confirmation.
- The production build runs an entry-budget check that rejects eager Terminal/Editor preload and caps initial JavaScript gzip at 110 kB.
- Existing E2E, TypeScript build, Rust workspace verification, and secret scan remain green.
