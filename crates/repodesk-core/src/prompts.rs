use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::budget::{evaluate_context, format_verdict, load_budget_config};
use crate::context::estimate_active_context;
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::format_estimate;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PromptKind {
    Codex,
    ChatGpt,
    Review,
}

#[derive(Debug, Clone)]
pub struct PromptBuildResult {
    pub prompt_file: PathBuf,
    pub prompt_kind: PromptKind,
    pub estimated_tokens: usize,
}

impl PromptKind {
    pub fn file_name(&self) -> &'static str {
        match self {
            PromptKind::Codex => "prompt.codex.md",
            PromptKind::ChatGpt => "prompt.chatgpt.md",
            PromptKind::Review => "prompt.review.md",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PromptKind::Codex => "Codex Patch Prompt",
            PromptKind::ChatGpt => "ChatGPT Architecture Prompt",
            PromptKind::Review => "Independent Review Prompt",
        }
    }
}

pub fn generate_prompt(kind: PromptKind) -> RepoDeskResult<PromptBuildResult> {
    let project = get_active_project()?;
    let task = show_active_task()?;

    let context_file = task.config.run_dir.join("context.md");
    let context = fs::read_to_string(&context_file)?;

    let (_context_path, estimate) = estimate_active_context()?;
    let budget_config = load_budget_config()?;
    let budget_verdict = evaluate_context(&estimate, &budget_config);

    let prompt = match kind {
        PromptKind::Codex => build_codex_prompt(
            &project.name,
            &task.config.title,
            &context,
            &estimate,
            &budget_verdict,
        ),
        PromptKind::ChatGpt => build_chatgpt_prompt(
            &project.name,
            &task.config.title,
            &context,
            &estimate,
            &budget_verdict,
        ),
        PromptKind::Review => build_review_prompt(
            &project.name,
            &task.config.title,
            &context,
            &estimate,
            &budget_verdict,
        ),
    };

    let prompt_file = task.config.run_dir.join(kind.file_name());
    fs::write(&prompt_file, prompt)?;

    Ok(PromptBuildResult {
        prompt_file,
        prompt_kind: kind,
        estimated_tokens: estimate.estimated_tokens,
    })
}

fn build_codex_prompt(
    project_name: &str,
    task_title: &str,
    context: &str,
    estimate: &crate::tokens::TokenEstimate,
    budget_verdict: &crate::budget::BudgetVerdict,
) -> String {
    format!(
        r#"# Codex Patch Prompt

You are working as a bounded patch executor for the project `{project_name}`.

## Task

{task_title}

## Your Role

You are not the architect.
You are not allowed to broadly redesign the project.
Your job is to implement a small, scoped patch based on the context below.

## Hard Rules

- Do not rewrite unrelated modules.
- Do not expand the task scope.
- Do not run expensive checks repeatedly.
- Do not read the full repository unless the context clearly says it is necessary.
- Do not change public CLI behavior unless the task explicitly requires it.
- Add or update tests near the changed code when relevant.
- Prefer small files and clear module boundaries.
- If the context is insufficient, ask for specific missing files.
- Return a concise diff summary.

## Recommended Local Checks

Run only when you have completed the patch or when the user asks:

```bash
cargo fmt
cargo check
cargo test
```

For Rust CLI projects, also consider:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Budget Status

```txt
{estimate}
```

```txt
{budget}
```

## Required Output

Return:

```txt
1. Changed files
2. What changed
3. Why it changed
4. Tests/checks run
5. Remaining risks
6. Any files you still need
```

## Context Pack

{context}
"#,
        project_name = project_name,
        task_title = task_title,
        estimate = format_estimate(estimate),
        budget = format_verdict(budget_verdict),
        context = context,
    )
}

fn build_chatgpt_prompt(
    project_name: &str,
    task_title: &str,
    context: &str,
    estimate: &crate::tokens::TokenEstimate,
    budget_verdict: &crate::budget::BudgetVerdict,
) -> String {
    format!(
        r#"# ChatGPT Architecture Prompt

You are acting as an architect/reviewer for the project `{project_name}`.

## Task

{task_title}

## Your Role

Analyze the context and help decide the best next engineering step.
Do not produce a huge implementation.
Do not ask to read the whole repository unless absolutely necessary.
Focus on architecture, risk, task boundaries, and a practical patch plan.

## What I Need From You

Return:

```txt
1. Short diagnosis
2. Architecture risks
3. Smallest safe implementation plan
4. Files likely involved
5. What Codex should do
6. What Codex must not do
7. Acceptance criteria
8. Follow-up slice
```

## Guardrails

- Keep the plan bounded.
- Prefer small slices.
- Avoid "rewrite everything" answers.
- Identify token waste risks.
- Identify whether context should be compressed before paid-agent work.
- If this should be handled locally by shell/Ollama, say so.

## Budget Status

```txt
{estimate}
```

```txt
{budget}
```

## Context Pack

{context}
"#,
        project_name = project_name,
        task_title = task_title,
        estimate = format_estimate(estimate),
        budget = format_verdict(budget_verdict),
        context = context,
    )
}

fn build_review_prompt(
    project_name: &str,
    task_title: &str,
    context: &str,
    estimate: &crate::tokens::TokenEstimate,
    budget_verdict: &crate::budget::BudgetVerdict,
) -> String {
    format!(
        r#"# Independent Review Prompt

You are reviewing the project `{project_name}` as a second-opinion reviewer.

## Task

{task_title}

## Your Role

Do not modify files.
Do not propose a broad rewrite.
Review the context for missed risks, bad assumptions, and unsafe next steps.

## Review Checklist

Check for:

```txt
- unclear task scope
- unnecessary token usage
- too much context
- missing acceptance criteria
- risky agent permissions
- missing tests
- over-engineering
- bad module boundaries
- likely false positives
- places where local shell/Ollama should be used instead of paid agents
```

## Required Output

Return:

```txt
1. Top 5 risks
2. What should be blocked
3. What should be simplified
4. What is safe to do next
5. Suggested next prompt for a patch agent
```

## Budget Status

```txt
{estimate}
```

```txt
{budget}
```

## Context Pack

{context}
"#,
        project_name = project_name,
        task_title = task_title,
        estimate = format_estimate(estimate),
        budget = format_verdict(budget_verdict),
        context = context,
    )
}
