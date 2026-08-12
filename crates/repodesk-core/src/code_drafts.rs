//! Local recovery store for unsaved Code Workspace editor buffers.
//!
//! Drafts are deliberately separate from the capability Recovery Engine and
//! from repository state. Raw editor content lives under RepoDesk's local cache,
//! keyed by hashes of project identity and guarded repository-relative path.
//! A draft is always bound to the disk fingerprint it was edited from so a
//! changed file can never be silently overwritten during recovery.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::code_workspace::{MAX_EDITABLE_FILE_BYTES, guard_code_relative_path};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

const CODE_DRAFT_VERSION: u32 = 1;
const CODE_DRAFTS_DIR: &str = "code-drafts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeDraftSaveInput {
    pub path: String,
    pub content: String,
    pub base_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeDraftLoadInput {
    pub path: String,
    pub current_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeDraftRecord {
    pub path: String,
    pub content: String,
    pub base_fingerprint: String,
    pub content_fingerprint: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeDraftRecoveryState {
    Safe,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeDraftRecovery {
    pub draft: CodeDraftRecord,
    pub state: CodeDraftRecoveryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedCodeDraft {
    version: u32,
    project_identity: String,
    draft: CodeDraftRecord,
}

pub fn save_active_code_draft(input: CodeDraftSaveInput) -> RepoDeskResult<CodeDraftRecord> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;
    save_code_draft(
        &paths.cache_dir,
        &project.name,
        &project.path,
        input,
        Utc::now(),
    )
}

pub fn load_active_code_draft(
    input: CodeDraftLoadInput,
) -> RepoDeskResult<Option<CodeDraftRecovery>> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;
    load_code_draft(&paths.cache_dir, &project.name, &project.path, input)
}

pub fn delete_active_code_draft(path: &str) -> RepoDeskResult<bool> {
    let project = get_active_project()?;
    let paths = RepoDeskPaths::resolve()?;
    delete_code_draft(&paths.cache_dir, &project.name, &project.path, path)
}

fn save_code_draft(
    cache_dir: &Path,
    project_name: &str,
    project_root: &Path,
    input: CodeDraftSaveInput,
    now: DateTime<Utc>,
) -> RepoDeskResult<CodeDraftRecord> {
    validate_content(&input.content)?;
    validate_fingerprint("Draft base", &input.base_fingerprint)?;
    let path = guarded_display_path(&input.path)?;
    let project_identity = project_identity(project_name, project_root)?;
    let file_path = draft_file_path(cache_dir, &project_identity, &path);
    let parent = file_path
        .parent()
        .ok_or_else(|| RepoDeskError::Api("Draft cache path has no parent".into()))?;
    fs::create_dir_all(parent)?;

    let record = CodeDraftRecord {
        path,
        content_fingerprint: fingerprint(input.content.as_bytes()),
        content: input.content,
        base_fingerprint: input.base_fingerprint,
        updated_at: now,
    };
    let persisted = PersistedCodeDraft {
        version: CODE_DRAFT_VERSION,
        project_identity,
        draft: record.clone(),
    };

    let mut temporary = NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut temporary, &persisted)?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&file_path)
        .map_err(|error| RepoDeskError::Io(error.error))?;
    sync_parent_directory(parent)?;

    Ok(record)
}

fn load_code_draft(
    cache_dir: &Path,
    project_name: &str,
    project_root: &Path,
    input: CodeDraftLoadInput,
) -> RepoDeskResult<Option<CodeDraftRecovery>> {
    validate_fingerprint("Current file", &input.current_fingerprint)?;
    let path = guarded_display_path(&input.path)?;
    let project_identity = project_identity(project_name, project_root)?;
    let file_path = draft_file_path(cache_dir, &project_identity, &path);

    let contents = match fs::read_to_string(&file_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    let persisted: PersistedCodeDraft = match serde_json::from_str(&contents) {
        Ok(persisted) => persisted,
        Err(_) => {
            let _ = fs::remove_file(&file_path);
            return Ok(None);
        }
    };
    if !valid_persisted_draft(&persisted, &project_identity, &path) {
        let _ = fs::remove_file(&file_path);
        return Ok(None);
    }

    if persisted.draft.content_fingerprint == input.current_fingerprint {
        let _ = fs::remove_file(&file_path);
        return Ok(None);
    }

    let state = if persisted.draft.base_fingerprint == input.current_fingerprint {
        CodeDraftRecoveryState::Safe
    } else {
        CodeDraftRecoveryState::Conflict
    };

    Ok(Some(CodeDraftRecovery {
        draft: persisted.draft,
        state,
    }))
}

fn delete_code_draft(
    cache_dir: &Path,
    project_name: &str,
    project_root: &Path,
    relative_path: &str,
) -> RepoDeskResult<bool> {
    let path = guarded_display_path(relative_path)?;
    let project_identity = project_identity(project_name, project_root)?;
    let file_path = draft_file_path(cache_dir, &project_identity, &path);
    match fs::remove_file(file_path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn valid_persisted_draft(
    persisted: &PersistedCodeDraft,
    project_identity: &str,
    path: &str,
) -> bool {
    persisted.version == CODE_DRAFT_VERSION
        && persisted.project_identity == project_identity
        && persisted.draft.path == path
        && validate_content(&persisted.draft.content).is_ok()
        && validate_fingerprint("Draft base", &persisted.draft.base_fingerprint).is_ok()
        && validate_fingerprint("Draft content", &persisted.draft.content_fingerprint).is_ok()
        && fingerprint(persisted.draft.content.as_bytes()) == persisted.draft.content_fingerprint
}

fn guarded_display_path(path: &str) -> RepoDeskResult<String> {
    let relative = guard_code_relative_path(path)?;
    Ok(relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/"))
}

fn project_identity(project_name: &str, project_root: &Path) -> RepoDeskResult<String> {
    let canonical = project_root.canonicalize()?;
    let mut hasher = Sha256::new();
    hasher.update(project_name.as_bytes());
    hasher.update([0]);
    hasher.update(canonical.to_string_lossy().as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

fn draft_file_path(cache_dir: &Path, project_identity: &str, relative_path: &str) -> PathBuf {
    let path_hash = fingerprint(relative_path.as_bytes());
    cache_dir
        .join(CODE_DRAFTS_DIR)
        .join(project_identity)
        .join(format!("{path_hash}.json"))
}

fn validate_content(content: &str) -> RepoDeskResult<()> {
    if content.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(format!(
            "Editor draft exceeds the {MAX_EDITABLE_FILE_BYTES} byte limit"
        )));
    }
    if content.contains('\0') {
        return Err(RepoDeskError::Api(
            "Binary-like content cannot be persisted as an editor draft".into(),
        ));
    }
    Ok(())
}

fn validate_fingerprint(label: &str, value: &str) -> RepoDeskResult<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RepoDeskError::Api(format!(
            "{label} fingerprint must be a SHA-256 hex digest"
        )));
    }
    Ok(())
}

fn fingerprint(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sha(value: &str) -> String {
        fingerprint(value.as_bytes())
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempdir().unwrap();
        let project = root.path().join("project");
        let cache = root.path().join("cache");
        fs::create_dir_all(&project).unwrap();
        (root, project, cache)
    }

    #[test]
    fn round_trip_marks_matching_base_as_safe() {
        let (_root, project, cache) = setup();
        let base = sha("disk");
        save_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftSaveInput {
                path: "src/main.rs".into(),
                content: "draft".into(),
                base_fingerprint: base.clone(),
            },
            Utc::now(),
        )
        .unwrap();

        let recovery = load_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftLoadInput {
                path: "src/main.rs".into(),
                current_fingerprint: base,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(recovery.state, CodeDraftRecoveryState::Safe);
        assert_eq!(recovery.draft.content, "draft");
    }

    #[test]
    fn changed_disk_marks_draft_as_conflict() {
        let (_root, project, cache) = setup();
        save_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftSaveInput {
                path: "src/main.rs".into(),
                content: "draft".into(),
                base_fingerprint: sha("old disk"),
            },
            Utc::now(),
        )
        .unwrap();

        let recovery = load_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftLoadInput {
                path: "src/main.rs".into(),
                current_fingerprint: sha("new disk"),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(recovery.state, CodeDraftRecoveryState::Conflict);
    }

    #[test]
    fn already_persisted_draft_is_cleaned_up() {
        let (_root, project, cache) = setup();
        let record = save_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftSaveInput {
                path: "src/main.rs".into(),
                content: "draft".into(),
                base_fingerprint: sha("old disk"),
            },
            Utc::now(),
        )
        .unwrap();

        let recovery = load_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftLoadInput {
                path: "src/main.rs".into(),
                current_fingerprint: record.content_fingerprint,
            },
        )
        .unwrap();
        assert!(recovery.is_none());
        assert!(!delete_code_draft(&cache, "RepoDesk", &project, "src/main.rs").unwrap());
    }

    #[test]
    fn corrupted_draft_is_tolerated_and_removed() {
        let (_root, project, cache) = setup();
        let identity = project_identity("RepoDesk", &project).unwrap();
        let path = draft_file_path(&cache, &identity, "src/main.rs");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ definitely not json").unwrap();

        let recovery = load_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftLoadInput {
                path: "src/main.rs".into(),
                current_fingerprint: sha("disk"),
            },
        )
        .unwrap();
        assert!(recovery.is_none());
        assert!(!path.exists());
    }

    #[test]
    fn drafts_are_hashed_outside_repository_tree() {
        let (_root, project, cache) = setup();
        let record = save_code_draft(
            &cache,
            "RepoDesk",
            &project,
            CodeDraftSaveInput {
                path: "src/notes file.rs".into(),
                content: "draft".into(),
                base_fingerprint: sha("disk"),
            },
            Utc::now(),
        )
        .unwrap();
        assert_eq!(record.path, "src/notes file.rs");

        let identity = project_identity("RepoDesk", &project).unwrap();
        let stored = draft_file_path(&cache, &identity, &record.path);
        assert!(stored.starts_with(cache.join(CODE_DRAFTS_DIR)));
        assert!(!stored.starts_with(&project));
        assert!(!stored.to_string_lossy().contains("notes file"));
    }

    #[test]
    fn blocked_and_traversal_paths_are_rejected() {
        let (_root, project, cache) = setup();
        for path in ["../escape.rs", ".git/config", ".env"] {
            let result = save_code_draft(
                &cache,
                "RepoDesk",
                &project,
                CodeDraftSaveInput {
                    path: path.into(),
                    content: "draft".into(),
                    base_fingerprint: sha("disk"),
                },
                Utc::now(),
            );
            assert!(result.is_err(), "{path} should be blocked");
        }
    }
}
