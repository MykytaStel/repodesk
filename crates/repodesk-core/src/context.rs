use std::collections::HashSet;
use std::fs;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::context_freshness::apply_context_freshness;
use crate::context_packing::{ContextPackingPolicy, pack_context_candidates};
use crate::context_pipeline::{
    ContextCandidate, ContextPipelineSnapshot, ContextProvenance, ContextSelection,
    ContextSelectionState, ContextSourceKind, ContextTrust,
};
use crate::context_relevance::apply_context_relevance;
use crate::engineering::{
    ContextBuildTelemetry, ContextComponentTelemetry, EngineeringKnowledgeStatus, WorkItemContract,
    engineering_knowledge_context, load_active_engineering_knowledge, read_work_item_contract,
    record_context_build, select_task_scope_files, write_context_manifest,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::git_workspace::run_git_captured as run_git;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::{TokenEstimate, estimate_text, format_estimate};
use crate::usage::budget::load_budget_config;
use crate::utils::format_list;

/// RepoDesk 2 Project Knowledge is the reviewed engineering memory injected
/// first. The legacy Memory Brain remains during migration, but at a smaller
/// budget so it cannot dominate task-specific context.
const ENGINEERING_KNOWLEDGE_BUDGET_TOKENS: usize = 1_400;
const MEMORY_BUDGET_TOKENS: usize = 800;
const TASK_BUDGET_CHARS: usize = 8_000;
const PROJECT_MATERIAL_BUDGET_CHARS: usize = 6_000;
const CONTEXT_PIPELINE_SNAPSHOT_FILE: &str = "context-pipeline.json";
/// Component estimates are independent while the final mixed context can use a
/// slightly different language-cost model. Keep deterministic headroom so the
/// final rendered pack stays comfortably below the configured OK limit.
const PACKING_HEADROOM_PERCENT: usize = 10;

#[derive(Debug, Clone)]
pub struct ContextBuildResult {
    pub context_file: String,
    pub token_estimate_file: String,
    pub estimate: TokenEstimate,
}

#[derive(Clone, Copy)]
struct ContextPackInput<'a> {
    project_metadata: &'a str,
    repository_map: &'a str,
    task_metadata: &'a str,
    work_item_contract: &'a str,
    task_markdown: &'a str,
    scoped_files: &'a str,
    engineering_knowledge: &'a str,
    memory: &'a str,
    decisions: &'a str,
    risks: &'a str,
    checks: &'a str,
    ignore_rules: &'a str,
    git_state: &'a str,
    agent_notes: &'a str,
}

impl<'a> ContextPackInput<'a> {
    fn empty() -> Self {
        Self {
            project_metadata: "",
            repository_map: "",
            task_metadata: "",
            work_item_contract: "",
            task_markdown: "",
            scoped_files: "",
            engineering_knowledge: "",
            memory: "",
            decisions: "",
            risks: "",
            checks: "",
            ignore_rules: "",
            git_state: "",
            agent_notes: "",
        }
    }
}

pub fn build_context() -> RepoDeskResult<ContextBuildResult> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let project = get_active_project()?;
    let task = show_active_task()?;

    let project_meta_dir = paths.project_dir(&project.name);

    let task_markdown = read_optional_file(&task.task_markdown_file);
    let contract = read_work_item_contract(&task.config.run_dir)?;
    let contract_markdown = render_contract_context(contract.as_ref());
    let scope_source = scope_source_markdown(contract.as_ref(), &task_markdown);

    let knowledge_query = format!(
        "{}\n{}",
        task.config.title,
        contract
            .as_ref()
            .map(|value| value.goal.as_str())
            .unwrap_or_default()
    );
    let engineering_knowledge = engineering_knowledge_context(
        &project.name,
        &knowledge_query,
        ENGINEERING_KNOWLEDGE_BUDGET_TOKENS,
    )?;
    let engineering_knowledge_observed_at =
        engineering_knowledge_observed_at(&engineering_knowledge.included_ids);

    // Legacy Memory Brain remains temporarily as a lower-priority compatibility
    // slice. Keep the selected slice around long enough to attach provenance age
    // to the shared Context Pipeline; fallback memory.md remains unevaluated.
    let memory_slice = crate::memory::retrieval::memory_slice(&project.name, MEMORY_BUDGET_TOKENS)
        .ok()
        .filter(|slice| !slice.is_empty());
    let memory_observed_at = memory_slice
        .as_ref()
        .and_then(|slice| memory_slice_observed_at(&project.name, &slice.included_ids));
    let memory = memory_slice
        .as_ref()
        .map(|slice| slice.markdown.clone())
        .unwrap_or_else(|| read_optional_file(&project_meta_dir.join("memory.md")));
    let decisions = read_optional_file(&project_meta_dir.join("decisions.md"));
    let risks = read_optional_file(&project_meta_dir.join("risks.md"));

    // Repository structure is scanned through the same deterministic bounded
    // repo-map implementation used by explicit inspection surfaces. Only the
    // compact structural projection becomes a context candidate; no file bodies
    // are read for this source.
    let repository_map = crate::repo_map::build_repo_map_for(
        project.name.clone(),
        project.path.clone(),
    )?;
    let repository_map = crate::repo_map::format_repo_context(&repository_map);
    let repository_map_observed_at = Utc::now();

    // A configured typed contract is authoritative for file inclusion. Existing
    // tasks without one keep using the explicit backtick paths in task.md.
    let scoped_files = select_task_scope_files(
        &project.name,
        &task.config.id,
        &project.path,
        &scope_source,
        &project.context_ignore,
    );

    let git_branch = run_git(&project.path, &["branch", "--show-current"]);
    let git_status = run_git(&project.path, &["status", "--short"]);
    let git_diff_stat = run_git(&project.path, &["diff", "--stat"]);
    let git_changed_files = run_git(&project.path, &["diff", "--name-only"]);
    let changed_files = git_changed_files
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let project_metadata = format!(
        "Name: `{}`\nPath: `{}`\nType: `{}`\nMain language: `{}`",
        project.name,
        project.path.display(),
        project.project_type,
        project
            .main_language
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    );
    let task_metadata = format!(
        "Id: `{}`\nTitle: `{}`\nStatus: `open`\nRun dir: `{}`",
        task.config.id,
        task.config.title,
        task.config.run_dir.display()
    );
    let checks = format_list(&project.checks);
    let ignore_rules = format_list(&project.context_ignore);
    let git_state = format!(
        "## Git Branch\n\n```txt\n{}\n```\n\n## Git Status\n\n```txt\n{}\n```\n\n## Git Diff Stat\n\n```txt\n{}\n```\n\n## Git Changed Files\n\n```txt\n{}\n```",
        git_branch.trim(),
        fallback_empty(git_status.trim(), "No git status output."),
        fallback_empty(git_diff_stat.trim(), "No git diff stat."),
        fallback_empty(git_changed_files.trim(), "No changed files.")
    );
    let scope_source_label = if contract
        .as_ref()
        .is_some_and(|value| !value.allowed_paths.is_empty())
    {
        "the typed Work Item Contract"
    } else {
        "explicit backtick paths in the legacy Work Item Scope"
    };
    let agent_notes = format!(
        "- This context pack is generated by RepoDesk.\n- Repository file content is bounded by {scope_source_label}.\n- Repository Structure is metadata-only and never implies permission to read or edit listed files.\n- Accepted Project Engineering Knowledge is human-reviewed; candidates are never injected.\n- The Work Item Contract is authoritative when configured; protected paths must not be changed.\n- Acceptance criteria describe what must be proven before completion.\n- Do not assume this is the full repository.\n- Ask for additional specific files only when needed.\n- Do not run expensive checks repeatedly.\n- Prefer local shell checks through RepoDesk."
    );

    let included_task_markdown = trim_for_context(&task_markdown, TASK_BUDGET_CHARS);
    let included_memory = trim_for_context(&memory, PROJECT_MATERIAL_BUDGET_CHARS);
    let included_decisions = trim_for_context(&decisions, PROJECT_MATERIAL_BUDGET_CHARS);
    let included_risks = trim_for_context(&risks, PROJECT_MATERIAL_BUDGET_CHARS);

    let candidate_input = ContextPackInput {
        project_metadata: &project_metadata,
        repository_map: &repository_map,
        task_metadata: &task_metadata,
        work_item_contract: &contract_markdown,
        task_markdown: &task_markdown,
        scoped_files: &scoped_files.candidate_rendered,
        engineering_knowledge: &engineering_knowledge.candidate_markdown,
        memory: &memory,
        decisions: &decisions,
        risks: &risks,
        checks: &checks,
        ignore_rules: &ignore_rules,
        git_state: &git_state,
        agent_notes: &agent_notes,
    };
    let bounded_input = ContextPackInput {
        task_markdown: &included_task_markdown,
        scoped_files: &scoped_files.rendered,
        engineering_knowledge: &engineering_knowledge.markdown,
        memory: &included_memory,
        decisions: &included_decisions,
        risks: &included_risks,
        ..candidate_input
    };

    let candidate_context = render_context_pack(&candidate_input);
    let candidate_estimate = estimate_text(&candidate_context);

    let relevance_text = format!(
        "{}\n{}",
        task.config.title,
        contract
            .as_ref()
            .map(|value| value.goal.as_str())
            .unwrap_or_default()
    );
    let mut pipeline_candidates = build_pipeline_candidates(&bounded_input, contract.is_some());
    for candidate in &mut pipeline_candidates {
        apply_context_relevance(candidate, &relevance_text, &changed_files);
    }
    apply_named_freshness(
        &mut pipeline_candidates,
        "repository_map",
        Some(repository_map_observed_at),
    );
    apply_named_freshness(
        &mut pipeline_candidates,
        "engineering_knowledge",
        engineering_knowledge_observed_at,
    );
    apply_named_freshness(
        &mut pipeline_candidates,
        "legacy_memory",
        memory_observed_at,
    );

    let budget_config = load_budget_config()?;
    let framing_tokens = estimate_text(&render_context_pack(&ContextPackInput::empty())).estimated_tokens;
    let raw_content_budget = budget_config.context_ok_limit.saturating_sub(framing_tokens);
    let content_budget = raw_content_budget
        .saturating_mul(100usize.saturating_sub(PACKING_HEADROOM_PERCENT))
        / 100;
    let packing = pack_context_candidates(
        &pipeline_candidates,
        ContextPackingPolicy {
            token_budget: content_budget,
        },
    );

    let packed_input = ContextPackInput {
        project_metadata: selected_component(
            "project_metadata",
            bounded_input.project_metadata,
            &packing.selections,
        ),
        repository_map: selected_component(
            "repository_map",
            bounded_input.repository_map,
            &packing.selections,
        ),
        task_metadata: selected_component(
            "task_metadata",
            bounded_input.task_metadata,
            &packing.selections,
        ),
        work_item_contract: selected_component(
            "work_item_contract",
            bounded_input.work_item_contract,
            &packing.selections,
        ),
        task_markdown: selected_component(
            "task_document",
            bounded_input.task_markdown,
            &packing.selections,
        ),
        scoped_files: selected_component(
            "scoped_files",
            bounded_input.scoped_files,
            &packing.selections,
        ),
        engineering_knowledge: selected_component(
            "engineering_knowledge",
            bounded_input.engineering_knowledge,
            &packing.selections,
        ),
        memory: selected_component(
            "legacy_memory",
            bounded_input.memory,
            &packing.selections,
        ),
        decisions: selected_component(
            "decisions",
            bounded_input.decisions,
            &packing.selections,
        ),
        risks: selected_component("risks", bounded_input.risks, &packing.selections),
        checks: selected_component("checks", bounded_input.checks, &packing.selections),
        ignore_rules: selected_component(
            "ignore_rules",
            bounded_input.ignore_rules,
            &packing.selections,
        ),
        git_state: selected_component(
            "git_state",
            bounded_input.git_state,
            &packing.selections,
        ),
        agent_notes: selected_component(
            "agent_notes",
            bounded_input.agent_notes,
            &packing.selections,
        ),
    };

    let context = render_context_pack(&packed_input);
    let estimate = estimate_text(&context);
    let context_fingerprint = sha256_hex(&context);

    let components = vec![
        context_component(
            "project_metadata",
            &project_metadata,
            packed_input.project_metadata,
        ),
        context_component("repository_map", &repository_map, packed_input.repository_map),
        context_component("task_metadata", &task_metadata, packed_input.task_metadata),
        context_component(
            "work_item_contract",
            &contract_markdown,
            packed_input.work_item_contract,
        ),
        context_component("task", &task_markdown, packed_input.task_markdown),
        context_component(
            "scoped_files",
            &scoped_files.candidate_rendered,
            packed_input.scoped_files,
        ),
        context_component(
            "engineering_knowledge",
            &engineering_knowledge.candidate_markdown,
            packed_input.engineering_knowledge,
        ),
        context_component("memory", &memory, packed_input.memory),
        context_component("decisions", &decisions, packed_input.decisions),
        context_component("risks", &risks, packed_input.risks),
        context_component("checks", &checks, packed_input.checks),
        context_component("ignore_rules", &ignore_rules, packed_input.ignore_rules),
        context_component("git_state", &git_state, packed_input.git_state),
        context_component("agent_notes", &agent_notes, packed_input.agent_notes),
    ];

    let pipeline_snapshot = ContextPipelineSnapshot::new(
        &project.name,
        &task.config.id,
        &context_fingerprint,
        Some(content_budget),
        pipeline_candidates,
        packing.selections,
    )
    .map_err(|error| RepoDeskError::Api(format!("context pipeline validation: {error}")))?;

    let context_file = task.config.run_dir.join("context.md");
    let token_estimate_file = task.config.run_dir.join("token-estimate.txt");
    let pipeline_file = task.config.run_dir.join(CONTEXT_PIPELINE_SNAPSHOT_FILE);
    let manifest_file = write_context_manifest(&task.config.run_dir, &scoped_files.manifest)?;

    fs::write(&context_file, context)?;
    fs::write(&token_estimate_file, format_estimate(&estimate))?;
    fs::write(
        pipeline_file,
        serde_json::to_string_pretty(&pipeline_snapshot)?,
    )?;

    let context_file = context_file.display().to_string();
    let token_estimate_file = token_estimate_file.display().to_string();
    let manifest_file = manifest_file.display().to_string();

    // Engineering telemetry stays best-effort: a context pack that was built
    // successfully remains usable even if the append-only ledger cannot be
    // updated. The event stores counts, paths, and hashes, never raw source
    // content from the scoped-file selection or knowledge store.
    let _ = record_context_build(
        &task.config,
        ContextBuildTelemetry {
            context_file: &context_file,
            token_estimate_file: &token_estimate_file,
            manifest_file: Some(&manifest_file),
            manifest: Some(&scoped_files.manifest),
            included_tokens: estimate.estimated_tokens,
            candidate_tokens: candidate_estimate.estimated_tokens,
            context_fingerprint: &context_fingerprint,
            components: &components,
        },
    );

    Ok(ContextBuildResult {
        context_file,
        token_estimate_file,
        estimate,
    })
}

pub fn estimate_active_context() -> RepoDeskResult<(String, TokenEstimate)> {
    let task = show_active_task()?;
    let context_file = task.config.run_dir.join("context.md");

    let content = fs::read_to_string(&context_file)?;
    let estimate = estimate_text(&content);

    Ok((context_file.display().to_string(), estimate))
}

fn engineering_knowledge_observed_at(included_ids: &[String]) -> Option<DateTime<Utc>> {
    if included_ids.is_empty() {
        return None;
    }
    let included = included_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let snapshot = load_active_engineering_knowledge().ok()?;

    snapshot
        .records
        .into_iter()
        .filter(|record| record.status == EngineeringKnowledgeStatus::Accepted)
        .filter(|record| included.contains(record.id.as_str()))
        .map(|record| record.updated_at)
        .min()
}

fn memory_slice_observed_at(project: &str, included_ids: &[i64]) -> Option<DateTime<Utc>> {
    if included_ids.is_empty() {
        return None;
    }
    let included = included_ids.iter().copied().collect::<HashSet<_>>();

    crate::memory::store::list_active(project)
        .ok()?
        .into_iter()
        .filter(|entry| included.contains(&entry.id))
        .map(|entry| entry.updated_at.unwrap_or(entry.timestamp))
        .min()
}

fn apply_named_freshness(
    candidates: &mut [ContextCandidate],
    candidate_id: &str,
    observed_at: Option<DateTime<Utc>>,
) {
    if let Some(candidate) = candidates
        .iter_mut()
        .find(|candidate| candidate.id == candidate_id)
    {
        apply_context_freshness(candidate, observed_at);
    }
}

fn build_pipeline_candidates(
    input: &ContextPackInput<'_>,
    has_contract: bool,
) -> Vec<ContextCandidate> {
    vec![
        pipeline_candidate(
            "project_metadata",
            ContextSourceKind::ProjectMetadata,
            ContextTrust::Observed,
            "project:metadata",
            input.project_metadata,
            true,
        ),
        pipeline_candidate(
            "repository_map",
            ContextSourceKind::RepositoryMap,
            ContextTrust::Observed,
            "repository:bounded-map",
            input.repository_map,
            false,
        ),
        pipeline_candidate(
            "task_metadata",
            ContextSourceKind::TaskMetadata,
            ContextTrust::Authoritative,
            "task:metadata",
            input.task_metadata,
            true,
        ),
        pipeline_candidate(
            "work_item_contract",
            ContextSourceKind::WorkItemContract,
            ContextTrust::Authoritative,
            crate::engineering::WORK_ITEM_CONTRACT_FILE,
            input.work_item_contract,
            has_contract,
        ),
        pipeline_candidate(
            "task_document",
            ContextSourceKind::TaskDocument,
            ContextTrust::Authoritative,
            "task.md",
            input.task_markdown,
            true,
        ),
        pipeline_candidate(
            "scoped_files",
            ContextSourceKind::ScopedFile,
            ContextTrust::Observed,
            "task-scope:repository-files",
            input.scoped_files,
            false,
        ),
        pipeline_candidate(
            "engineering_knowledge",
            ContextSourceKind::EngineeringKnowledge,
            ContextTrust::Reviewed,
            "project:engineering-knowledge",
            input.engineering_knowledge,
            false,
        ),
        pipeline_candidate(
            "legacy_memory",
            ContextSourceKind::LegacyMemory,
            ContextTrust::Legacy,
            "project:legacy-memory",
            input.memory,
            false,
        ),
        pipeline_candidate(
            "decisions",
            ContextSourceKind::DecisionLog,
            ContextTrust::Observed,
            "project:decisions",
            input.decisions,
            false,
        ),
        pipeline_candidate(
            "risks",
            ContextSourceKind::RiskLog,
            ContextTrust::Observed,
            "project:risks",
            input.risks,
            false,
        ),
        pipeline_candidate(
            "checks",
            ContextSourceKind::Checks,
            ContextTrust::Authoritative,
            "project:checks",
            input.checks,
            false,
        ),
        pipeline_candidate(
            "ignore_rules",
            ContextSourceKind::AgentRules,
            ContextTrust::Authoritative,
            "project:context-ignore",
            input.ignore_rules,
            false,
        ),
        pipeline_candidate(
            "git_state",
            ContextSourceKind::GitState,
            ContextTrust::Observed,
            "git:working-tree",
            input.git_state,
            false,
        ),
        pipeline_candidate(
            "agent_notes",
            ContextSourceKind::AgentRules,
            ContextTrust::Authoritative,
            "repodesk:agent-notes",
            input.agent_notes,
            true,
        ),
    ]
}

fn pipeline_candidate(
    id: &str,
    kind: ContextSourceKind,
    trust: ContextTrust,
    locator: &str,
    text: &str,
    required: bool,
) -> ContextCandidate {
    ContextCandidate {
        id: id.to_string(),
        provenance: ContextProvenance {
            kind,
            locator: locator.to_string(),
            fingerprint: sha256_hex(text),
            observed_at: None,
        },
        trust,
        candidate_tokens: estimate_text(text).estimated_tokens,
        required,
        relevance_score: None,
        freshness_score: None,
    }
}

fn selected_component<'a>(
    id: &str,
    value: &'a str,
    selections: &[ContextSelection],
) -> &'a str {
    selections
        .iter()
        .find(|selection| selection.candidate_id == id)
        .filter(|selection| selection.state == ContextSelectionState::Included)
        .map(|_| value)
        .unwrap_or("")
}

fn render_context_pack(input: &ContextPackInput<'_>) -> String {
    format!(
        r#"# RepoDesk Context Pack

## Project

{project_metadata}

## Repository Structure

{repository_map}

## Active Task

{task_metadata}

## Work Item Contract

{work_item_contract}

## Task File

{task_markdown}

## Scoped Repository Files

{scoped_files}

## Project Engineering Knowledge

{engineering_knowledge}

## Legacy Project Memory

{memory}

## Decisions

{decisions}

## Risks

{risks}

## Configured Checks

{checks}

## Context Ignore Rules

{ignore_rules}

{git_state}

## Notes For Agents

{agent_notes}

"#,
        project_metadata = input.project_metadata,
        repository_map = input.repository_map,
        task_metadata = input.task_metadata,
        work_item_contract = input.work_item_contract,
        task_markdown = input.task_markdown,
        scoped_files = input.scoped_files,
        engineering_knowledge = input.engineering_knowledge,
        memory = input.memory,
        decisions = input.decisions,
        risks = input.risks,
        checks = input.checks,
        ignore_rules = input.ignore_rules,
        git_state = input.git_state,
        agent_notes = input.agent_notes,
    )
}

fn render_contract_context(contract: Option<&WorkItemContract>) -> String {
    let Some(contract) = contract else {
        return "Not configured. RepoDesk is using the legacy task document for scope guidance."
            .to_string();
    };

    format!(
        "Source: `{}`\n\n### Goal\n\n{}\n\n### Allowed paths\n\n{}\n\n### Protected paths\n\n{}\n\n### Acceptance criteria\n\n{}",
        crate::engineering::WORK_ITEM_CONTRACT_FILE,
        fallback_empty(contract.goal.trim(), "Not defined."),
        contract_list(&contract.allowed_paths),
        contract_list(&contract.protected_paths),
        contract_list(&contract.acceptance_criteria),
    )
}

fn contract_list(values: &[String]) -> String {
    if values.is_empty() {
        return "- Not defined.".to_string();
    }
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn scope_source_markdown(contract: Option<&WorkItemContract>, task_markdown: &str) -> String {
    let Some(contract) = contract.filter(|value| !value.allowed_paths.is_empty()) else {
        return task_markdown.to_string();
    };

    let paths = contract
        .allowed_paths
        .iter()
        .map(|path| format!("- `{path}`"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("## Scope\n\n{paths}\n")
}

fn context_component(name: &str, candidate: &str, included: &str) -> ContextComponentTelemetry {
    ContextComponentTelemetry {
        name: name.to_string(),
        candidate_tokens: estimate_text(candidate).estimated_tokens,
        included_tokens: estimate_text(included).estimated_tokens,
        trimmed: candidate != included,
        fingerprint: sha256_hex(included),
    }
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn read_optional_file(path: &std::path::Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| "Not available.".to_string())
}

fn trim_for_context(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let trimmed = value.chars().take(max_chars).collect::<String>();

    format!("{trimmed}\n\n[RepoDesk: content trimmed for context budget]")
}

fn fallback_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn component_telemetry_records_trimming_without_raw_text() {
        let component = context_component("task", "abcdef", "abc");

        assert_eq!(component.name, "task");
        assert!(component.trimmed);
        assert!(!component.fingerprint.contains("abc"));
        assert_eq!(component.fingerprint.len(), 64);
    }

    #[test]
    fn render_keeps_candidate_and_included_pack_structure_identical() {
        let input = ContextPackInput {
            project_metadata: "project",
            repository_map: "rust: 20 files",
            task_metadata: "task meta",
            work_item_contract: "contract",
            task_markdown: "task body",
            scoped_files: "### `src/lib.rs`\ncode",
            engineering_knowledge: "### [hazard] Auth\nNever bypass validation.",
            memory: "legacy memory",
            decisions: "decisions",
            risks: "risks",
            checks: "checks",
            ignore_rules: "ignore",
            git_state: "git",
            agent_notes: "notes",
        };

        let rendered = render_context_pack(&input);
        assert!(rendered.contains("## Repository Structure\n\nrust: 20 files"));
        assert!(rendered.contains("## Work Item Contract\n\ncontract"));
        assert!(rendered.contains("## Task File\n\ntask body"));
        assert!(rendered.contains("## Scoped Repository Files\n\n### `src/lib.rs`"));
        assert!(rendered.contains("## Project Engineering Knowledge"));
        assert!(rendered.contains("Never bypass validation"));
        assert!(rendered.contains("## Legacy Project Memory\n\nlegacy memory"));
        assert!(rendered.contains("## Notes For Agents\n\nnotes"));
    }

    #[test]
    fn typed_contract_scope_becomes_selection_source() {
        let contract = WorkItemContract {
            version: 1,
            project: "repodesk".into(),
            work_item_id: "task-1".into(),
            goal: "goal".into(),
            allowed_paths: vec!["src/lib.rs".into(), "Cargo.toml".into()],
            protected_paths: vec!["src/security".into()],
            acceptance_criteria: vec!["tests pass".into()],
            updated_at: Utc::now(),
        };

        let source = scope_source_markdown(Some(&contract), "## Scope\n\n- `legacy.rs`");
        assert!(source.contains("`src/lib.rs`"));
        assert!(source.contains("`Cargo.toml`"));
        assert!(!source.contains("legacy.rs"));
    }

    #[test]
    fn pipeline_candidates_keep_authoritative_material_required() {
        let input = ContextPackInput {
            project_metadata: "project",
            repository_map: "repo map",
            task_metadata: "task meta",
            work_item_contract: "contract",
            task_markdown: "task body",
            scoped_files: "files",
            engineering_knowledge: "knowledge",
            memory: "memory",
            decisions: "decisions",
            risks: "risks",
            checks: "checks",
            ignore_rules: "ignore",
            git_state: "git",
            agent_notes: "notes",
        };

        let candidates = build_pipeline_candidates(&input, true);
        let required = candidates
            .iter()
            .filter(|candidate| candidate.required)
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>();

        assert!(required.contains(&"project_metadata"));
        assert!(required.contains(&"task_metadata"));
        assert!(required.contains(&"work_item_contract"));
        assert!(required.contains(&"task_document"));
        assert!(required.contains(&"agent_notes"));
        assert!(!required.contains(&"repository_map"));
        assert!(!required.contains(&"legacy_memory"));
    }

    #[test]
    fn excluded_pipeline_component_renders_no_payload() {
        let selections = vec![ContextSelection {
            candidate_id: "repository_map".into(),
            state: ContextSelectionState::Excluded,
            included_tokens: 0,
            trimmed: false,
            exclusion_reason: Some(crate::context_pipeline::ContextExclusionReason::Budget),
            order: None,
        }];

        assert_eq!(selected_component("repository_map", "structural metadata", &selections), "");
    }

    #[test]
    fn named_freshness_updates_only_the_target_candidate() {
        let input = ContextPackInput {
            project_metadata: "project",
            repository_map: "repo map",
            task_metadata: "task meta",
            work_item_contract: "contract",
            task_markdown: "task body",
            scoped_files: "files",
            engineering_knowledge: "knowledge",
            memory: "memory",
            decisions: "decisions",
            risks: "risks",
            checks: "checks",
            ignore_rules: "ignore",
            git_state: "git",
            agent_notes: "notes",
        };
        let mut candidates = build_pipeline_candidates(&input, true);
        let observed_at = Utc::now() - Duration::days(7);

        apply_named_freshness(&mut candidates, "repository_map", Some(observed_at));

        let repository = candidates
            .iter()
            .find(|candidate| candidate.id == "repository_map")
            .unwrap();
        let knowledge = candidates
            .iter()
            .find(|candidate| candidate.id == "engineering_knowledge")
            .unwrap();
        assert_eq!(repository.provenance.observed_at, Some(observed_at));
        assert_eq!(repository.freshness_score, Some(0.5));
        assert_eq!(knowledge.freshness_score, None);
    }
}
