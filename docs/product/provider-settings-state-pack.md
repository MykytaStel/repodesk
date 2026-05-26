# Provider Settings + State Pack

Recommended branch: `feature/provider-settings-state`

This pack makes RepoDesk more usable as a daily desktop product by adding persistent provider settings and a small SQLite state store.

## What it adds

- Provider settings for Ollama, ChatGPT, Codex, Gemini, and manual mode.
- Safe local-only Ollama URL validation.
- Preferred provider routing for patch, compression, and review workflows.
- SQLite-backed settings table in `~/.repodesk/repodesk.db`.
- Desktop commands for reading/saving provider settings.
- Runtime tab UI for editing settings.
- DB status in the UI.
- Rust tests for provider validation.

## Security rules

- Provider settings do not store API keys.
- Ollama URL is restricted to `http://127.0.0.1...` or `http://localhost...`.
- Paid agents can be disabled globally.
- The desktop UI still cannot run arbitrary shell commands.

