//! Guarded repository browsing and text editing for the RepoDesk Code workspace.
//!
//! This module is deliberately independent from the frontend editor engine. It
//! owns the invariants that must remain true whether the UI is a lightweight
//! editor, Monaco, an LSP client, or a future plugin:
//!
//! - repository-relative paths only;
//! - blocked/sensitive paths never become editable documents;
//! - symlinks are not editable in v0;
//! - files must stay inside the active project after canonicalization;
//! - binary/oversized files are rejected;
//! - saves use an optimistic fingerprint so concurrent external edits are never
//!   overwritten silently;
//! - saves replace validated text atomically instead of truncating the live file;
//! - repository enumeration is bounded and prefers Git's already-optimized
//!   tracked/untracked index instead of recursively walking generated folders.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;
use crate::security::is_blocked_path;

pub const MAX_CODE_WORKSPACE_FILES: usize = 20_000;
pub const MAX_EDITABLE_FILE_BYTES: u64 = 512 * 1024;
const MAX_FALLBACK_DEPTH: usize = 32;
const GIT_STATUS_CACHE_TTL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeWorkspaceSource {
    GitIndex,
    FilesystemFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeWorkspaceFileStatus {
    Clean,
    Modified,
    Added,
    Deleted,
    Untracked,
    Renamed,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceFile {
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub language: String,
    pub bytes: u64,
    pub status: CodeWorkspaceFileStatus,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceSnapshot {
    pub project: String,
    pub source: CodeWorkspaceSource,
    pub files: Vec<CodeWorkspaceFile>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceDocument {
    pub path: String,
    pub content: String,
    pub bytes: u64,
    pub line_count: usize,
    pub language: String,
    pub status: CodeWorkspaceFileStatus,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceSaveInput {
    pub path: String,
    pub content: String,
    pub expected_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceSaveResult {
    pub document: CodeWorkspaceDocument,
    pub previous_fingerprint: String,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardedCodeText {
    pub content: String,
    pub bytes: u64,
}

#[derive(Clone)]
struct GitStatusCacheEntry {
    observed_at: Instant,
    statuses: BTreeMap<String, CodeWorkspaceFileStatus>,
}

static GIT_STATUS_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, GitStatusCacheEntry>>> = OnceLock::new();

pub fn load_active_code_workspace() -> RepoDeskResult<CodeWorkspaceSnapshot> {
    let project = get_active_project()?;
    load_code_workspace(&project.name, &project.path)
}

pub fn read_active_code_document(relative_path: &str) -> RepoDeskResult<CodeWorkspaceDocument> {
    let project = get_active_project()?;
    read_code_document(&project.path, relative_path)
}

pub fn save_active_code_document(
    input: CodeWorkspaceSaveInput,
) -> RepoDeskResult<CodeWorkspaceSaveResult> {
    let project = get_active_project()?;
    save_code_document(&project.path, input)
}

pub fn load_code_workspace(
    project_name: &str,
    project_path: &Path,
) -> RepoDeskResult<CodeWorkspaceSnapshot> {
    let root = project_path.canonicalize()?;
    let statuses = cached_git_statuses(&root);
    let (paths, source) = match git_visible_files(&root) {
        Some(paths) => (paths, CodeWorkspaceSource::GitIndex),
        None => (
            filesystem_visible_files(&root)?,
            CodeWorkspaceSource::FilesystemFallback,
        ),
    };

    let truncated = paths.len() > MAX_CODE_WORKSPACE_FILES;
    let files = paths
        .into_iter()
        .take(MAX_CODE_WORKSPACE_FILES)
        .filter_map(|relative_path| {
            let relative_path = normalize_relative_path(&relative_path).ok()?;
            let full_path = root.join(&relative_path);
            let metadata = fs::metadata(&full_path).ok();
            let path = slash_path(&relative_path);
            let name = relative_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&path)
                .to_string();
            let extension = relative_path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase);
            Some(CodeWorkspaceFile {
                language: language_for_path(&path).to_string(),
                bytes: metadata.map(|value| value.len()).unwrap_or(0),
                status: statuses
                    .get(&path)
                    .copied()
                    .unwrap_or(CodeWorkspaceFileStatus::Clean),
                blocked: is_blocked_path(&path).is_some(),
                path,
                name,
                extension,
            })
        })
        .collect();

    Ok(CodeWorkspaceSnapshot {
        project: project_name.to_string(),
        source,
        files,
        truncated,
    })
}

pub fn read_code_document(
    project_path: &Path,
    relative_path: &str,
) -> RepoDeskResult<CodeWorkspaceDocument> {
    let safe = resolve_existing_editable_file(project_path, relative_path)?;
    document_from_path(project_path, &safe.relative, &safe.canonical)
}

pub fn save_code_document(
    project_path: &Path,
    input: CodeWorkspaceSaveInput,
) -> RepoDeskResult<CodeWorkspaceSaveResult> {
    validate_save_content(&input.content)?;

    let safe = resolve_existing_editable_file(project_path, &input.path)?;
    let current_bytes = fs::read(&safe.canonical)?;
    let current_content = String::from_utf8(current_bytes.clone())
        .map_err(|_| RepoDeskError::Api("Code workspace only edits UTF-8 text files".into()))?;
    if current_content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like file cannot be edited in Code Workspace".into(),
        ));
    }

    let current_fingerprint = fingerprint(&current_bytes);
    if input.expected_fingerprint.trim() != current_fingerprint {
        return Err(stale_editor_error());
    }

    if current_content == input.content {
        let document = document_from_path(project_path, &safe.relative, &safe.canonical)?;
        return Ok(CodeWorkspaceSaveResult {
            document,
            previous_fingerprint: current_fingerprint,
            changed: false,
        });
    }

    atomic_replace_validated_text(
        &safe.canonical,
        input.content.as_bytes(),
        &current_fingerprint,
    )?;

    invalidate_git_status_cache(&project_path.canonicalize()?);
    let document = document_from_path(project_path, &safe.relative, &safe.canonical)?;
    Ok(CodeWorkspaceSaveResult {
        document,
        previous_fingerprint: current_fingerprint,
        changed: true,
    })
}

fn validate_save_content(content: &str) -> RepoDeskResult<()> {
    if content.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(format!(
            "Code editor content exceeds the {} byte limit",
            MAX_EDITABLE_FILE_BYTES
        )));
    }
    if content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like content cannot be saved in Code Workspace".into(),
        ));
    }
    Ok(())
}

fn stale_editor_error() -> RepoDeskError {
    RepoDeskError::Api(
        "File changed outside RepoDesk after it was opened; reload before saving".into(),
    )
}

fn atomic_replace_validated_text(
    path: &Path,
    content: &[u8],
    expected_fingerprint: &str,
) -> RepoDeskResult<()> {
    let parent = path.parent().ok_or_else(|| {
        RepoDeskError::Api("Code workspace file has no writable parent directory".into())
    })?;
    let original_metadata = fs::metadata(path)?;
    if original_metadata.permissions().readonly() {
        return Err(RepoDeskError::Api(
            "Code workspace file is read-only".into(),
        ));
    }

    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .as_file()
        .set_permissions(original_metadata.permissions())?;

    // Re-check immediately before replacement so slow temporary-file I/O does
    // not widen the optimistic-concurrency window and silently overwrite an
    // external edit made after the document was opened.
    let latest_bytes = fs::read(path)?;
    if fingerprint(&latest_bytes) != expected_fingerprint {
        return Err(stale_editor_error());
    }

    temporary
        .persist(path)
        .map_err(|error| RepoDeskError::Io(error.error))?;
    sync_parent_directory(parent)?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> RepoDeskResult<()> {
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> RepoDeskResult<()> {
    Ok(())
}

struct SafeEditableFile {
    relative: PathBuf,
    canonical: PathBuf,
}

fn resolve_existing_editable_file(
    project_path: &Path,
    relative_path: &str,
) -> RepoDeskResult<SafeEditableFile> {
    let root = project_path.canonicalize()?;
    resolve_existing_editable_file_from_root(&root, relative_path)
}

fn resolve_existing_editable_file_from_root(
    canonical_root: &Path,
    relative_path: &str,
) -> RepoDeskResult<SafeEditableFile> {
    let relative = normalize_relative_path(Path::new(relative_path))?;
    let display = slash_path(&relative);
    if let Some(reason) = is_blocked_path(&display) {
        return Err(RepoDeskError::Api(reason));
    }

    let joined = canonical_root.join(&relative);
    let symlink_metadata = fs::symlink_metadata(&joined)?;
    if symlink_metadata.file_type().is_symlink() {
        return Err(RepoDeskError::Api(
            "Symlink editing is intentionally disabled in Code Workspace v0".into(),
        ));
    }

    let canonical = joined.canonicalize()?;
    if !canonical.starts_with(canonical_root) {
        return Err(RepoDeskError::Api("Path escapes active project".into()));
    }
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(RepoDeskError::Api(
            "Code workspace path is not a file".into(),
        ));
    }
    if metadata.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(format!(
            "File exceeds the {} byte editor limit",
            MAX_EDITABLE_FILE_BYTES
        )));
    }

    Ok(SafeEditableFile {
        relative,
        canonical,
    })
}

pub(crate) fn read_guarded_code_text_from_root(
    canonical_root: &Path,
    relative_path: &str,
) -> RepoDeskResult<GuardedCodeText> {
    let safe = resolve_existing_editable_file_from_root(canonical_root, relative_path)?;
    let bytes = fs::read(&safe.canonical)?;
    let content = String::from_utf8(bytes)
        .map_err(|_| RepoDeskError::Api("Code workspace only opens UTF-8 text files".into()))?;
    if content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like file cannot be opened in Code Workspace".into(),
        ));
    }

    Ok(GuardedCodeText {
        bytes: content.len() as u64,
        content,
    })
}

fn document_from_path(
    project_path: &Path,
    relative: &Path,
    canonical: &Path,
) -> RepoDeskResult<CodeWorkspaceDocument> {
    let bytes = fs::read(canonical)?;
    if bytes.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(
            "File is too large for Code Workspace".into(),
        ));
    }
    let content = String::from_utf8(bytes.clone())
        .map_err(|_| RepoDeskError::Api("Code workspace only opens UTF-8 text files".into()))?;
    if content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like file cannot be opened in Code Workspace".into(),
        ));
    }

    let path = slash_path(relative);
    let status = cached_git_statuses(&project_path.canonicalize()?)
        .get(&path)
        .copied()
        .unwrap_or(CodeWorkspaceFileStatus::Clean);

    Ok(CodeWorkspaceDocument {
        path: path.clone(),
        bytes: bytes.len() as u64,
        line_count: if content.is_empty() {
            0
        } else {
            content.lines().count()
        },
        language: language_for_path(&path).to_string(),
        status,
        fingerprint: fingerprint(&bytes),
        content,
    })
}

fn normalize_relative_path(path: &Path) -> RepoDeskResult<PathBuf> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(RepoDeskError::Api(
            "Expected a project-relative path".into(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RepoDeskError::Api("Unsafe project-relative path".into()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(RepoDeskError::Api(
            "Expected a project-relative file path".into(),
        ));
    }
    Ok(normalized)
}

fn git_visible_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-co", "--exclude-standard", "-z"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let mut paths = BTreeSet::new();
    for raw in output.stdout.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let value = String::from_utf8_lossy(raw);
        if let Ok(path) = normalize_relative_path(Path::new(value.as_ref())) {
            paths.insert(path);
        }
    }
    Some(paths.into_iter().collect())
}

fn git_status_cache() -> &'static Mutex<BTreeMap<PathBuf, GitStatusCacheEntry>> {
    GIT_STATUS_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn cached_git_statuses(root: &Path) -> BTreeMap<String, CodeWorkspaceFileStatus> {
    if let Ok(cache) = git_status_cache().lock()
        && let Some(entry) = cache.get(root)
        && entry.observed_at.elapsed() <= GIT_STATUS_CACHE_TTL
    {
        return entry.statuses.clone();
    }

    let statuses = git_statuses_uncached(root);
    if let Ok(mut cache) = git_status_cache().lock() {
        cache.insert(
            root.to_path_buf(),
            GitStatusCacheEntry {
                observed_at: Instant::now(),
                statuses: statuses.clone(),
            },
        );
    }
    statuses
}

fn invalidate_git_status_cache(root: &Path) {
    if let Ok(mut cache) = git_status_cache().lock() {
        cache.remove(root);
    }
}

fn git_statuses_uncached(root: &Path) -> BTreeMap<String, CodeWorkspaceFileStatus> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return BTreeMap::new(),
    };

    parse_porcelain_z(&output.stdout)
}

fn parse_porcelain_z(raw: &[u8]) -> BTreeMap<String, CodeWorkspaceFileStatus> {
    let mut result = BTreeMap::new();
    let records = raw
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;

    while index < records.len() {
        let record = String::from_utf8_lossy(records[index]);
        index += 1;
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        let path = record[3..].replace('\\', "/");
        if status.contains('R') || status.contains('C') {
            // Porcelain -z emits an additional original path after a rename/copy.
            index = index.saturating_add(1).min(records.len());
        }
        result.insert(path, status_from_porcelain(status));
    }
    result
}

fn status_from_porcelain(status: &str) -> CodeWorkspaceFileStatus {
    if status == "??" {
        return CodeWorkspaceFileStatus::Untracked;
    }
    if matches!(status, "DD" | "AU" | "UD" | "UA" | "DU" | "AA" | "UU") {
        return CodeWorkspaceFileStatus::Conflict;
    }
    if status.contains('R') {
        return CodeWorkspaceFileStatus::Renamed;
    }
    if status.contains('A') {
        return CodeWorkspaceFileStatus::Added;
    }
    if status.contains('D') {
        return CodeWorkspaceFileStatus::Deleted;
    }
    CodeWorkspaceFileStatus::Modified
}

fn filesystem_visible_files(root: &Path) -> RepoDeskResult<Vec<PathBuf>> {
    let mut output = BTreeSet::new();
    walk_visible_files(root, root, 0, &mut output)?;
    Ok(output.into_iter().collect())
}

fn walk_visible_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    output: &mut BTreeSet<PathBuf>,
) -> RepoDeskResult<()> {
    if depth > MAX_FALLBACK_DEPTH || output.len() > MAX_CODE_WORKSPACE_FILES {
        return Ok(());
    }

    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if output.len() > MAX_CODE_WORKSPACE_FILES {
            break;
        }
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            if skip_fallback_directory(&entry.file_name().to_string_lossy()) {
                continue;
            }
            walk_visible_files(root, &path, depth + 1, output)?;
        } else if file_type.is_file()
            && let Ok(relative) = path.strip_prefix(root)
            && let Ok(relative) = normalize_relative_path(relative)
        {
            output.insert(relative);
        }
    }
    Ok(())
}

fn skip_fallback_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".turbo"
            | "coverage"
            | ".cache"
    )
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn fingerprint(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn language_for_path(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "json" | "jsonc" => "json",
        "md" | "mdx" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "html" | "htm" => "html",
        "css" | "scss" | "sass" | "less" => "css",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "sh" | "bash" | "zsh" => "shell",
        "ps1" => "powershell",
        "sql" => "sql",
        "xml" => "xml",
        "graphql" | "gql" => "graphql",
        _ => "plaintext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_normalization_rejects_escape() {
        assert!(normalize_relative_path(Path::new("src/lib.rs")).is_ok());
        assert!(normalize_relative_path(Path::new("../secret.txt")).is_err());
        assert!(normalize_relative_path(Path::new("/tmp/file")).is_err());
    }

    #[test]
    fn porcelain_status_is_parsed_without_line_based_path_assumptions() {
        let parsed = parse_porcelain_z(
            b" M src/lib.rs\0?? notes with spaces.md\0R  src/new.rs\0src/old.rs\0",
        );
        assert_eq!(parsed["src/lib.rs"], CodeWorkspaceFileStatus::Modified);
        assert_eq!(
            parsed["notes with spaces.md"],
            CodeWorkspaceFileStatus::Untracked
        );
        assert_eq!(parsed["src/new.rs"], CodeWorkspaceFileStatus::Renamed);
    }

    #[test]
    fn language_mapping_is_stable_for_primary_repo_languages() {
        assert_eq!(language_for_path("src/main.rs"), "rust");
        assert_eq!(language_for_path("src/App.tsx"), "typescript");
        assert_eq!(language_for_path("README.md"), "markdown");
        assert_eq!(language_for_path("unknown.xyz"), "plaintext");
    }

    #[test]
    fn fallback_skips_generated_heavy_directories() {
        assert!(skip_fallback_directory("node_modules"));
        assert!(skip_fallback_directory("target"));
        assert!(!skip_fallback_directory("src"));
    }

    #[test]
    fn save_content_validation_rejects_binary_like_text_before_io() {
        let error = validate_save_content("before\0after").unwrap_err();
        assert!(error.to_string().contains("Binary-like content"));
    }

    #[test]
    fn status_cache_invalidation_removes_only_the_target_repository() {
        let root_a = PathBuf::from("/tmp/repodesk-code-cache-a");
        let root_b = PathBuf::from("/tmp/repodesk-code-cache-b");
        let mut statuses = BTreeMap::new();
        statuses.insert("src/lib.rs".to_string(), CodeWorkspaceFileStatus::Modified);

        if let Ok(mut cache) = git_status_cache().lock() {
            cache.insert(
                root_a.clone(),
                GitStatusCacheEntry {
                    observed_at: Instant::now(),
                    statuses: statuses.clone(),
                },
            );
            cache.insert(
                root_b.clone(),
                GitStatusCacheEntry {
                    observed_at: Instant::now(),
                    statuses,
                },
            );
        }

        invalidate_git_status_cache(&root_a);
        let cache = git_status_cache().lock().expect("status cache lock");
        assert!(!cache.contains_key(&root_a));
        assert!(cache.contains_key(&root_b));
        drop(cache);
        invalidate_git_status_cache(&root_b);
    }
}
