# RepoDesk Desktop Experience MVP

This pack upgrades RepoDesk from a primitive dashboard into a product cockpit.

## Product intent

RepoDesk is a local control brain for AI-assisted development. It should not behave like a random chat wrapper. It should:

- understand the active project and task;
- build bounded context packs;
- scan context before sending it to AI;
- judge whether an agent should be allowed, warned, or blocked;
- route work to local or paid AI systems;
- run verification checks;
- store receipts of actions.

## Current desktop rules

The desktop UI is intentionally action-whitelisted.

Allowed:

- read local RepoDesk state;
- run known bounded CLI actions;
- show action output;
- store local action receipts.

Not allowed yet:

- unrestricted shell from UI;
- arbitrary command execution;
- secret file reads;
- agent writes without guard/judge.

## Next product increments

1. Project/task management from UI.
2. Provider settings screen.
3. SQLite-backed shared persistence for CLI and desktop.
4. Human approval queue for guarded actions.
5. Real Ollama runtime integration.
6. Better context visualizer.
7. Agent run screen with prompt preview, safety scan, and receipts.
