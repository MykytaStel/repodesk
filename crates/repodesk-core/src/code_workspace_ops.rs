//! Guarded filesystem mutations for the Code workspace.
//!
//! These operations intentionally expose a narrow repository-scoped API instead
//! of a generic filesystem bridge. Every path is project-relative, `.git` and
//! secret-like paths are blocked, symlink traversal is rejected, and destructive
//! file operations can be bound to the editor fingerprint the user reviewed.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::code_workspace::{language_for_path, MAX_EDITABLE_FILE_BYTES};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;
use crate::security::is_blocked_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceCreateFileInput {
    pub path: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceRenameInput {
    pub path: String,
    pub new_path: String,
    #[serde(default)]
    pub expected_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceDeleteInput {
    pub path: String,
    #[serde(default)]
    pub expected_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWorkspaceMutationResult {
    pub path: String,
    pub previous_path: Option<String>,
    pub kind: String,
}

pub fn create_active_code_file(
    input: CodeWorkspaceCreateFileInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let project = get_active_project()?;
    create_code_file(&project.path, input)
}

pub fn create_active_code_directory(path: &str) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let project = get_active_project()?;
    create_code_directory(&project.path, path)
}

pub fn rename_active_code_path(
    input: CodeWorkspaceRenameInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let project = get_active_project()?;
    rename_code_path(&project.path, input)
}

pub fn delete_active_code_path(
    input: CodeWorkspaceDeleteInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let project = get_active_project()?;
    delete_code_path(&project.path, input)
}

pub fn create_code_file(
    project_path: &Path,
    input: CodeWorkspaceCreateFileInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    validate_text_content(&input.content)?;
    let target = resolve_new_path(project_path, &input.path)?;
    if target.path.exists() {
        return Err(RepoDeskError::Api("Code workspace path already exists".into()));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target.path)?;
    file.write_all(input.content.as_bytes())?;
    file.sync_all()?;
    sync_directory(&target.parent)?;

    Ok(CodeWorkspaceMutationResult {
        path: slash_path(&target.relative),
        previous_path: None,
        kind: "file_created".into(),
    })
}

pub fn create_code_directory(
    project_path: &Path,
    path: &str,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let target = resolve_new_path(project_path, path)?;
    if target.path.exists() {
        return Err(RepoDeskError::Api("Code workspace path already exists".into()));
    }

    fs::create_dir(&target.path)?;
    sync_directory(&target.parent)?;

    Ok(CodeWorkspaceMutationResult {
        path: slash_path(&target.relative),
        previous_path: None,
        kind: "directory_created".into(),
    })
}

pub fn rename_code_path(
    project_path: &Path,
    input: CodeWorkspaceRenameInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let source = resolve_existing_path(project_path, &input.path)?;
    let destination = resolve_new_path(project_path, &input.new_path)?;
    if destination.path.exists() {
        return Err(RepoDeskError::Api("Rename destination already exists".into()));
    }

    if source.metadata.is_file() {
        validate_expected_fingerprint(&source.path, input.expected_fingerprint.as_deref())?;
    } else if input.expected_fingerprint.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        return Err(RepoDeskError::Api(
            "Directory rename does not accept a file fingerprint".into(),
        ));
    }

    fs::rename(&source.path, &destination.path)?;
    sync_directory(&source.parent)?;
    if source.parent != destination.parent {
        sync_directory(&destination.parent)?;
    }

    Ok(CodeWorkspaceMutationResult {
        path: slash_path(&destination.relative),
        previous_path: Some(slash_path(&source.relative)),
        kind: if source.metadata.is_dir() {
            "directory_renamed".into()
        } else {
            "file_renamed".into()
        },
    })
}

pub fn delete_code_path(
    project_path: &Path,
    input: CodeWorkspaceDeleteInput,
) -> RepoDeskResult<CodeWorkspaceMutationResult> {
    let target = resolve_existing_path(project_path, &input.path)?;

    let kind = if target.metadata.is_file() {
        validate_expected_fingerprint(&target.path, input.expected_fingerprint.as_deref())?;
        fs::remove_file(&target.path)?;
        "file_deleted"
    } else if target.metadata.is_dir() {
        if input.expected_fingerprint.as_deref().is_some_and(|value| !value.trim().is_empty()) {
            return Err(RepoDeskError::Api(
                "Directory delete does not accept a file fingerprint".into(),
            ));
        }
        // Intentionally non-recursive. A directory with repository content must
        // never disappear because one UI action expanded to `remove_dir_all`.
        fs::remove_dir(&target.path)?;
        "directory_deleted"
    } else {
        return Err(RepoDeskError::Api(
            "Code workspace path is neither a regular file nor directory".into(),
        ));
    };
    sync_directory(&target.parent)?;

    Ok(CodeWorkspaceMutationResult {
        path: slash_path(&target.relative),
        previous_path: None,
        kind: kind.into(),
    })
}

struct ExistingPath {
    relative: PathBuf,
    path: PathBuf,
    parent: PathBuf,
    metadata: fs::Metadata,
}

struct NewPath {
    relative: PathBuf,
    path: PathBuf,
    parent: PathBuf,
}

fn resolve_existing_path(project_path: &Path, value: &str) -> RepoDeskResult<ExistingPath> {
    let root = project_path.canonicalize()?;
    let relative = validate_relative_path(value)?;
    reject_symlink_components(&root, &relative, true)?;
    let joined = root.join(&relative);
    let canonical = joined.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(RepoDeskError::Api("Path escapes active project".into()));
    }
    let metadata = fs::metadata(&canonical)?;
    let parent = canonical
        .parent()
        .ok_or_else(|| RepoDeskError::Api("Code workspace path has no parent".into()))?
        .to_path_buf();

    Ok(ExistingPath {
        relative,
        path: canonical,
        parent,
        metadata,
    })
}

fn resolve_new_path(project_path: &Path, value: &str) -> RepoDeskResult<NewPath> {
    let root = project_path.canonicalize()?;
    let relative = validate_relative_path(value)?;
    let parent_relative = relative
        .parent()
        .ok_or_else(|| RepoDeskError::Api("Code workspace path has no parent".into()))?;

    reject_symlink_components(&root, parent_relative, true)?;
    let parent = root.join(parent_relative).canonicalize()?;
    if !parent.starts_with(&root) {
        return Err(RepoDeskError::Api("Path escapes active project".into()));
    }
    let metadata = fs::metadata(&parent)?;
    if !metadata.is_dir() {
        return Err(RepoDeskError::Api(
            "Code workspace parent path is not a directory".into(),
        ));
    }

    Ok(NewPath {
        path: root.join(&relative),
        relative,
        parent,
    })
}

fn validate_relative_path(value: &str) -> RepoDeskResult<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RepoDeskError::Api("Expected a project-relative path".into()));
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(RepoDeskError::Api(
            "Code workspace path contains unsupported characters".into(),
        ));
    }

    let raw = Path::new(trimmed);
    if raw.is_absolute() {
        return Err(RepoDeskError::Api("Expected a project-relative path".into()));
    }

    let mut normalized = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RepoDeskError::Api("Unsafe project-relative path".into()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(RepoDeskError::Api("Expected a project-relative path".into()));
    }

    let display = slash_path(&normalized);
    if display == ".git" || display.starts_with(".git/") {
        return Err(RepoDeskError::Api(
            "The repository .git directory is never writable from Code Workspace".into(),
        ));
    }
    if let Some(reason) = is_blocked_path(&display) {
        return Err(RepoDeskError::Api(reason));
    }

    Ok(normalized)
}

fn reject_symlink_components(
    root: &Path,
    relative: &Path,
    allow_missing_final: bool,
) -> RepoDeskResult<()> {
    let mut current = root.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(value) = component else {
            continue;
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoDeskError::Api(
                    "Symlink traversal is intentionally disabled in Code Workspace".into(),
                ));
            }
            Ok(_) => {}
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && allow_missing_final
                    && index + 1 == components.len() => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn validate_text_content(content: &str) -> RepoDeskResult<()> {
    if content.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(format!(
            "Code editor content exceeds the {} byte limit",
            MAX_EDITABLE_FILE_BYTES
        )));
    }
    if content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like content cannot be created in Code Workspace".into(),
        ));
    }
    Ok(())
}

fn validate_expected_fingerprint(path: &Path, expected: Option<&str>) -> RepoDeskResult<()> {
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RepoDeskError::Api(
                "Destructive file operation requires the fingerprint of the reviewed editor document"
                    .into(),
            )
        })?;
    let bytes = fs::read(path)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected {
        return Err(RepoDeskError::Api(
            "File changed outside RepoDesk; reload it before rename or delete".into(),
        ));
    }
    Ok(())
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

#[cfg(unix)]
fn sync_directory(directory: &Path) -> RepoDeskResult<()> {
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> RepoDeskResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fingerprint(content: &[u8]) -> String {
        hex::encode(Sha256::digest(content))
    }

    #[test]
    fn traversal_and_git_metadata_are_rejected() {
        assert!(validate_relative_path("../outside.txt").is_err());
        assert!(validate_relative_path(".git/config").is_err());
        assert!(validate_relative_path("config/.env.local").is_err());
    }

    #[test]
    fn create_rename_and_delete_file_keep_repository_boundary() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("src")).unwrap();

        let created = create_code_file(
            root.path(),
            CodeWorkspaceCreateFileInput {
                path: "src/new.rs".into(),
                content: "fn main() {}\n".into(),
            },
        )
        .unwrap();
        assert_eq!(created.path, "src/new.rs");

        let content = fs::read(root.path().join("src/new.rs")).unwrap();
        let renamed = rename_code_path(
            root.path(),
            CodeWorkspaceRenameInput {
                path: "src/new.rs".into(),
                new_path: "src/renamed.rs".into(),
                expected_fingerprint: Some(fingerprint(&content)),
            },
        )
        .unwrap();
        assert_eq!(renamed.path, "src/renamed.rs");
        assert!(!root.path().join("src/new.rs").exists());

        let renamed_content = fs::read(root.path().join("src/renamed.rs")).unwrap();
        delete_code_path(
            root.path(),
            CodeWorkspaceDeleteInput {
                path: "src/renamed.rs".into(),
                expected_fingerprint: Some(fingerprint(&renamed_content)),
            },
        )
        .unwrap();
        assert!(!root.path().join("src/renamed.rs").exists());
    }

    #[test]
    fn destructive_file_operation_fails_closed_after_external_change() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("file.txt"), "before").unwrap();
        let expected = fingerprint(b"before");
        fs::write(root.path().join("file.txt"), "after").unwrap();

        let result = delete_code_path(
            root.path(),
            CodeWorkspaceDeleteInput {
                path: "file.txt".into(),
                expected_fingerprint: Some(expected),
            },
        );
        assert!(result.is_err());
        assert!(root.path().join("file.txt").exists());
    }

    #[test]
    fn delete_directory_is_intentionally_non_recursive() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/lib.rs"), "pub fn lib() {}\n").unwrap();

        let result = delete_code_path(
            root.path(),
            CodeWorkspaceDeleteInput {
                path: "src".into(),
                expected_fingerprint: None,
            },
        );
        assert!(result.is_err());
        assert!(root.path().join("src/lib.rs").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_component_is_rejected_for_new_files() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();

        let result = create_code_file(
            root.path(),
            CodeWorkspaceCreateFileInput {
                path: "linked/escape.txt".into(),
                content: String::new(),
            },
        );
        assert!(result.is_err());
        assert!(!outside.path().join("escape.txt").exists());
    }

    #[test]
    fn language_mapping_remains_shared_with_editor_documents() {
        assert_eq!(language_for_path("src/new.rs"), "rust");
    }
}
