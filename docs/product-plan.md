# RepoDesk — Personal AI Operations Hub

## 1. What RepoDesk is

**RepoDesk** is a private, local desktop/CLI workspace for managing AI-assisted development across multiple projects.

It is not a RepoPilot feature, not a public package, and not something that has to be published to GitHub. It is your personal control room for projects, context, agents, checks, logs, prompts, token usage, risks, and decisions.

Core idea:

> RepoDesk is not another AI agent. RepoDesk is the control layer above agents, projects, context, checks, and decisions.

RepoDesk should work not only with RepoPilot, but also with future projects: Rust CLIs, backend services, React Native apps, Job Copilot, portfolio projects, learning projects, and experiments.

---

## 2. Problems RepoDesk solves

1. AI tools consume too many tokens.
2. Agents read too many irrelevant files.
3. Agents often inspect full logs instead of short failure summaries.
4. Tests are sometimes run by agents, although a local shell should run them.
5. There is no single cockpit for active project, active task, checks, risks, decisions, and token estimate.
6. Tasks often start without scope, constraints, or acceptance criteria.
7. There is no local project memory per project.
8. There is no structured decision log.
9. Agent roles are mixed: planning, patching, reviewing, compressing context, and running checks are not separated.
10. There is no repeatable workflow: task → context → prompt → patch → checks → summary → review.

---

## 3. Main principles

### 3.1. Token discipline

> Never pay a model to read noise. Never pay a model to run commands. Pay a model only for bounded reasoning, review, planning, or patching.

In practice:

- Do not send full repositories to paid agents.
- Do not send full test logs to paid agents.
- Do not ask AI to run checks repeatedly.
- Build small context packs.
- Use local tools for command execution.
- Use local AI, such as Ollama, for compression and summarization.

### 3.2. Human-controlled workflow

RepoDesk should not make big architecture decisions by itself. It should help the owner control the workflow.

The human remains the operator:

```txt
You → decide
RepoDesk → organize
Ollama → compress/summarize locally
ChatGPT → architect/review/mentor
Codex → bounded patch executor
Gemini/Antigravity → second opinion
Shell → run checks
```

### 3.3. Local-first by default

RepoDesk data should live locally:

- project configs;
- tasks;
- runs;
- logs;
- context packs;
- generated prompts;
- decisions;
- risks;
- token ledger;
- project memory.

### 3.4. Context packs instead of full repo context

RepoDesk should compile focused context packs for agents instead of giving them the entire repository.

A context pack should include:

- active task;
- scope;
- constraints;
- acceptance criteria;
- changed files;
- relevant snippets;
- nearest tests;
- last failure summary;
- project memory;
- open risks.

---

## 4. Target architecture

```txt
RepoDesk
│
├── Rust Core
│   ├── projects
│   ├── tasks
│   ├── context
│   ├── checks
│   ├── tokens
│   ├── prompts
│   ├── agents
│   ├── decisions
│   ├── risks
│   └── storage
│
├── CLI
│   └── repodesk commands
│
├── Desktop App
│   ├── Tauri 2
│   ├── React + TypeScript UI
│   └── Rust command bridge
│
├── Local AI Layer
│   ├── Ollama
│   ├── local summarizers
│   ├── local embeddings later
│   └── sandbox agents later
│
├── External Agent Layer
│   ├── ChatGPT
│   ├── Codex
│   ├── Gemini / Antigravity
│   ├── Aider / OpenCode optional
│   └── Cline / Roo optional
│
└── Optional MCP Layer Later
    ├── read-only project/context tools
    ├── controlled checks tools
    └── no unrestricted shell access
```

---

## 5. Technology choice

### Core

**Rust**

Reasoning:

- good for local tooling;
- fast and reliable;
- good filesystem/process handling;
- useful for learning Rust deeply;
- reusable from CLI and desktop;
- future MCP server can be added.

### CLI

**clap**

The CLI is important even if there is a desktop UI. It gives automation, repeatability, and easy debugging.

### Desktop

**Tauri 2 + React + TypeScript**

Reasoning:

- Rust handles local/system logic;
- React helps build a useful dashboard faster;
- you already know React/TypeScript well;
- Tauri is suitable for local desktop tools.

### Storage v0.1

**Local files**

Start with files because it is simpler and easier to inspect:

```txt
~/.repodesk/
├── config
├── projects
├── runs
├── logs
└── cache
```

### Storage v0.2+

**SQLite**

Add SQLite when filtering, history, dashboards, token reports, and search become important.

---

## 6. Repository structure

```txt
repodesk/
├── README.md
├── PLAN.md
├── Cargo.toml
├── crates/
│   ├── repodesk-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── paths.rs
│   │       ├── init.rs
│   │       ├── projects.rs
│   │       ├── tasks.rs
│   │       ├── context.rs
│   │       ├── checks.rs
│   │       ├── tokens.rs
│   │       ├── prompts.rs
│   │       ├── agents.rs
│   │       ├── decisions.rs
│   │       ├── risks.rs
│   │       └── errors.rs
│   │
│   ├── repodesk-cli/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   │
│   └── repodesk-mcp/
│       ├── Cargo.toml
│       └── src/main.rs
│
├── apps/
│   └── desktop/
│       ├── src-tauri/
│       └── web-ui/
│
├── templates/
│   ├── task.md
│   ├── codex_prompt.md
│   ├── chatgpt_review.md
│   ├── ollama_compress.md
│   └── failure_summary.md
│
└── docs/
    ├── architecture.md
    ├── agents.md
    ├── workflows.md
    └── roadmap.md
```

For the MVP, `repodesk-mcp` and `apps/desktop` can be postponed. Keep the architecture in mind, but start with core + CLI.

---

## 7. Local user data structure

```txt
~/.repodesk/
├── config/
│   ├── repodesk.toml
│   ├── agents.toml
│   └── budgets.toml
│
├── projects/
│   ├── repopilot/
│   │   ├── project.toml
│   │   ├── memory.md
│   │   ├── decisions.md
│   │   ├── risks.md
│   │   └── prompts/
│   │
│   └── job-copilot/
│       ├── project.toml
│       ├── memory.md
│       ├── decisions.md
│       └── risks.md
│
├── runs/
│   ├── repopilot/
│   │   └── 2026-05-26-001/
│   │       ├── task.md
│   │       ├── context.md
│   │       ├── compressed-context.md
│   │       ├── prompt.codex.md
│   │       ├── prompt.chatgpt.md
│   │       ├── checks.log
│   │       ├── checks-summary.md
│   │       ├── decision.md
│   │       └── token-estimate.json
│   │
│   └── job-copilot/
│
├── cache/
│   ├── context/
│   ├── summaries/
│   └── embeddings/
│
└── logs/
    ├── repodesk.log
    └── token-ledger.csv
```

---

## 8. Core modules

### 8.1. Project Registry

Responsible for registering and selecting projects.

Commands:

```bash
repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli
repodesk project list
repodesk project use repopilot
repodesk project info
```

Example `project.toml`:

```toml
name = "repopilot"
path = "/Users/mykyta/Documents/projects/repopilot"
type = "rust-cli"
main_language = "rust"

checks = [
  "cargo fmt --all -- --check",
  "cargo clippy --all-targets --all-features -- -D warnings",
  "cargo test --all"
]

context_ignore = [
  ".git",
  "target",
  "node_modules",
  "dist",
  "reports",
  "*.pdf",
  "*.html"
]
```

### 8.2. Task Manager

Every workflow starts with a task.

Commands:

```bash
repodesk task new "Improve architecture anti-pattern detection"
repodesk task show
repodesk task status
repodesk task close
```

Task template:

```md
# Task

Improve architecture anti-pattern detection.

## Goal

Reduce false positives and improve evidence quality.

## Scope

- src/audits/architecture/**
- related tests

## Do not change

- public CLI flags
- release process
- unrelated report rendering

## Acceptance criteria

- cargo fmt passes
- cargo clippy passes
- cargo test passes
- new tests added or existing tests updated
- findings include clear evidence
```

### 8.3. Context Compiler

The most important module.

It should collect:

- active task;
- git status;
- changed files;
- diff stat;
- relevant source snippets;
- nearest tests;
- project memory;
- last checks summary;
- known risks.

Commands:

```bash
repodesk context build
repodesk context estimate
repodesk context show
```

It should avoid:

- `target/`;
- `.git/`;
- `node_modules/`;
- generated reports;
- full logs;
- lock files unless needed;
- random docs that are not relevant.

### 8.4. Token Estimator

MVP formula:

```txt
estimated_tokens = characters / 3
```

Statuses:

```txt
0-8k       OK
8k-12k     Medium
12k-30k    Large
30k+       Too large, compress first
```

Commands:

```bash
repodesk tokens estimate context.md
repodesk tokens report --today
repodesk tokens report --week
```

### 8.5. Prompt Generator

Generates prompts for agents.

Commands:

```bash
repodesk prompt codex
repodesk prompt chatgpt
repodesk prompt ollama
repodesk prompt review
```

Codex prompt should be bounded:

```md
# Role

You are a patch executor.

# Task

Use the active task and context pack.

# Rules

- Modify only files in scope unless absolutely necessary.
- Do not rewrite unrelated modules.
- Do not run expensive commands repeatedly.
- Add or update tests near changed code.
- Preserve public CLI/API behavior unless explicitly allowed.

# Return

- changed files
- behavior summary
- test recommendation
- remaining risks
```

### 8.6. Check Runner

Runs checks locally without AI.

Commands:

```bash
repodesk checks run
repodesk checks summarize
repodesk checks last
```

For RepoPilot:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Full logs stay local as `checks.log`. Agents should receive only `checks-summary.md`.

### 8.7. Agent Registry

Defines agent roles and restrictions.

Example `agents.toml`:

```toml
[agents.ollama]
role = "local_summarizer"
cost = "local"
use_for = ["compress_context", "summarize_logs", "summarize_diff", "find_relevant_files"]
never_use_for = ["final_architecture_decision", "large_refactor"]

[agents.chatgpt]
role = "architect_reviewer"
cost = "paid"
use_for = ["architecture_plan", "code_review", "codex_prompt", "risk_analysis", "pr_text"]
never_use_for = ["reading_full_repo", "running_tests"]

[agents.codex]
role = "patch_executor"
cost = "paid"
use_for = ["bounded_code_changes", "small_refactor", "add_tests"]
never_use_for = ["broad_unscoped_refactor", "repeated_full_test_runs"]

[agents.google]
role = "second_opinion"
cost = "paid_or_plan"
use_for = ["independent_review", "missed_risks", "docs_review"]
never_use_for = ["primary_patch_execution"]

[agents.hermes]
role = "sandbox_agent"
cost = "local_or_free"
use_for = ["experiment", "local_reasoning", "sandbox_review"]
never_use_for = ["direct_write_access", "unrestricted_shell", "production_patch"]
```

### 8.8. Decisions

Stores process and architecture decisions.

Commands:

```bash
repodesk decision add "RepoDesk is private local tooling, not part of RepoPilot"
repodesk decision list
```

Example:

```md
## 2026-05-26 — RepoDesk is local-only

Decision:
RepoDesk will live outside target repositories.

Reason:
It is a personal workflow/control room, not a product feature.

Consequences:
- no public GitHub repo required initially;
- no release process initially;
- local paths and private workflow rules are allowed.
```

### 8.9. Risks

Stores known risks and mitigations.

Commands:

```bash
repodesk risk add "Codex over-edits when scope is vague"
repodesk risk list
```

Example:

```md
## Risk: Agents over-edit unrelated files

Mitigation:
- always use bounded prompts;
- max 8 source files per patch task;
- no broad refactor prompts;
- ask for diff summary before more changes.
```

---

## 9. Agent roles

### Ollama

Role: local summarizer / compressor.

Use for:

- log summaries;
- diff summaries;
- context compression;
- local experiments;
- embeddings later.

Do not use as final architect.

### ChatGPT

Role: architect / reviewer / mentor.

Use for:

- architecture;
- planning;
- review;
- Codex prompt writing;
- complex explanations;
- roadmap.

Do not use for:

- full repo reading;
- test execution;
- full log analysis.

### Codex

Role: patch executor.

Use for:

- bounded code changes;
- tests;
- small refactors;
- implementation from prepared prompt.

Do not use for:

- broad “improve the whole project” tasks;
- repeated full test runs;
- unsupervised architecture redesign.

### Gemini / Antigravity

Role: second opinion.

Use for:

- independent review;
- missed risks;
- docs/release review;
- alternative plan.

Do not use as main patch executor.

### Hermes Agent

Role: sandbox/local experimental agent.

Use only with restrictions:

- read-only review;
- sandbox only;
- no production repo write access;
- no unrestricted shell;
- no direct production patches.

### Aider / OpenCode / Cline / Roo

Optional tools for experiments.

Rule:

> Any coding agent must receive a small scope, a clear prompt, and limited permissions.

---

## 10. Desktop UI plan

RepoDesk Desktop should be an operations center.

### Dashboard

Shows:

- active project;
- active task;
- context size;
- budget status;
- last checks;
- recommended next action;
- open risks;
- recent decisions.

### Projects

Shows:

- project list;
- path;
- type;
- checks profile;
- ignore rules;
- project memory.

### Task Desk

Shows:

- task goal;
- scope;
- constraints;
- acceptance criteria;
- run folder;
- generated prompts.

### Context Builder

Shows:

- included files;
- excluded files;
- token estimate;
- warnings;
- compress button;
- copy/export context.

### Agents

Shows:

- agent roles;
- allowed tasks;
- forbidden tasks;
- current recommendation.

### Checks

Shows:

- available checks;
- last run;
- full local log;
- summary for AI;
- failed command.

### Token Ledger

Shows:

- estimated tokens per task;
- estimated tokens per agent;
- daily/weekly totals;
- most expensive tasks;
- warnings.

### Decisions & Risks

Shows:

- decision log;
- open risks;
- mitigations;
- process notes.

---

## 11. MVP v0.1 — CLI-first

Milestone name:

```txt
Local AI Control Room
```

Scope:

- Rust workspace;
- `repodesk-core` crate;
- `repodesk-cli` crate;
- local home directory initialization;
- project registry;
- active project;
- task creation;
- context builder;
- token estimator;
- prompt generator;
- check runner;
- failure summarizer.

Commands:

```bash
repodesk init
repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli
repodesk project list
repodesk project use repopilot
repodesk task new "Improve architecture anti-pattern detection"
repodesk task show
repodesk context build
repodesk context estimate
repodesk prompt codex
repodesk checks run
repodesk checks summarize
```

Acceptance criteria:

1. `~/.repodesk` can be initialized.
2. RepoPilot can be registered as a project.
3. RepoPilot can be selected as active project.
4. A task can be created.
5. `context.md` can be generated.
6. Estimated token count is shown.
7. Codex prompt can be generated.
8. Local checks can be run.
9. `checks-summary.md` is created when checks fail.

---

## 12. Roadmap

### v0.1 — CLI Local Control Room

- init;
- project registry;
- tasks;
- context builder;
- token estimator;
- prompt generator;
- check runner;
- failure summarizer.

### v0.2 — Ollama Integration

- compress context through Ollama;
- summarize logs through Ollama;
- summarize git diff;
- suggest relevant files;
- local model config.

### v0.3 — Project Profiles

- rust-cli profile;
- node/react profile;
- react-native profile;
- monorepo profile;
- python profile;
- custom checks per project.

### v0.4 — Budget Enforcement

- context budget warnings;
- block paid prompt generation if context too large;
- token ledger;
- daily/weekly reports;
- per-agent limits.

### v0.5 — Desktop MVP

- Tauri app shell;
- dashboard;
- project list;
- active task;
- context preview;
- checks panel;
- prompt copy/export.

### v0.6 — Agent Workflows

- ChatGPT architecture workflow;
- Codex patch workflow;
- Gemini second-review workflow;
- Ollama compression workflow;
- failure-fix workflow.

### v0.7 — Read-only MCP

- expose active project;
- expose active task;
- expose context pack;
- expose checks summary;
- expose token budget;
- no write access.

### v0.8 — Controlled Tools

- run fmt;
- run checks;
- create task;
- add decision;
- all with approval.

### v1.0 — Personal AI Operations Hub

- multi-project support;
- local memory;
- context packs;
- agent routing;
- token budgets;
- desktop dashboard;
- optional MCP;
- stable local workflow.

---

## 13. What not to build in v0.1

Do not build yet:

- full MCP server;
- web dashboard;
- complex database;
- cloud sync;
- login/auth;
- automatic git push;
- agents with unrestricted write access;
- automatic PR creation;
- marketplace integrations;
- full AI automation.

Start with what immediately reduces chaos:

- task boundaries;
- context packs;
- token estimate;
- local checks;
- failure summaries;
- prompt generation.

---

## 14. First practical implementation plan

### Step 1 — Create Rust workspace

```bash
mkdir repodesk
cd repodesk
mkdir -p crates
cargo new crates/repodesk-core --lib
cargo new crates/repodesk-cli --bin
```

Root `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/repodesk-core",
  "crates/repodesk-cli"
]
resolver = "2"
```

### Step 2 — Add dependencies

Core:

```toml
[dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }
walkdir = "2"
ignore = "0.4"
```

CLI:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
repodesk-core = { path = "../repodesk-core" }
```

### Step 3 — Implement core modules in order

1. `paths` — resolve `~/.repodesk`.
2. `init` — create local folders.
3. `projects` — add/list/use/info.
4. `tasks` — create/show active task.
5. `tokens` — estimate tokens.
6. `context` — build context pack.
7. `prompts` — generate Codex prompt.
8. `checks` — run configured checks.
9. `checks summary` — extract relevant failure lines.

### Step 4 — Test with RepoPilot

```bash
repodesk init
repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli
repodesk project use repopilot
repodesk task new "Improve architecture anti-pattern detection"
repodesk context build
repodesk context estimate
repodesk prompt codex
repodesk checks run
repodesk checks summarize
```

### Step 5 — Start desktop UI only after CLI works

Once CLI works, add Tauri desktop app.

---

## 15. First implementation slice

First slice:

```txt
Slice 1: init + project add/list/use/info
```

Files:

```txt
crates/repodesk-core/src/lib.rs
crates/repodesk-core/src/paths.rs
crates/repodesk-core/src/init.rs
crates/repodesk-core/src/projects.rs
crates/repodesk-cli/src/main.rs
```

Acceptance criteria:

```bash
repodesk init
repodesk project add repopilot ~/Documents/projects/repopilot --type rust-cli
repodesk project list
repodesk project use repopilot
repodesk project info
```

Expected result:

- `~/.repodesk` created;
- project config saved;
- active project saved;
- project list visible;
- no desktop UI yet.

---

## 16. Product direction summary

RepoDesk should become a private local tool for managing AI-assisted development as a professional process.

Value:

- less chaos;
- fewer wasted tokens;
- fewer accidental agent changes;
- better prompts;
- clear task scope;
- local logs;
- understandable checks;
- project memory;
- agent discipline;
- scalable workflow for future projects.

RepoDesk is not “AI writes code for me”.

RepoDesk is:

> My local cockpit for managing AI, code, context, agents, tests, and decisions.
