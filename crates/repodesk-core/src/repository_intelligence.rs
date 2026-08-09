//! Bounded, evidence-backed repository intelligence for the active project.
//!
//! This module deliberately prefers small explainable neighborhoods over a
//! repository-wide in-memory graph. Rust import/module relationships come from
//! `syn`; Git co-change relationships come from bounded history. Other
//! languages remain eligible for history/test proximity without pretending
//! RepoDesk has parsed symbols it cannot yet resolve.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use syn::{Item, UseTree};

use crate::code_workspace::{CodeWorkspaceFile, language_for_path, load_code_workspace};
use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;

pub const REPOSITORY_INTELLIGENCE_VERSION: u32 = 1;
const MAX_RUST_FILES: usize = 4_000;
const MAX_RUST_INDEX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SINGLE_INDEX_FILE_BYTES: u64 = 384 * 1024;
const MAX_GIT_COMMITS: usize = 200;
const MAX_FILES_PER_COMMIT: usize = 200;
const MAX_RELATIONS: usize = 24;
const MAX_TESTS: usize = 12;
const MAX_CO_CHANGES: usize = 12;
const MAX_CONTEXT_CANDIDATES: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryIntelligenceSnapshot {
    pub version: u32,
    pub project: String,
    pub focus_path: Option<String>,
    pub indexed_files: usize,
    pub rust_files_indexed: usize,
    pub rust_bytes_indexed: u64,
    pub truncated: bool,
    pub git_history_available: bool,
    pub focus: Option<RepositoryFileIntelligence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryFileIntelligence {
    pub path: String,
    pub language: String,
    pub dependencies: Vec<RepositoryRelation>,
    pub dependents: Vec<RepositoryRelation>,
    pub closest_tests: Vec<RepositoryTestCandidate>,
    pub co_changes: Vec<RepositoryCoChange>,
    pub context_candidates: Vec<RepositoryContextCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRelation {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryTestCandidate {
    pub path: String,
    pub score: u16,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryCoChange {
    pub path: String,
    pub commits_together: usize,
    pub focus_commits_sampled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryContextCandidate {
    pub path: String,
    pub score: u16,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct RustFileFacts {
    modules: Vec<String>,
    uses: Vec<Vec<String>>,
    has_inline_tests: bool,
}

pub fn active_repository_intelligence(
    focus_path: Option<&str>,
) -> RepoDeskResult<RepositoryIntelligenceSnapshot> {
    let project = get_active_project()?;
    build_repository_intelligence(&project.name, &project.path, focus_path)
}

pub fn build_repository_intelligence(
    project_name: &str,
    project_path: &Path,
    focus_path: Option<&str>,
) -> RepoDeskResult<RepositoryIntelligenceSnapshot> {
    let root = project_path.canonicalize()?;
    let workspace = load_code_workspace(project_name, &root)?;
    let all_paths = workspace
        .files
        .iter()
        .filter(|file| !file.blocked)
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();

    let (rust_facts, rust_bytes_indexed, rust_truncated) =
        index_rust_files(&root, &workspace.files, &all_paths);
    let dependencies = build_dependency_map(&rust_facts, &all_paths);
    let dependents = reverse_relations(&dependencies);

    let focus_path = focus_path
        .map(normalize_slashes)
        .filter(|path| all_paths.contains(path));
    let git_history_available = is_git_repository(&root);

    let focus = focus_path.as_ref().map(|path| {
        build_focus_intelligence(
            &root,
            path,
            &workspace.files,
            &rust_facts,
            &dependencies,
            &dependents,
            git_history_available,
        )
    });

    Ok(RepositoryIntelligenceSnapshot {
        version: REPOSITORY_INTELLIGENCE_VERSION,
        project: project_name.to_string(),
        focus_path,
        indexed_files: workspace.files.len(),
        rust_files_indexed: rust_facts.len(),
        rust_bytes_indexed,
        truncated: workspace.truncated || rust_truncated,
        git_history_available,
        focus,
    })
}

fn index_rust_files(
    root: &Path,
    files: &[CodeWorkspaceFile],
    all_paths: &BTreeSet<String>,
) -> (BTreeMap<String, RustFileFacts>, u64, bool) {
    let mut facts = BTreeMap::new();
    let mut indexed_bytes = 0_u64;
    let mut truncated = false;

    for file in files.iter().filter(|file| file.language == "rust" && !file.blocked) {
        if facts.len() >= MAX_RUST_FILES
            || indexed_bytes.saturating_add(file.bytes) > MAX_RUST_INDEX_BYTES
        {
            truncated = true;
            break;
        }
        if file.bytes > MAX_SINGLE_INDEX_FILE_BYTES || !all_paths.contains(&file.path) {
            continue;
        }
        let Ok(source) = fs::read_to_string(root.join(&file.path)) else {
            continue;
        };
        let Ok(parsed) = syn::parse_file(&source) else {
            continue;
        };

        let mut modules = Vec::new();
        let mut uses = Vec::new();
        for item in parsed.items {
            match item {
                Item::Mod(item_mod) if item_mod.content.is_none() => {
                    modules.push(item_mod.ident.to_string());
                }
                Item::Use(item_use) => collect_use_paths(&item_use.tree, Vec::new(), &mut uses),
                _ => {}
            }
        }
        modules.sort();
        modules.dedup();
        uses.sort();
        uses.dedup();
        indexed_bytes = indexed_bytes.saturating_add(file.bytes);
        facts.insert(
            file.path.clone(),
            RustFileFacts {
                modules,
                uses,
                has_inline_tests: source.contains("#[cfg(test)]"),
            },
        );
    }

    (facts, indexed_bytes, truncated)
}

fn collect_use_paths(tree: &UseTree, prefix: Vec<String>, output: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            collect_use_paths(&path.tree, next, output);
        }
        UseTree::Name(name) => {
            let mut value = prefix;
            value.push(name.ident.to_string());
            output.push(value);
        }
        UseTree::Rename(rename) => {
            let mut value = prefix;
            value.push(rename.ident.to_string());
            output.push(value);
        }
        UseTree::Glob(_) => output.push(prefix),
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_paths(item, prefix.clone(), output);
            }
        }
    }
}

fn build_dependency_map(
    facts: &BTreeMap<String, RustFileFacts>,
    all_paths: &BTreeSet<String>,
) -> BTreeMap<String, Vec<RepositoryRelation>> {
    let mut output = BTreeMap::new();

    for (path, facts) in facts {
        let mut relations = BTreeMap::<String, BTreeSet<String>>::new();
        for module in &facts.modules {
            if let Some(target) = resolve_declared_module(path, module, all_paths) {
                relations
                    .entry(target)
                    .or_default()
                    .insert(format!("mod {module}"));
            }
        }
        for use_path in &facts.uses {
            if let Some(target) = resolve_use_path(path, use_path, all_paths)
                && target != *path
            {
                relations
                    .entry(target)
                    .or_default()
                    .insert(format!("use {}", use_path.join("::")));
            }
        }

        output.insert(
            path.clone(),
            relations
                .into_iter()
                .take(MAX_RELATIONS)
                .map(|(path, reasons)| RepositoryRelation {
                    path,
                    reason: reasons.into_iter().collect::<Vec<_>>().join(", "),
                })
                .collect(),
        );
    }

    output
}

fn reverse_relations(
    dependencies: &BTreeMap<String, Vec<RepositoryRelation>>,
) -> BTreeMap<String, Vec<RepositoryRelation>> {
    let mut output = BTreeMap::<String, BTreeMap<String, String>>::new();
    for (source, relations) in dependencies {
        for relation in relations {
            output
                .entry(relation.path.clone())
                .or_default()
                .entry(source.clone())
                .or_insert_with(|| format!("referenced by {source}"));
        }
    }

    output
        .into_iter()
        .map(|(path, sources)| {
            (
                path,
                sources
                    .into_iter()
                    .take(MAX_RELATIONS)
                    .map(|(path, reason)| RepositoryRelation { path, reason })
                    .collect(),
            )
        })
        .collect()
}

fn resolve_declared_module(
    current: &str,
    module: &str,
    all_paths: &BTreeSet<String>,
) -> Option<String> {
    let base = module_directory(current);
    first_existing_module(&base, &[module.to_string()], all_paths)
}

fn resolve_use_path(
    current: &str,
    segments: &[String],
    all_paths: &BTreeSet<String>,
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    let mut index = 0;
    let mut base = source_root(current);
    match segments.first().map(String::as_str) {
        Some("crate") => index = 1,
        Some("self") => {
            base = module_directory(current);
            index = 1;
        }
        Some("super") => {
            base = module_directory(current);
            while segments.get(index).map(String::as_str) == Some("super") {
                base = parent_slash_path(&base);
                index += 1;
            }
        }
        _ => {}
    }

    let remaining = &segments[index..];
    if remaining.is_empty() {
        return None;
    }
    first_existing_module(&base, remaining, all_paths)
}

fn first_existing_module(
    base: &str,
    segments: &[String],
    all_paths: &BTreeSet<String>,
) -> Option<String> {
    for length in (1..=segments.len()).rev() {
        let mut prefix = base.trim_matches('/').to_string();
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(&segments[..length].join("/"));
        for candidate in [format!("{prefix}.rs"), format!("{prefix}/mod.rs")] {
            if all_paths.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn source_root(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().rposition(|part| *part == "src") {
        return parts[..=index].join("/");
    }
    parent_slash_path(path)
}

fn module_directory(path: &str) -> String {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let parent = parent_slash_path(path);
    if matches!(file_name, "lib.rs" | "main.rs" | "mod.rs") {
        return parent;
    }
    let stem = file_name.strip_suffix(".rs").unwrap_or(file_name);
    join_slash(&parent, stem)
}

fn parent_slash_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn join_slash(left: &str, right: &str) -> String {
    if left.is_empty() {
        right.to_string()
    } else {
        format!("{left}/{right}")
    }
}

fn build_focus_intelligence(
    root: &Path,
    focus: &str,
    files: &[CodeWorkspaceFile],
    rust_facts: &BTreeMap<String, RustFileFacts>,
    dependencies: &BTreeMap<String, Vec<RepositoryRelation>>,
    dependents: &BTreeMap<String, Vec<RepositoryRelation>>,
    git_history_available: bool,
) -> RepositoryFileIntelligence {
    let direct_dependencies = dependencies.get(focus).cloned().unwrap_or_default();
    let direct_dependents = dependents.get(focus).cloned().unwrap_or_default();
    let closest_tests = closest_tests(focus, files, rust_facts, &direct_dependents);
    let co_changes = if git_history_available {
        git_co_changes(root, focus)
    } else {
        Vec::new()
    };
    let context_candidates = context_candidates(
        focus,
        &direct_dependencies,
        &direct_dependents,
        &closest_tests,
        &co_changes,
    );

    RepositoryFileIntelligence {
        path: focus.to_string(),
        language: language_for_path(focus).to_string(),
        dependencies: direct_dependencies,
        dependents: direct_dependents,
        closest_tests,
        co_changes,
        context_candidates,
    }
}

fn closest_tests(
    focus: &str,
    files: &[CodeWorkspaceFile],
    rust_facts: &BTreeMap<String, RustFileFacts>,
    dependents: &[RepositoryRelation],
) -> Vec<RepositoryTestCandidate> {
    let focus_stem = Path::new(focus)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let focus_parent = parent_slash_path(focus);
    let dependent_paths = dependents
        .iter()
        .map(|item| item.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<String, (u16, String)>::new();

    if rust_facts
        .get(focus)
        .map(|facts| facts.has_inline_tests)
        .unwrap_or(false)
    {
        candidates.insert(focus.to_string(), (100, "inline #[cfg(test)] module".into()));
    }

    for file in files.iter().filter(|file| !file.blocked && is_test_path(&file.path)) {
        let stem = Path::new(&file.path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let (score, reason) = if dependent_paths.contains(file.path.as_str()) {
            (100, "test file directly depends on focus".to_string())
        } else if !focus_stem.is_empty() && stem.contains(focus_stem) {
            (88, "test filename matches focus module".to_string())
        } else if parent_slash_path(&file.path) == focus_parent {
            (72, "test file is in the same directory".to_string())
        } else if crate_prefix(&file.path) == crate_prefix(focus) {
            (48, "test file is in the same crate/package area".to_string())
        } else {
            continue;
        };

        let entry = candidates.entry(file.path.clone()).or_insert((score, reason.clone()));
        if score > entry.0 {
            *entry = (score, reason);
        }
    }

    let mut values = candidates
        .into_iter()
        .map(|(path, (score, reason))| RepositoryTestCandidate { path, score, reason })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.score.cmp(&left.score).then_with(|| left.path.cmp(&right.path)));
    values.truncate(MAX_TESTS);
    values
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or(&lower);
    lower.starts_with("tests/")
        || lower.contains("/tests/")
        || file_name.contains("_test.")
        || file_name.contains("_tests.")
        || file_name == "test.rs"
        || file_name == "tests.rs"
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".test.tsx")
        || file_name.ends_with(".spec.ts")
        || file_name.ends_with(".spec.tsx")
        || file_name.ends_with(".test.js")
        || file_name.ends_with(".spec.js")
}

fn crate_prefix(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "src" || *part == "tests") {
        return parts[..index].join("/");
    }
    parent_slash_path(path)
}

fn is_git_repository(root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_co_changes(root: &Path, focus: &str) -> Vec<RepositoryCoChange> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--no-merges",
            &format!("-n{MAX_GIT_COMMITS}"),
            "--format=__REPODESK_COMMIT__%H",
            "--name-only",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    parse_git_history(&String::from_utf8_lossy(&output.stdout), focus)
}

fn parse_git_history(raw: &str, focus: &str) -> Vec<RepositoryCoChange> {
    let mut commits = Vec::<BTreeSet<String>>::new();
    let mut current = BTreeSet::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("__REPODESK_COMMIT__") {
            if !current.is_empty() {
                commits.push(std::mem::take(&mut current));
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        current.insert(normalize_slashes(line));
    }
    if !current.is_empty() {
        commits.push(current);
    }

    let mut focus_commits = 0_usize;
    let mut counts = BTreeMap::<String, usize>::new();
    for files in commits {
        if files.len() > MAX_FILES_PER_COMMIT || !files.contains(focus) {
            continue;
        }
        focus_commits += 1;
        for path in files {
            if path != focus {
                *counts.entry(path).or_default() += 1;
            }
        }
    }

    let mut values = counts
        .into_iter()
        .map(|(path, commits_together)| RepositoryCoChange {
            path,
            commits_together,
            focus_commits_sampled: focus_commits,
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .commits_together
            .cmp(&left.commits_together)
            .then_with(|| left.path.cmp(&right.path))
    });
    values.truncate(MAX_CO_CHANGES);
    values
}

fn context_candidates(
    focus: &str,
    dependencies: &[RepositoryRelation],
    dependents: &[RepositoryRelation],
    tests: &[RepositoryTestCandidate],
    co_changes: &[RepositoryCoChange],
) -> Vec<RepositoryContextCandidate> {
    let mut candidates = BTreeMap::<String, (u16, BTreeSet<String>)>::new();
    let mut add = |path: &str, score: u16, reason: String| {
        if path == focus {
            return;
        }
        let entry = candidates.entry(path.to_string()).or_default();
        entry.0 = entry.0.max(score);
        entry.1.insert(reason);
    };

    for relation in dependencies {
        add(&relation.path, 92, format!("dependency: {}", relation.reason));
    }
    for relation in dependents {
        add(&relation.path, 84, relation.reason.clone());
    }
    for test in tests {
        add(&test.path, test.score.max(80), format!("test: {}", test.reason));
    }
    for co_change in co_changes {
        let score = 40_u16.saturating_add((co_change.commits_together.min(6) as u16) * 5);
        add(
            &co_change.path,
            score,
            format!(
                "changed together in {}/{} sampled focus commits",
                co_change.commits_together, co_change.focus_commits_sampled
            ),
        );
    }

    let mut values = candidates
        .into_iter()
        .map(|(path, (score, reasons))| RepositoryContextCandidate {
            path,
            score,
            reasons: reasons.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.score.cmp(&left.score).then_with(|| left.path.cmp(&right.path)));
    values.truncate(MAX_CONTEXT_CANDIDATES);
    values
}

fn normalize_slashes(path: &str) -> String {
    path.trim().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_tree_groups_expand_into_paths() {
        let item: syn::ItemUse = syn::parse_str("use crate::engineering::{knowledge::Store, events};")
            .expect("parse use");
        let mut paths = Vec::new();
        collect_use_paths(&item.tree, Vec::new(), &mut paths);
        assert!(paths.contains(&vec!["crate".into(), "engineering".into(), "knowledge".into(), "Store".into()]));
        assert!(paths.contains(&vec!["crate".into(), "engineering".into(), "events".into()]));
    }

    #[test]
    fn module_resolution_prefers_longest_existing_file() {
        let paths = BTreeSet::from([
            "crates/core/src/engineering.rs".to_string(),
            "crates/core/src/engineering/knowledge.rs".to_string(),
        ]);
        let resolved = resolve_use_path(
            "crates/core/src/lib.rs",
            &["crate".into(), "engineering".into(), "knowledge".into(), "Store".into()],
            &paths,
        );
        assert_eq!(resolved.as_deref(), Some("crates/core/src/engineering/knowledge.rs"));
    }

    #[test]
    fn co_change_history_is_bounded_and_explainable() {
        let history = "__REPODESK_COMMIT__a\nsrc/lib.rs\nsrc/a.rs\n\n__REPODESK_COMMIT__b\nsrc/lib.rs\nsrc/a.rs\nsrc/b.rs\n\n__REPODESK_COMMIT__c\nsrc/other.rs\nsrc/a.rs\n";
        let values = parse_git_history(history, "src/lib.rs");
        assert_eq!(values[0].path, "src/a.rs");
        assert_eq!(values[0].commits_together, 2);
        assert_eq!(values[0].focus_commits_sampled, 2);
    }

    #[test]
    fn test_path_detection_covers_rust_and_typescript() {
        assert!(is_test_path("tests/api.rs"));
        assert!(is_test_path("src/foo_tests.rs"));
        assert!(is_test_path("src/foo.test.tsx"));
        assert!(!is_test_path("src/foo.rs"));
    }
}
