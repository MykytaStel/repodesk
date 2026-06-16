# CLAUDE.md

Project context for Claude Code. The full, AI-agnostic context lives in **@AGENTS.md** —
read it first (it covers layout, build/test/verify commands, conventions, gotchas, security,
and roadmap status).

## Claude-specific notes
- Subagents for this repo are in `.claude/agents/` (e.g. `verify-gates`, `test-author`).
- Permission allowlist is in `.claude/settings.json` (committed); personal overrides go in
  `.claude/settings.local.json` (git-ignored — never commit it).
- When adding tests, follow `crates/repodesk-core/tests/core_safety_paths.rs`: temp
  `REPODESK_HOME` fixture + `#[serial]`.
- Reserve full gates (`./scripts/verify-all.sh`) for handoff; run only the tests relevant to a
  change during iteration. Don't push or commit unless asked.
</content>
