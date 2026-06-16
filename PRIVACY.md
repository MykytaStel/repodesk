# Privacy & Data Handling

RepoDesk is **local-first**. It is designed so that your code and project data stay on
your machine unless *you* explicitly route a task to a cloud AI provider. This document
states plainly what is stored and what — if anything — leaves your computer.

## What stays on your machine
- **All project state** lives under `~/.repodesk` (overridable via `REPODESK_HOME`):
  the project registry, task runs, generated context packs, prompts, memory, logs, and
  the local SQLite database.
- **API keys are read from environment variables only** (`ANTHROPIC_API_KEY`,
  `OPENAI_API_KEY`, `GEMINI_API_KEY`, …). RepoDesk does **not** write your keys to disk
  or its database.
- **No telemetry, analytics, crash reporting, or "phone home."** RepoDesk does not
  collect usage data and contains no third-party tracking.

## What can leave your machine — and only when you choose it
- **Cloud AI calls.** If you enable and select a *paid/cloud* provider for a task,
  RepoDesk sends that provider's API the **bounded context pack** plus your prompt.
  Local providers (e.g. Ollama) keep everything on your machine. Cloud providers are
  **off by default** and require explicit enablement.
- **What the context pack contains.** Only RepoDesk-managed files (task notes, memory,
  decisions, risks) and **git metadata** (branch, status, diff *stats* and changed file
  *names*). It does **not** include raw repository file contents. A secret/safety scan
  runs before any content is sent, and blocks on detected secrets.
- **Update checks.** The auto-updater contacts GitHub Releases
  (`github.com` / `objects.githubusercontent.com`) to check for and download signed
  updates. This happens only when an update check is triggered — **not automatically on
  launch**.

## Your responsibilities
- Treat your API keys as secrets; RepoDesk only reads them from the environment.
- Review what you send when using a cloud provider — once data reaches a third-party
  AI API, it is governed by *that provider's* privacy terms, not this document.

## Changes
Material changes to data handling will be noted in [`CHANGELOG.md`](CHANGELOG.md).

_This document describes the software's behavior; it is not legal advice. If you
distribute RepoDesk commercially, have counsel review your privacy obligations._
