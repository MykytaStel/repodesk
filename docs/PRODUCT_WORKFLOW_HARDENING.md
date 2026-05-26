# Product Workflow Hardening

This increment makes the desktop UI behave more like a product and less like a debug panel.

## What changed

- Replaced the desktop shell UI with a clearer cockpit layout.
- Added startup skeletons and a blocking loading overlay for long actions.
- Added persistent tabs via localStorage.
- Added a visible top status strip: task, Git state, next action.
- Added a Setup screen for project/task onboarding.
- Added a Workflow screen with a product self-test.
- Added clearer Git workspace visibility.
- Added clearer AI Discovery found/missing/endpoints sections.
- Added a Debug console where every Tauri invocation is visible.
- Preserved the security model: no unrestricted shell is exposed to UI.

## Testing checklist

1. Start desktop app with `./scripts/dev-desktop.sh`.
2. Confirm startup skeleton appears before data loads.
3. Confirm the Debug tab records `desktop_snapshot`, `product_workflow_state`, `git_workspace_snapshot`, `ai_discovery_scan`, and `desktop_actions`.
4. Confirm Git tab shows branch, dirty state and changed files.
5. Confirm AI Discovery shows found/missing tools or a clear empty state.
6. Run Product Self-Test from Workflow.
7. Run a bounded action and confirm toast + Debug event + refreshed state.

## Next product step

The next increment should add a proper before/after action impact view:

- capture Git snapshot before an action
- capture Git snapshot after an action
- show changed files caused by that action
- show ready-to-commit checklist
- show suggested commit message
