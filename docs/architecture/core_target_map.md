# Target Core Module Map

This document outlines the long-term target module grouping for `repodesk-core`.

**Important Rule:**
> Do not mass-move all core modules yet.
> Document the target map first, then migrate one domain at a time.

## Target Architecture

```text
crates/repodesk-core/src/
  ai/
    mod.rs
    adapters.rs
    discovery.rs

  orchestration/
    mod.rs
    brain.rs
    workflow.rs
    workflow_doctor.rs
    judge.rs
    guard.rs

  execution/
    mod.rs
    checks.rs
    command_sandbox.rs
    sandbox.rs
    runtime.rs

  system/
    mod.rs
    paths.rs
    module_registry.rs
    capabilities.rs
    peripherals.rs
    sessions.rs

  security/
    mod.rs
    safety.rs
    security.rs

  presentation/
    mod.rs
    dashboard.rs
    desktop.rs
    ui_snapshot.rs

  project/
    mod.rs
    projects.rs
    tasks.rs
    context.rs
    smart_context.rs
    repo_map.rs
    project_token_check.rs

  routing/
    mod.rs
    routing.rs

  agents/
    mod.rs
    agents.rs

  usage/
    mod.rs
    budget.rs
    cost.rs
    token_ledger.rs

  persistence/
    mod.rs
    db.rs
    event_journal.rs
    receipts.rs
    knowledge.rs

  errors.rs
  lib.rs
```
