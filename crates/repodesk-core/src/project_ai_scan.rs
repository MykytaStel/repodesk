//! Scan and import project-local AI instruction artifacts.
//!
//! This is intentionally narrow: it reads only well-known AI assistant config
//! files (`AGENTS.md`, `CLAUDE.md`, Cursor/Claude/Copilot instruction dirs),
//! applies path and size guards, redacts previews, and refuses to import files
//! that look like they contain secrets.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::memory::model::{NewMemoryInput, source, status};
use crate::projects::ProjectConfig;

const MAX_SCAN_FILE_BYTES: u64 = 96_000;
const MAX_SCAN_PREVIEW_CHARS: usize = 4_000;
const MAX_MEMORY_CHARS: usize = 7_000;
const MAX_DISCOVERED_FILES: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiScanReport {
    pub generated_at: String,
    pub project: String,
    pub project_path: String,
    pub files: Vec<ProjectAiFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiFile {
    pub relative_path: String,
    pub kind: String,
    pub label: String,
    pub size_bytes: u64,
    /// Redacted, bounded preview safe to render in the UI.
    pub preview: String,
    pub truncated: bool,
    pub blocked: bool,
    pub importable: bool,
    pub secret_findings: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiImportResult {
    pub imported: Vec<ProjectAiImportedFile>,
    pub skipped: Vec<ProjectAiSkippedFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiImportedFile {
    pub relative_path: String,
    pub memory_id: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiSkippedFile {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
struct CandidateRoot {
    relative_path: &'static str,
    kind: &'static str,
    label: &'static str,
    recursive: bool,
}

const CANDIDATES: &[CandidateRoot] = &[
    CandidateRoot {
        relative_path: "AGENTS.md",
        kind: "agents",
        label: "AGENTS.md",
        recursive: false,
    },
    CandidateRoot {
        relative_path: "CLAUDE.md",
        kind: "claude",
        label: "Claude instructions",
        recursive: false,
    },
    CandidateRoot {
        relative_path: ".cursorrules",
        kind: "cursor",
        label: "Cursor rules",
        recursive: false,
    },
    CandidateRoot {
        relative_path: "copilot-instructions.md",
        kind: "copilot",
        label: "Copilot instructions",
        recursive: false,
    },
    CandidateRoot {
        relative_path: ".claude",
        kind: "claude",
        label: "Claude project config",
        recursive: true,
    },
    CandidateRoot {
        relative_path: ".cursor",
        kind: "cursor",
        label: "Cursor project config",
        recursive: true,
    },
    CandidateRoot {
        relative_path: ".github/copilot-instructions.md",
        kind: "copilot",
        label: "Copilot instructions",
        recursive: false,
    },
    CandidateRoot {
        relative_path: ".github/instructions",
        kind: "copilot",
        label: "Copilot instruction files",
        recursive: true,
    },
    CandidateRoot {
        relative_path: ".copilot",
        kind: "copilot",
        label: "Copilot project config",
        recursive: true,
    },
];

/// Scan the active RepoDesk project for AI instruction files.
pub fn scan_active_project_ai() -> RepoDeskResult<ProjectAiScanReport> {
    let project = crate::projects::get_active_project()?;
    scan_project_ai(&project)
}

/// Import selected scan results into Memory Brain entries. An empty `paths`
/// list means "import every clean/importable file from the latest scan".
pub fn import_active_project_ai(paths: Vec<String>) -> RepoDeskResult<ProjectAiImportResult> {
    let project = crate::projects::get_active_project()?;
    import_project_ai(&project, paths)
}

pub fn scan_project_ai(project: &ProjectConfig) -> RepoDeskResult<ProjectAiScanReport> {
    let root = canonical_root(&project.path)?;
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();

    for candidate in CANDIDATES {
        let Some(path) = safe_join(&root, candidate.relative_path) else {
            warnings.push(format!(
                "Skipped {} because it is outside the project root.",
                candidate.relative_path
            ));
            continue;
        };

        if !path.exists() {
            continue;
        }

        if candidate.recursive && path.is_dir() {
            walk_candidate_dir(
                &root,
                &path,
                candidate,
                &mut seen,
                &mut files,
                &mut warnings,
            );
        } else if path.is_file() {
            push_candidate_file(
                &root,
                &path,
                candidate.kind,
                candidate.label,
                &mut seen,
                &mut files,
                &mut warnings,
            );
        }
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(ProjectAiScanReport {
        generated_at: Utc::now().to_rfc3339(),
        project: project.name.clone(),
        project_path: project.path.display().to_string(),
        files,
        warnings,
    })
}

pub fn import_project_ai(
    project: &ProjectConfig,
    paths: Vec<String>,
) -> RepoDeskResult<ProjectAiImportResult> {
    let report = scan_project_ai(project)?;
    let requested = normalize_requested_paths(paths)?;
    let import_all = requested.is_empty();

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut seen_report_paths = BTreeSet::new();

    for file in report.files {
        seen_report_paths.insert(file.relative_path.clone());
        if !import_all && !requested.contains(&file.relative_path) {
            continue;
        }
        if !file.importable {
            let reason = skip_reason(&file);
            skipped.push(ProjectAiSkippedFile {
                relative_path: file.relative_path,
                reason,
            });
            continue;
        }

        let content = read_clean_import_content(&project.path, &file.relative_path)?;
        let findings = crate::security::scan_text_for_secrets(&content);
        if !findings.is_empty() {
            skipped.push(ProjectAiSkippedFile {
                relative_path: file.relative_path,
                reason: format!("secret scan blocked import: {}", findings.join(", ")),
            });
            continue;
        }

        let (body, truncated) = truncate_chars(&content, MAX_MEMORY_CHARS);
        let memory_content = format!(
            "Imported project AI instructions from `{}` ({}).\n\n{}{}",
            file.relative_path,
            file.label,
            body.trim(),
            if truncated {
                "\n\n[RepoDesk truncated this imported instruction file.]"
            } else {
                ""
            }
        );
        let tags = vec![
            "project-ai".to_string(),
            file.kind.clone(),
            file.relative_path.clone(),
        ];
        let input = NewMemoryInput {
            project: project.name.clone(),
            content: memory_content,
            category: "constraint".to_string(),
            tags,
            source: source::SYSTEM.to_string(),
            agent: "project_ai_scan".to_string(),
            task_id: String::new(),
            salience: 0.85,
            confidence: 0.95,
            status: status::ACTIVE.to_string(),
            supersedes_id: None,
        };
        let entry = crate::memory::store::add_entry(input)?;
        imported.push(ProjectAiImportedFile {
            relative_path: file.relative_path,
            memory_id: entry.id,
            truncated,
        });
    }

    if !import_all {
        for path in requested {
            if !seen_report_paths.contains(&path) {
                skipped.push(ProjectAiSkippedFile {
                    relative_path: path,
                    reason: "not found in project AI scan".to_string(),
                });
            }
        }
    }

    Ok(ProjectAiImportResult {
        imported,
        skipped,
        warnings: report.warnings,
    })
}

fn canonical_root(path: &Path) -> RepoDeskResult<PathBuf> {
    path.canonicalize().map_err(RepoDeskError::Io)
}

fn safe_join(root: &Path, relative_path: &str) -> Option<PathBuf> {
    let rel = Path::new(relative_path);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(root.join(rel))
}

fn normalize_requested_paths(paths: Vec<String>) -> RepoDeskResult<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for path in paths {
        let normalized = path.trim().replace('\\', "/");
        if normalized.is_empty() {
            continue;
        }
        if safe_join(Path::new("/"), &normalized).is_none() {
            return Err(RepoDeskError::RoutingFailed {
                detail: format!("invalid project AI import path '{normalized}'"),
            });
        }
        out.insert(normalized);
    }
    Ok(out)
}

fn walk_candidate_dir(
    root: &Path,
    dir: &Path,
    candidate: &CandidateRoot,
    seen: &mut BTreeSet<String>,
    files: &mut Vec<ProjectAiFile>,
    warnings: &mut Vec<String>,
) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if files.len() >= MAX_DISCOVERED_FILES {
            warnings.push(format!(
                "Project AI scan stopped after {MAX_DISCOVERED_FILES} files."
            ));
            return;
        }

        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(format!("Skipped {}: {error}", display_rel(root, &current)));
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                warnings.push(format!("Skipped symlink {}", display_rel(root, &path)));
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file() && is_supported_instruction_file(&path) {
                push_candidate_file(
                    root,
                    &path,
                    candidate.kind,
                    candidate.label,
                    seen,
                    files,
                    warnings,
                );
            }
        }
    }
}

fn push_candidate_file(
    root: &Path,
    path: &Path,
    kind: &str,
    label: &str,
    seen: &mut BTreeSet<String>,
    files: &mut Vec<ProjectAiFile>,
    warnings: &mut Vec<String>,
) {
    let relative_path = display_rel(root, path);
    if !seen.insert(relative_path.clone()) {
        return;
    }

    match scan_one_file(root, path, kind, label) {
        Ok(file) => files.push(file),
        Err(error) => warnings.push(format!("Skipped {relative_path}: {error}")),
    }
}

fn scan_one_file(
    root: &Path,
    path: &Path,
    kind: &str,
    label: &str,
) -> RepoDeskResult<ProjectAiFile> {
    let relative_path = display_rel(root, path);
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(blocked_file(
            relative_path,
            kind,
            label,
            0,
            "symlink skipped",
        ));
    }

    let canonical = path.canonicalize()?;
    if !canonical.starts_with(root) {
        return Ok(blocked_file(
            relative_path,
            kind,
            label,
            meta.len(),
            "path resolves outside the project root",
        ));
    }
    if meta.len() > MAX_SCAN_FILE_BYTES {
        return Ok(blocked_file(
            relative_path,
            kind,
            label,
            meta.len(),
            "file is too large for project AI import",
        ));
    }

    let raw = fs::read_to_string(path).map_err(|error| RepoDeskError::RoutingFailed {
        detail: format!("not a readable UTF-8 instruction file: {error}"),
    })?;
    let findings = crate::security::scan_text_for_secrets(&raw);
    let (redacted, redacted_kinds) = crate::security::redact_secrets(&raw);
    let mut secret_findings = findings;
    for kind in redacted_kinds {
        if !secret_findings.contains(&kind) {
            secret_findings.push(kind);
        }
    }
    let (preview, truncated) = truncate_chars(&redacted, MAX_SCAN_PREVIEW_CHARS);
    let blocked = !secret_findings.is_empty();

    Ok(ProjectAiFile {
        relative_path,
        kind: kind.to_string(),
        label: label.to_string(),
        size_bytes: meta.len(),
        preview,
        truncated,
        blocked,
        importable: !blocked,
        secret_findings,
        warnings: Vec::new(),
    })
}

fn blocked_file(
    relative_path: String,
    kind: &str,
    label: &str,
    size_bytes: u64,
    reason: &str,
) -> ProjectAiFile {
    ProjectAiFile {
        relative_path,
        kind: kind.to_string(),
        label: label.to_string(),
        size_bytes,
        preview: String::new(),
        truncated: false,
        blocked: true,
        importable: false,
        secret_findings: Vec::new(),
        warnings: vec![reason.to_string()],
    }
}

fn is_supported_instruction_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "agents.md" | "claude.md" | ".cursorrules" | "copilot-instructions.md"
    ) || name.ends_with(".instructions.md")
    {
        return true;
    }

    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "mdc" | "txt" | "json" | "yml" | "yaml")
    )
}

fn read_clean_import_content(project_path: &Path, relative_path: &str) -> RepoDeskResult<String> {
    let root = canonical_root(project_path)?;
    let Some(path) = safe_join(&root, relative_path) else {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("invalid project AI import path '{relative_path}'"),
        });
    };
    let scanned = scan_one_file(&root, &path, "project-ai", "Project AI instructions")?;
    if !scanned.importable {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("project AI import blocked for '{relative_path}'"),
        });
    }
    fs::read_to_string(path).map_err(|error| RepoDeskError::RoutingFailed {
        detail: format!("could not read '{relative_path}' for import: {error}"),
    })
}

fn skip_reason(file: &ProjectAiFile) -> String {
    if !file.secret_findings.is_empty() {
        return format!(
            "secret scan blocked import: {}",
            file.secret_findings.join(", ")
        );
    }
    file.warnings
        .first()
        .cloned()
        .unwrap_or_else(|| "not importable".to_string())
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let truncated = value.chars().count() > max_chars;
    let text = chars.by_ref().take(max_chars).collect::<String>();
    (text, truncated)
}

fn display_rel(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::memory::store;
    use crate::memory::test_support::with_temp_home;
    use crate::projects::{AddProjectInput, add_project, use_project};

    fn activate_project(repo: &TempDir) -> ProjectConfig {
        let config = add_project(AddProjectInput {
            name: "demo".to_string(),
            path: repo.path().to_path_buf(),
            project_type: "rust".to_string(),
            main_language: Some("rust".to_string()),
        })
        .unwrap();
        use_project("demo").unwrap();
        config
    }

    #[test]
    fn scan_detects_known_ai_instruction_files() {
        with_temp_home(|| {
            let repo = TempDir::new().unwrap();
            fs::create_dir_all(repo.path().join(".cursor/rules")).unwrap();
            fs::create_dir_all(repo.path().join(".github/instructions")).unwrap();
            fs::write(repo.path().join("AGENTS.md"), "Use cargo test.").unwrap();
            fs::write(repo.path().join("CLAUDE.md"), "Be concise.").unwrap();
            fs::write(
                repo.path().join(".cursor/rules/repo.mdc"),
                "Use app tokens.",
            )
            .unwrap();
            fs::write(
                repo.path().join(".github/copilot-instructions.md"),
                "Prefer small PRs.",
            )
            .unwrap();
            let project = activate_project(&repo);

            let report = scan_project_ai(&project).unwrap();
            let paths = report
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>();
            assert!(paths.contains(&"AGENTS.md"));
            assert!(paths.contains(&"CLAUDE.md"));
            assert!(paths.contains(&".cursor/rules/repo.mdc"));
            assert!(paths.contains(&".github/copilot-instructions.md"));
        });
    }

    #[test]
    fn secret_files_are_redacted_and_not_imported() {
        with_temp_home(|| {
            let repo = TempDir::new().unwrap();
            fs::write(
                repo.path().join("AGENTS.md"),
                "api_key=abcdefghijklmnopqrstuvwxyz",
            )
            .unwrap();
            let project = activate_project(&repo);

            let report = scan_project_ai(&project).unwrap();
            let file = report
                .files
                .iter()
                .find(|f| f.relative_path == "AGENTS.md")
                .unwrap();
            assert!(file.blocked);
            assert!(file.preview.contains("[REDACTED:"));

            let result = import_project_ai(&project, Vec::new()).unwrap();
            assert!(result.imported.is_empty());
            assert_eq!(result.skipped.len(), 1);
            assert!(store::list_memory("demo").unwrap().is_empty());
        });
    }

    #[test]
    fn import_appends_clean_instruction_file_to_memory() {
        with_temp_home(|| {
            let repo = TempDir::new().unwrap();
            fs::write(repo.path().join("AGENTS.md"), "Always run cargo test.").unwrap();
            let project = activate_project(&repo);

            let result = import_project_ai(&project, vec!["AGENTS.md".to_string()]).unwrap();
            assert_eq!(result.imported.len(), 1);
            assert!(result.skipped.is_empty());

            let entries = store::list_memory("demo").unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].category, "constraint");
            assert_eq!(entries[0].source, source::SYSTEM);
            assert!(entries[0].content.contains("Always run cargo test."));
        });
    }
}
