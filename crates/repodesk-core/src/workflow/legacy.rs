use std::path::PathBuf;

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;

pub fn workflow_next() -> RepoDeskResult<String> {
    let project = get_active_project()?;
    let task = show_active_task()?;

    let run_dir = &task.config.run_dir;
    let context = run_dir.join("context.md");
    let codex_prompt = run_dir.join("prompt.codex.md");
    let chatgpt_prompt = run_dir.join("prompt.chatgpt.md");
    let review_prompt = run_dir.join("prompt.review.md");
    let checks_summary = run_dir.join("checks-summary.md");

    let next = if !context.exists() {
        "Run `repodesk context build`."
    } else if !codex_prompt.exists() || !chatgpt_prompt.exists() || !review_prompt.exists() {
        "Run `repodesk prompt all`."
    } else if !checks_summary.exists() {
        "Run `repodesk checks run` before using a patch agent."
    } else {
        "Run `repodesk guard preflight --agent codex`, then choose the next bounded agent step."
    };

    Ok(format!(
        r#"Workflow next step:

Project: {}
Task: {}
Run dir: {}

Next:
  {}

Suggested order:
  1. repodesk context build
  2. repodesk safety scan-context
  3. repodesk budget check-context
  4. repodesk prompt all
  5. repodesk checks run
  6. repodesk guard preflight --agent codex
  7. send generated prompt to selected agent only if guard allows
"#,
        project.name,
        task.config.title,
        run_dir.display(),
        next
    ))
}

pub fn create_workflow_plan() -> RepoDeskResult<PathBuf> {
    let task = show_active_task()?;
    let file = task.config.run_dir.join("workflow-plan.md");
    let content = build_workflow_plan_markdown()?;
    std::fs::write(&file, content)?;
    Ok(file)
}

pub fn read_or_create_workflow_plan() -> RepoDeskResult<String> {
    let task = show_active_task()?;
    let file = task.config.run_dir.join("workflow-plan.md");

    if !file.exists() {
        create_workflow_plan()?;
    }

    Ok(std::fs::read_to_string(file)?)
}

fn build_workflow_plan_markdown() -> RepoDeskResult<String> {
    let project = get_active_project()?;
    let task = show_active_task()?;

    Ok(format!(
        r#"# RepoDesk Workflow Plan

## Project

- Name: `{}`
- Path: `{}`

## Task

- Id: `{}`
- Title: `{}`

## Operating Principle

RepoDesk is the control brain. Agents are modules. Agents do not decide scope, budget, or safety by themselves.

## Workflow

1. Build context

```bash
repodesk context build
```

2. Scan context safety

```bash
repodesk safety scan-context
```

3. Check budget

```bash
repodesk budget check-context
```

4. Generate prompts

```bash
repodesk prompt all
```

5. Run checks

```bash
repodesk checks run
```

6. Run guard

```bash
repodesk guard preflight --agent codex
```

7. Use the selected agent only with generated bounded prompt.

## Do Not

- Do not give agents the whole repository by default.
- Do not send secrets or `.env` content.
- Do not let patch agents run unrestricted shell commands.
- Do not expand scope without creating a new task.
"#,
        project.name,
        project.path.display(),
        task.config.id,
        task.config.title,
    ))
}
