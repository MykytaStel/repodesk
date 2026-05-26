# Product Workflow MVP Pack

Recommended branch: `feature/product-workflow-mvp`

This pack turns RepoDesk Desktop from a set of debug panels into a product workflow.

## Product promise

Open RepoDesk, choose a project, create a task, and let the control brain guide the next safe step:

1. Select project
2. Create active task
3. Build context
4. Build smart context
5. Scan safety
6. Generate prompts
7. Run checks
8. Review artifacts and history

## Security rules

- The UI cannot run arbitrary shell commands.
- The UI can only invoke whitelisted Tauri commands.
- The primary button resolves to an allowed action from the Rust action catalog.
- Unsafe project/task input is rejected before reaching the CLI.
- Artifact reading is limited to the active task run directory.

## Why this matters

RepoDesk should not feel like a pile of tools. It should feel like a cockpit with one clear next step.

