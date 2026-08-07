//! Explicit file evidence for RepoDesk context packs.
//!
//! v0 is deliberately conservative: only repository files explicitly written
//! as backtick paths in the Work Item `## Scope` section are candidates. The
//! selector never expands directories or scans the repository implicitly.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::engineering::events::{EngineeringEvent, EngineeringEventKind, read_events};
use crate::errors::RepoDeskResult;
use crate::tokens::estimate_text;

pub const CONTEXT_MANIFEST_FILE: &str = "context-manifest.json";
pub const CONTEXT_MANIFEST_VERSION: u32 = 1;
pub const DEFAULT_MAX_SCOPED_FILES: usize = 12;
pub const DEFAULT_SCOPED_FILE_BUDGET_CHARS: usize = 24_000;
pub const DEFAULT_PER_FILE_BUDGET_CHARS: usize = 8_000;
pub const DEFAULT_MAX_SCOPED_FILE_BYTES: u64 = 512_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFileReason {
    TaskScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFileStatus {
    Included,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFileExclusionReason {
    InvalidPath,
    Ignored,
    BlockedBySecurity,
    Missing,
    NotFile,
    OutsideProject,
    ReadError,
    FileLimit,
    TooLarge,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFileEntry {
    pub path: String,
    pub reason: ContextFileReason,
    pub status: ContextFileStatus,
    pub exclusion_reason: Option<ContextFileExclusionReason>,
    pub candidate_tokens: Option<usize>,
    pub included_tokens: Option<usize>,
    pub trimmed: bool,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub version: u32,
    pub project: String,
    pub work_item_id: String,
    pub generated_at: DateTime<Utc>,
    pub included_files: usize,
    pub excluded_files: usize,
    pub included_file_tokens: usize,
    pub entries: Vec<ContextFileEntry>,
}

#[derive(Debug, Clone)]
pub struct ContextFileSelection {
    /// Eligible scoped-file content before per-file and aggregate context
    /// trimming. Used only to keep Context Compactness semantics honest.
    pub candidate_rendered: String,
    /// Final bounded scoped-file content injected into `context.md`.
    pub rendered: String,
    pub manifest: ContextManifest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContextFileEvidenceReport {
    /// ContextBuilt events that contain concrete included/excluded path lists.
    pub evidenced_context_builds: usize,
    /// ChangeSets compared against the context that immediately preceded them.
    pub compared_changesets: usize,
    /// Evidence for the latest changeset after the latest evidenced context
    /// build. A newer context build clears this until another changeset exists.
    pub latest: Option<ContextChangeCoverage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextChangeCoverage {
    pub included_files: Vec<String>,
    pub excluded_files: Vec<String>,
    pub changed_files: Vec<String>,
    pub changed_files_present_in_context: Vec<String>,
    pub changed_files_missing_from_context: Vec<String>,
    /// Changed files present in context / changed files. This is evidence about
    /// the observed changeset, not a claim that every included file was useful.
    pub change_coverage: Option<f64>,
}

pub fn select_task_scope_files(
    project: &str,
    work_item_id: &str,
    project_root: &Path,
    task_markdown: &str,
    ignore_rules: &[String],
) -> ContextFileSelection {
    let scoped_paths = parse_explicit_scope_paths(task_markdown);
    let canonical_root = fs::canonicalize(project_root).ok();
    let mut entries = Vec::with_capacity(scoped_paths.len());
    let mut candidate_rendered = String::new();
    let mut rendered = String::new();
    let mut included_files = 0usize;
    let mut excluded_files = 0usize;
    let mut included_file_tokens = 0usize;
    let mut remaining_chars = DEFAULT_SCOPED_FILE_BUDGET_CHARS;

    for path in scoped_paths {
        let mut entry = excluded_entry(path.clone());

        if included_files >= DEFAULT_MAX_SCOPED_FILES {
            exclude(
                &mut entry,
                ContextFileExclusionReason::FileLimit,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let Some(relative) = validate_relative_repo_path(&path) else {
            exclude(
                &mut entry,
                ContextFileExclusionReason::InvalidPath,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        };

        if crate::security::is_blocked_path(&path).is_some() {
            exclude(
                &mut entry,
                ContextFileExclusionReason::BlockedBySecurity,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        if is_ignored(&relative, ignore_rules) {
            exclude(
                &mut entry,
                ContextFileExclusionReason::Ignored,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let joined = project_root.join(&relative);
        if !joined.exists() {
            exclude(
                &mut entry,
                ContextFileExclusionReason::Missing,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }
        if !joined.is_file() {
            exclude(
                &mut entry,
                ContextFileExclusionReason::NotFile,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let Ok(canonical_file) = fs::canonicalize(&joined) else {
            exclude(
                &mut entry,
                ContextFileExclusionReason::ReadError,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        };
        if canonical_root
            .as_ref()
            .is_none_or(|root| !canonical_file.starts_with(root))
        {
            exclude(
                &mut entry,
                ContextFileExclusionReason::OutsideProject,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let Ok(metadata) = fs::metadata(&canonical_file) else {
            exclude(
                &mut entry,
                ContextFileExclusionReason::ReadError,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        };
        if metadata.len() > DEFAULT_MAX_SCOPED_FILE_BYTES {
            exclude(
                &mut entry,
                ContextFileExclusionReason::TooLarge,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let Ok(candidate) = fs::read_to_string(&canonical_file) else {
            exclude(
                &mut entry,
                ContextFileExclusionReason::ReadError,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        };
        entry.candidate_tokens = Some(estimate_text(&candidate).estimated_tokens);
        append_file_section(&mut candidate_rendered, &path, &candidate);

        if remaining_chars == 0 {
            exclude(
                &mut entry,
                ContextFileExclusionReason::BudgetExceeded,
                &mut excluded_files,
            );
            entries.push(entry);
            continue;
        }

        let file_budget = DEFAULT_PER_FILE_BUDGET_CHARS.min(remaining_chars);
        let included = trim_file_for_context(&candidate, file_budget);
        let consumed_chars = candidate.chars().count().min(file_budget);
        remaining_chars = remaining_chars.saturating_sub(consumed_chars);
        let included_tokens = estimate_text(&included).estimated_tokens;

        entry.status = ContextFileStatus::Included;
        entry.included_tokens = Some(included_tokens);
        entry.trimmed = included != candidate;
        entry.fingerprint = Some(sha256_hex(&included));
        included_files += 1;
        included_file_tokens = included_file_tokens.saturating_add(included_tokens);

        append_file_section(&mut rendered, &path, &included);
        entries.push(entry);
    }

    if candidate_rendered.is_empty() {
        candidate_rendered
            .push_str("No explicit repository files were eligible from task scope.\n");
    }
    if rendered.is_empty() {
        rendered.push_str("No explicit repository files were included from task scope.\n");
    }

    ContextFileSelection {
        candidate_rendered,
        rendered,
        manifest: ContextManifest {
            version: CONTEXT_MANIFEST_VERSION,
            project: project.to_string(),
            work_item_id: work_item_id.to_string(),
            generated_at: Utc::now(),
            included_files,
            excluded_files,
            included_file_tokens,
            entries,
        },
    }
}

pub fn write_context_manifest(
    run_dir: &Path,
    manifest: &ContextManifest,
) -> RepoDeskResult<PathBuf> {
    let path = run_dir.join(CONTEXT_MANIFEST_FILE);
    let content = serde_json::to_string_pretty(manifest)?;
    fs::write(&path, content)?;
    Ok(path)
}

pub fn read_context_manifest(run_dir: &Path) -> RepoDeskResult<Option<ContextManifest>> {
    let path = run_dir.join(CONTEXT_MANIFEST_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

pub fn load_context_file_evidence(run_dir: &Path) -> RepoDeskResult<ContextFileEvidenceReport> {
    let events = read_events(run_dir)?;
    Ok(derive_context_file_evidence(&events))
}

pub fn derive_context_file_evidence(events: &[EngineeringEvent]) -> ContextFileEvidenceReport {
    let mut report = ContextFileEvidenceReport::default();
    let mut active_context: Option<(Vec<String>, Vec<String>)> = None;

    for event in events {
        match event.kind {
            EngineeringEventKind::ContextBuilt => {
                report.latest = None;
                let Some(included) = attribute_string_vec(event, "included_files") else {
                    active_context = None;
                    continue;
                };
                let Some(excluded) = attribute_string_vec(event, "excluded_files") else {
                    active_context = None;
                    continue;
                };
                report.evidenced_context_builds += 1;
                active_context = Some((dedup_paths(included), dedup_paths(excluded)));
            }
            EngineeringEventKind::ChangeSetCreated => {
                let Some((included, excluded)) = active_context.as_ref() else {
                    report.latest = None;
                    continue;
                };
                let Some(changed) = attribute_string_vec(event, "files") else {
                    report.latest = None;
                    continue;
                };
                let changed = dedup_paths(changed);
                let included_set = included.iter().cloned().collect::<BTreeSet<_>>();
                let present = changed
                    .iter()
                    .filter(|path| included_set.contains(*path))
                    .cloned()
                    .collect::<Vec<_>>();
                let missing = changed
                    .iter()
                    .filter(|path| !included_set.contains(*path))
                    .cloned()
                    .collect::<Vec<_>>();

                report.compared_changesets += 1;
                report.latest = Some(ContextChangeCoverage {
                    included_files: included.clone(),
                    excluded_files: excluded.clone(),
                    change_coverage: ratio(present.len(), changed.len()),
                    changed_files: changed,
                    changed_files_present_in_context: present,
                    changed_files_missing_from_context: missing,
                });
            }
            _ => {}
        }
    }

    report
}

fn excluded_entry(path: String) -> ContextFileEntry {
    ContextFileEntry {
        path,
        reason: ContextFileReason::TaskScope,
        status: ContextFileStatus::Excluded,
        exclusion_reason: None,
        candidate_tokens: None,
        included_tokens: None,
        trimmed: false,
        fingerprint: None,
    }
}

fn exclude(
    entry: &mut ContextFileEntry,
    reason: ContextFileExclusionReason,
    excluded_files: &mut usize,
) {
    entry.exclusion_reason = Some(reason);
    *excluded_files = excluded_files.saturating_add(1);
}

fn parse_explicit_scope_paths(task_markdown: &str) -> Vec<String> {
    let mut in_scope = false;
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();

    for line in task_markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            in_scope = trimmed.eq_ignore_ascii_case("## Scope");
            continue;
        }
        if !in_scope || !(trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            continue;
        }

        let Some(start) = trimmed.find('`') else {
            continue;
        };
        let after_start = &trimmed[start + 1..];
        let Some(end) = after_start.find('`') else {
            continue;
        };
        let value = after_start[..end].trim();
        if !value.is_empty() && seen.insert(value.to_string()) {
            paths.push(value.to_string());
        }
    }

    paths
}

fn validate_relative_repo_path(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() || value.trim().is_empty() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

fn is_ignored(path: &Path, rules: &[String]) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");

    rules.iter().any(|raw| {
        let rule = raw.trim().trim_matches('/');
        if rule.is_empty() {
            return false;
        }
        if let Some(suffix) = rule.strip_prefix("*.") {
            return normalized.ends_with(&format!(".{suffix}"));
        }

        normalized == rule
            || normalized.starts_with(&format!("{rule}/"))
            || normalized.split('/').any(|component| component == rule)
    })
}

fn trim_file_for_context(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let trimmed = value.chars().take(max_chars).collect::<String>();
    format!("{trimmed}\n\n[RepoDesk: scoped file trimmed for context budget]")
}

fn append_file_section(output: &mut String, path: &str, content: &str) {
    output.push_str(&format!("### `{path}`\n\n```text\n{content}\n```\n\n"));
}

fn attribute_string_vec(event: &EngineeringEvent, key: &str) -> Option<Vec<String>> {
    event
        .attributes
        .get(key)
        .cloned()
        .and_then(|value: Value| serde_json::from_value(value).ok())
}

fn dedup_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    use crate::engineering::domain::{ChangeSetId, WorkItemId};

    fn base_event(kind: EngineeringEventKind) -> EngineeringEvent {
        EngineeringEvent::new("repodesk", WorkItemId::try_new("task-1").unwrap(), kind)
    }

    #[test]
    fn scope_parser_only_accepts_explicit_backtick_paths() {
        let markdown = "# Task\n\n## Scope\n\n- `src/lib.rs`\n- change the parser\n- `tests/a.rs` because related\n\n## Notes\n- `ignored.rs`\n";
        assert_eq!(
            parse_explicit_scope_paths(markdown),
            vec!["src/lib.rs".to_string(), "tests/a.rs".to_string()]
        );
    }

    #[test]
    fn selector_records_included_and_excluded_evidence() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }\n",
        )
        .unwrap();
        fs::write(root.path().join("debug.log"), "do not include").unwrap();
        fs::write(root.path().join(".env"), "TOKEN=secret").unwrap();

        let markdown =
            "## Scope\n- `src/lib.rs`\n- `missing.rs`\n- `debug.log`\n- `.env`\n- `../escape.rs`\n";
        let selection = select_task_scope_files(
            "repodesk",
            "task-1",
            root.path(),
            markdown,
            &["*.log".to_string()],
        );

        assert_eq!(selection.manifest.included_files, 1);
        assert_eq!(selection.manifest.excluded_files, 4);
        assert!(selection.rendered.contains("src/lib.rs"));
        assert!(selection.rendered.contains("pub fn answer"));
        assert!(!selection.candidate_rendered.contains("TOKEN=secret"));
        assert!(selection.manifest.entries.iter().any(|entry| {
            entry.path == "debug.log"
                && entry.exclusion_reason == Some(ContextFileExclusionReason::Ignored)
        }));
        assert!(selection.manifest.entries.iter().any(|entry| {
            entry.path == ".env"
                && entry.exclusion_reason == Some(ContextFileExclusionReason::BlockedBySecurity)
        }));
        assert!(selection.manifest.entries.iter().any(|entry| {
            entry.path == "../escape.rs"
                && entry.exclusion_reason == Some(ContextFileExclusionReason::InvalidPath)
        }));
    }

    #[test]
    fn manifest_round_trips() {
        let root = tempdir().unwrap();
        let selection = select_task_scope_files("p", "w", root.path(), "## Scope\n", &[]);
        write_context_manifest(root.path(), &selection.manifest).unwrap();
        let loaded = read_context_manifest(root.path()).unwrap().unwrap();
        assert_eq!(loaded, selection.manifest);
    }

    #[test]
    fn derives_changed_file_coverage_from_preceding_context() {
        let context = base_event(EngineeringEventKind::ContextBuilt)
            .with_attribute("included_files", json!(["src/a.rs", "src/b.rs"]))
            .with_attribute("excluded_files", json!(["src/c.rs"]));
        let changeset = base_event(EngineeringEventKind::ChangeSetCreated)
            .with_changeset(ChangeSetId::try_new("changes-1").unwrap())
            .with_attribute("files", json!(["src/a.rs", "src/c.rs"]));

        let report = derive_context_file_evidence(&[context, changeset]);
        let latest = report.latest.unwrap();

        assert_eq!(report.evidenced_context_builds, 1);
        assert_eq!(report.compared_changesets, 1);
        assert_eq!(latest.changed_files_present_in_context, vec!["src/a.rs"]);
        assert_eq!(latest.changed_files_missing_from_context, vec!["src/c.rs"]);
        assert_eq!(latest.change_coverage, Some(0.5));
    }

    #[test]
    fn newer_context_invalidates_stale_changeset_coverage() {
        let context = base_event(EngineeringEventKind::ContextBuilt)
            .with_attribute("included_files", json!(["src/a.rs"]))
            .with_attribute("excluded_files", json!([]));
        let changeset = base_event(EngineeringEventKind::ChangeSetCreated)
            .with_changeset(ChangeSetId::try_new("changes-1").unwrap())
            .with_attribute("files", json!(["src/a.rs"]));
        let rebuilt = base_event(EngineeringEventKind::ContextBuilt)
            .with_attribute("included_files", json!(["src/b.rs"]))
            .with_attribute("excluded_files", json!([]));

        let report = derive_context_file_evidence(&[context, changeset, rebuilt]);
        assert_eq!(report.compared_changesets, 1);
        assert_eq!(report.latest, None);
    }
}
