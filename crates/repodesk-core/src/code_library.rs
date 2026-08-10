//! Short-lived, read-only access to language-server definition documents.
//!
//! The frontend never receives or submits an arbitrary absolute path. A live
//! language-server response is exchanged for an opaque handle after the
//! canonical file has passed the dependency-root and text-file boundary.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::code_workspace::{MAX_EDITABLE_FILE_BYTES, language_for_path};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::security::is_blocked_path;

const HANDLE_TTL_MINUTES: i64 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLibraryRoot {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CodeLibraryGrantRequest {
    pub project: String,
    pub server_id: String,
    pub project_root: PathBuf,
    pub uri: String,
    pub allowed_roots: Vec<CodeLibraryRoot>,
    pub issued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLibraryDefinition {
    pub handle: String,
    pub display_path: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLibraryDocument {
    pub handle: String,
    pub display_path: String,
    pub content: String,
    pub bytes: u64,
    pub line_count: usize,
    pub language: String,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
struct LibraryGrant {
    project: String,
    server_id: String,
    canonical_path: PathBuf,
    canonical_root: PathBuf,
    display_path: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct CodeLibraryRegistry {
    grants: Mutex<HashMap<String, LibraryGrant>>,
    sequence: AtomicU64,
}

impl CodeLibraryRegistry {
    pub fn issue_definition(
        &self,
        request: CodeLibraryGrantRequest,
    ) -> RepoDeskResult<CodeLibraryDefinition> {
        if request.project.trim().is_empty() || request.server_id.trim().is_empty() {
            return Err(RepoDeskError::Api(
                "Library access requires an active project and language server".into(),
            ));
        }

        let candidate = path_from_file_uri(&request.uri)?;
        let canonical = candidate.canonicalize().map_err(|_| {
            RepoDeskError::Api("Language definition file is no longer available".into())
        })?;
        let project_root = request.project_root.canonicalize()?;
        let (root, relative) = approved_root_for(&canonical, &request.allowed_roots)?;
        if root.path.canonicalize()? == project_root {
            return Err(RepoDeskError::Api(
                "Repository definitions must use the normal workspace reader".into(),
            ));
        }

        let display_path = format!("{}/{}", root.label, slash_path(&relative));
        validate_library_file(&canonical, &display_path)?;

        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let handle = opaque_handle(
            &request.project,
            &request.server_id,
            &canonical,
            request.issued_at,
            sequence,
        );
        let language = language_for_path(&display_path).to_string();
        self.grants
            .lock()
            .map_err(|_| RepoDeskError::Api("Library handle registry is unavailable".into()))?
            .insert(
                handle.clone(),
                LibraryGrant {
                    project: request.project,
                    server_id: request.server_id,
                    canonical_path: canonical,
                    canonical_root: root.path.canonicalize()?,
                    display_path: display_path.clone(),
                    expires_at: request.issued_at + Duration::minutes(HANDLE_TTL_MINUTES),
                },
            );

        Ok(CodeLibraryDefinition {
            handle,
            display_path,
            language,
        })
    }

    pub fn read(&self, project: &str, handle: &str) -> RepoDeskResult<CodeLibraryDocument> {
        self.read_at(project, handle, Utc::now())
    }

    pub fn read_at(
        &self,
        project: &str,
        handle: &str,
        now: DateTime<Utc>,
    ) -> RepoDeskResult<CodeLibraryDocument> {
        let grant = self
            .grants
            .lock()
            .map_err(|_| RepoDeskError::Api("Library handle registry is unavailable".into()))?
            .get(handle)
            .cloned()
            .ok_or_else(|| RepoDeskError::Api("Library handle is invalid or expired".into()))?;
        if grant.project != project || now > grant.expires_at {
            return Err(RepoDeskError::Api(
                "Library handle is invalid for the active project or has expired".into(),
            ));
        }
        if grant.server_id.trim().is_empty() {
            return Err(RepoDeskError::Api(
                "Library handle has no server owner".into(),
            ));
        }

        let canonical = grant.canonical_path.canonicalize().map_err(|_| {
            RepoDeskError::Api("Library definition file is no longer available".into())
        })?;
        if !canonical.starts_with(&grant.canonical_root) {
            return Err(RepoDeskError::Api(
                "Library definition escaped its approved dependency root".into(),
            ));
        }
        validate_library_file(&canonical, &grant.display_path)?;
        let bytes = fs::read(&canonical)?;
        let content = String::from_utf8(bytes.clone())
            .map_err(|_| RepoDeskError::Api("Library documents must be UTF-8 text".into()))?;
        if content.contains('\0') {
            return Err(RepoDeskError::Api(
                "Binary-like library documents cannot be opened".into(),
            ));
        }

        Ok(CodeLibraryDocument {
            handle: handle.to_string(),
            display_path: grant.display_path.clone(),
            bytes: bytes.len() as u64,
            line_count: if content.is_empty() {
                0
            } else {
                content.lines().count()
            },
            language: language_for_path(&grant.display_path).to_string(),
            content,
            read_only: true,
        })
    }

    pub fn clear_project(&self, project: &str) {
        if let Ok(mut grants) = self.grants.lock() {
            grants.retain(|_, grant| grant.project != project);
        }
    }
}

pub fn reject_library_save(_handle: &str) -> RepoDeskResult<()> {
    Err(RepoDeskError::Api(
        "Library documents are read-only and cannot be saved".into(),
    ))
}

pub fn default_code_library_roots(project_root: &Path) -> Vec<CodeLibraryRoot> {
    let mut roots = vec![CodeLibraryRoot {
        label: "node_modules".into(),
        path: project_root.join("node_modules"),
    }];
    let home = dirs::home_dir();
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|path| path.join(".cargo")));
    if let Some(path) = cargo_home {
        roots.push(CodeLibraryRoot {
            label: "cargo-registry".into(),
            path: path.join("registry/src"),
        });
    }
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|path| path.join(".rustup")));
    if let Some(path) = rustup_home {
        roots.push(CodeLibraryRoot {
            label: "rust-toolchain".into(),
            path: path.join("toolchains"),
        });
    }
    if let Ok(paths) = RepoDeskPaths::resolve() {
        roots.push(CodeLibraryRoot {
            label: "repodesk-tools".into(),
            path: paths.home.join("tools/language-servers"),
        });
    }
    roots
}

fn approved_root_for(
    canonical: &Path,
    roots: &[CodeLibraryRoot],
) -> RepoDeskResult<(CodeLibraryRoot, PathBuf)> {
    for root in roots {
        if root.label.trim().is_empty()
            || root.label.contains('/')
            || root.label.contains('\\')
            || root.label.contains("..")
        {
            continue;
        }
        let Ok(canonical_root) = root.path.canonicalize() else {
            continue;
        };
        let Ok(relative) = canonical.strip_prefix(&canonical_root) else {
            continue;
        };
        return Ok((root.clone(), relative.to_path_buf()));
    }
    Err(RepoDeskError::Api(
        "Language definition is outside approved dependency roots".into(),
    ))
}

fn validate_library_file(path: &Path, display_path: &str) -> RepoDeskResult<()> {
    if let Some(reason) = is_blocked_path(display_path) {
        return Err(RepoDeskError::Api(reason));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !matches!(
        extension.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "md"
            | "txt"
    ) {
        return Err(RepoDeskError::Api(
            "Language definition is not a supported text source file".into(),
        ));
    }
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(RepoDeskError::Api(
            "Language definition path is not a file".into(),
        ));
    }
    if metadata.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(RepoDeskError::Api(format!(
            "Library definition exceeds the {} byte editor limit",
            MAX_EDITABLE_FILE_BYTES
        )));
    }
    Ok(())
}

fn path_from_file_uri(uri: &str) -> RepoDeskResult<PathBuf> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| RepoDeskError::Api("Only file: library definitions are supported".into()))?;
    let decoded = percent_decode(raw)?;
    let normalized =
        if cfg!(windows) && decoded.starts_with('/') && decoded.as_bytes().get(2) == Some(&b':') {
            decoded[1..].to_string()
        } else {
            decoded
        };
    let path = PathBuf::from(normalized);
    if !path.is_absolute() {
        return Err(RepoDeskError::Api(
            "Library definition URI must contain an absolute path".into(),
        ));
    }
    Ok(path)
}

fn percent_decode(value: &str) -> RepoDeskResult<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(RepoDeskError::Api("Invalid file URI encoding".into()));
            }
            let pair = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| RepoDeskError::Api("Invalid file URI encoding".into()))?;
            output.push(
                u8::from_str_radix(pair, 16)
                    .map_err(|_| RepoDeskError::Api("Invalid file URI encoding".into()))?,
            );
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| RepoDeskError::Api("File URI is not UTF-8".into()))
}

fn opaque_handle(
    project: &str,
    server_id: &str,
    path: &Path,
    issued_at: DateTime<Utc>,
    sequence: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project.as_bytes());
    hasher.update([0]);
    hasher.update(server_id.as_bytes());
    hasher.update([0]);
    hasher.update(path.as_os_str().to_string_lossy().as_bytes());
    hasher.update(
        issued_at
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .to_le_bytes(),
    );
    hasher.update(sequence.to_le_bytes());
    format!("lib_{}", hex::encode(hasher.finalize()))
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
