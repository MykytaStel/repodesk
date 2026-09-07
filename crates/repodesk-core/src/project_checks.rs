use std::path::{Component, Path};

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};

pub const DEFAULT_CHECK_TIMEOUT_SECS: u64 = 120;
const MAX_CHECK_TIMEOUT_SECS: u64 = 3_600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCheck {
    pub id: String,
    pub title: String,
    pub command: String,
    pub kind: String,
    pub required: bool,
    pub relevant_paths: Vec<String>,
    pub timeout_secs: u64,
}

impl ProjectCheck {
    pub fn new(id: &str, title: &str, command: &str) -> Self {
        let command = command.trim().to_string();
        Self {
            id: if id.trim().is_empty() {
                stable_check_id(&command)
            } else {
                id.trim().to_string()
            },
            title: if title.trim().is_empty() {
                command.clone()
            } else {
                title.trim().to_string()
            },
            kind: infer_check_kind(&command),
            command,
            required: false,
            relevant_paths: Vec::new(),
            timeout_secs: DEFAULT_CHECK_TIMEOUT_SECS,
        }
    }

    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_relevant_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.relevant_paths = paths
            .into_iter()
            .map(Into::into)
            .map(|path| normalize_relevant_path(&path))
            .collect();
        self
    }

    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    pub fn validate(&self) -> RepoDeskResult<()> {
        validate_check_id(&self.id)?;
        if self.command.trim().is_empty() {
            return Err(RepoDeskError::InvalidCheckCommand(
                "command cannot be empty".to_string(),
            ));
        }
        crate::checks::is_allowed_check_command(&self.command)
            .map_err(RepoDeskError::InvalidCheckCommand)?;
        if !(1..=MAX_CHECK_TIMEOUT_SECS).contains(&self.timeout_secs) {
            return Err(RepoDeskError::InvalidCheckCommand(format!(
                "timeout_secs must be between 1 and {MAX_CHECK_TIMEOUT_SECS}"
            )));
        }
        for path in &self.relevant_paths {
            validate_relevant_path(path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawProjectCheck {
    Legacy(String),
    Structured(ProjectCheckInput),
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectCheckInput {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    command: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    relevant_paths: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_CHECK_TIMEOUT_SECS
}

pub fn deserialize_project_checks<'de, D>(deserializer: D) -> Result<Vec<ProjectCheck>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<RawProjectCheck>::deserialize(deserializer)?;
    raw.into_iter()
        .map(normalize_raw_check)
        .collect::<RepoDeskResult<Vec<_>>>()
        .map_err(|error| D::Error::custom(error.to_string()))
}

pub fn serialize_project_checks<S>(
    checks: &Vec<ProjectCheck>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    checks.serialize(serializer)
}

pub fn normalize_legacy_commands(commands: Vec<String>) -> RepoDeskResult<Vec<ProjectCheck>> {
    commands
        .into_iter()
        .map(|command| {
            let check = ProjectCheck::new("", "", &command);
            check.validate()?;
            Ok(check)
        })
        .collect()
}

fn normalize_raw_check(raw: RawProjectCheck) -> RepoDeskResult<ProjectCheck> {
    let check = match raw {
        RawProjectCheck::Legacy(command) => ProjectCheck::new("", "", &command),
        RawProjectCheck::Structured(input) => {
            let command = input.command.trim().to_string();
            let mut check = ProjectCheck::new(
                input.id.as_deref().unwrap_or_default(),
                input.title.as_deref().unwrap_or_default(),
                &command,
            );
            if let Some(kind) = input.kind {
                check.kind = kind.trim().to_ascii_lowercase();
            }
            check.required = input.required;
            check.relevant_paths = input
                .relevant_paths
                .iter()
                .map(|path| normalize_relevant_path(path))
                .collect();
            check.timeout_secs = input.timeout_secs;
            check
        }
    };
    check.validate()?;
    Ok(check)
}

pub fn stable_check_id(command: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.trim().as_bytes());
    format!("check-{}", &hex::encode(hasher.finalize())[..16])
}

pub fn infer_check_kind(command: &str) -> String {
    let lower = command.to_ascii_lowercase();
    if lower.contains("test") || lower.contains("vitest") || lower.contains("jest") {
        "test".to_string()
    } else if lower.contains("lint") || lower.contains("eslint") || lower.contains("clippy") {
        "lint".to_string()
    } else if lower.contains("typecheck") || lower.contains("tsc") {
        "typecheck".to_string()
    } else if lower.contains("snyk")
        || lower.contains("trivy")
        || lower.contains("checkmarx")
        || lower.contains("sonar")
    {
        "security".to_string()
    } else {
        "check".to_string()
    }
}

pub fn default_checks_for_project_type(project_type: &str) -> Vec<ProjectCheck> {
    let commands = match project_type {
        "rust" | "rust-cli" | "rust-desktop" | "rust-tauri" => vec![
            ("format", "Format check", "cargo fmt --all -- --check"),
            (
                "lint",
                "Clippy warnings",
                "cargo clippy --all-targets --all-features -- -D warnings",
            ),
            ("tests", "All Rust tests", "cargo test --all"),
        ],
        "node" | "react" | "react-native" => vec![
            ("typecheck", "TypeScript check", "pnpm typecheck"),
            ("tests", "Project tests", "pnpm test"),
        ],
        "python" => vec![("tests", "Python tests", "python -m pytest")],
        _ => Vec::new(),
    };

    commands
        .into_iter()
        .map(|(id, title, command)| ProjectCheck::new(id, title, command))
        .collect()
}

fn validate_check_id(id: &str) -> RepoDeskResult<()> {
    if id.trim().is_empty()
        || id.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        })
    {
        return Err(RepoDeskError::InvalidCheckCommand(
            "check id must contain only letters, numbers, '-', '_', or '.'".to_string(),
        ));
    }
    Ok(())
}

fn normalize_relevant_path(path: &str) -> String {
    path.trim().trim_matches('/').replace('\\', "/")
}

fn validate_relevant_path(path: &str) -> RepoDeskResult<()> {
    if path.trim().is_empty()
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::ParentDir
            )
        })
    {
        return Err(RepoDeskError::InvalidCheckCommand(format!(
            "relevant path must be a non-empty repository-relative path: {path}"
        )));
    }
    Ok(())
}
