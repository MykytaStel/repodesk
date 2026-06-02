use std::path::{Path, PathBuf};
use tokio::fs;

use crate::errors::RepoDeskResult;
use crate::git_workspace::git_lines;
use crate::projects::get_active_project;
use crate::repo_map::{build_repo_map, format_repo_map};
use crate::tasks::show_active_task;
use crate::tokens::{TokenEstimate, estimate_text, format_estimate};
use crate::usage::budget::{evaluate_context, format_verdict, load_budget_config};

const MAX_CANDIDATE_FILES: usize = 12;
const MAX_PREVIEW_CHARS: usize = 4_000;
const MAX_TASK_CHARS: usize = 5_000;
const MAX_REPOMAP_CHARS: usize = 8_000;
/// Memory Brain gets a lean budget here — the smart pack is meant for paid
/// agents, so durable decisions/constraints are worth the tokens but must stay small.
const MAX_MEMORY_TOKENS: usize = 900;

#[derive(Debug, Clone)]
pub struct SmartContextResult {
    pub context_file: PathBuf,
    pub token_estimate_file: PathBuf,
    pub estimate: TokenEstimate,
    pub included_files: Vec<String>,
    pub skipped_files: Vec<String>,
}

pub async fn build_smart_context() -> RepoDeskResult<SmartContextResult> {
    let project = get_active_project()?;
    let task = show_active_task()?;
    let repo_map = build_repo_map().await?;

    let changed_files = git_lines(&project.path, &["diff", "--name-only"]);
    let staged_files = git_lines(&project.path, &["diff", "--cached", "--name-only"]);

    let mut candidate_files = Vec::new();
    candidate_files.extend(changed_files);
    candidate_files.extend(staged_files);
    candidate_files.sort();
    candidate_files.dedup();

    let mut included_files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut file_sections = Vec::new();

    for relative in candidate_files.iter().take(MAX_CANDIDATE_FILES) {
        if !is_safe_text_path(relative) {
            skipped_files.push(format!("{relative} — unsupported or risky file type"));
            continue;
        }

        let full_path = project.path.join(relative);
        let content = match fs::read_to_string(&full_path).await {
            Ok(content) => content,
            Err(_) => {
                skipped_files.push(format!("{relative} — could not read as UTF-8 text"));
                continue;
            }
        };

        let trimmed = trim_text(&content, MAX_PREVIEW_CHARS);
        included_files.push(relative.clone());
        file_sections.push(format!("## File: `{relative}`\n\n```txt\n{trimmed}\n```\n"));
    }

    if candidate_files.len() > MAX_CANDIDATE_FILES {
        skipped_files.push(format!(
            "{} additional changed files skipped by smart-context file limit",
            candidate_files.len() - MAX_CANDIDATE_FILES
        ));
    }

    let task_markdown = fs::read_to_string(&task.task_markdown_file)
        .await
        .unwrap_or_else(|_| "Task markdown is not available.".to_string());

    // Shared Memory Brain slice — the same durable context every agent sees.
    let memory_brain =
        crate::memory::retrieval::memory_slice_markdown(&project.name, MAX_MEMORY_TOKENS)
            .unwrap_or_else(|_| "Project memory unavailable.".to_string());

    let context = format!(
        r#"# RepoDesk Smart Context Pack

## Purpose

This is a smaller, safer context pack optimized for paid agents.
It prefers active task data, repository map, git status, and changed file snippets.

## Active Task

{task_markdown}

## Project Memory (Brain)

{memory_brain}

## Repository Map

{repo_map}

## Included Changed Files

{included_files}

## Skipped Files

{skipped_files}

## Changed File Snippets

{file_sections}

## Agent Rules

- Treat this as bounded context.
- Do not request the full repository unless necessary.
- Prefer small patches.
- Do not touch secrets or credentials.
- Ask for specific missing files only.
"#,
        task_markdown = trim_text(&task_markdown, MAX_TASK_CHARS),
        memory_brain = memory_brain,
        repo_map = trim_text(&format_repo_map(&repo_map), MAX_REPOMAP_CHARS),
        included_files = format_lines(&included_files),
        skipped_files = format_lines(&skipped_files),
        file_sections = file_sections.join("\n"),
    );

    let estimate = estimate_text(&context);
    let budget = load_budget_config()?;
    let verdict = evaluate_context(&estimate, &budget);

    let final_context = format!(
        "{context}\n\n## Token Estimate\n\n```txt\n{}\n```\n\n## Budget Verdict\n\n```txt\n{}\n```\n",
        format_estimate(&estimate),
        format_verdict(&verdict)
    );

    let context_file = task.config.run_dir.join("smart-context.md");
    let token_estimate_file = task.config.run_dir.join("smart-token-estimate.txt");

    fs::write(&context_file, final_context).await?;
    fs::write(&token_estimate_file, format_estimate(&estimate)).await?;

    Ok(SmartContextResult {
        context_file,
        token_estimate_file,
        estimate,
        included_files,
        skipped_files,
    })
}

pub fn list_smart_context_sources() -> RepoDeskResult<String> {
    let project = get_active_project()?;
    let changed_files = git_lines(&project.path, &["diff", "--name-only"]);
    let staged_files = git_lines(&project.path, &["diff", "--cached", "--name-only"]);

    let mut files = Vec::new();
    files.extend(changed_files);
    files.extend(staged_files);
    files.sort();
    files.dedup();

    if files.is_empty() {
        return Ok(
            "No changed or staged files detected. Smart context will use task + repo map only.\n"
                .to_string(),
        );
    }

    let mut output = String::new();
    output.push_str("Smart context candidate files:\n");

    for file in files {
        let marker = if is_safe_text_path(&file) {
            "include"
        } else {
            "skip"
        };
        output.push_str(&format!("  - [{marker}] {file}\n"));
    }

    Ok(output)
}

pub fn format_smart_context_result(result: &SmartContextResult) -> String {
    format!(
        "Smart context built:\n  context file: {}\n  token estimate file: {}\n  included files: {}\n  skipped files: {}\n\n{}",
        result.context_file.display(),
        result.token_estimate_file.display(),
        result.included_files.len(),
        result.skipped_files.len(),
        format_estimate(&result.estimate)
    )
}

fn is_safe_text_path(path: &str) -> bool {
    let lower = path.to_lowercase();

    if lower.contains(".env")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
    {
        return false;
    }

    matches!(
        Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or(""),
        "rs" | "toml"
            | "md"
            | "txt"
            | "json"
            | "yml"
            | "yaml"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "sh"
    )
}

fn trim_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!(
            "{}\n\n[RepoDesk: trimmed to {} chars]",
            value.chars().take(max_chars).collect::<String>(),
            max_chars
        )
    }
}

fn format_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        "- none".to_string()
    } else {
        lines
            .iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
