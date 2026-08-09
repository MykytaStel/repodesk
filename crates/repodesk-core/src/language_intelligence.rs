//! Language-intelligence discovery and protocol-facing data contracts.
//!
//! RD2-15 deliberately separates *discovery* from *session execution*.
//! Discovering a language server is cheap and side-effect free; starting a
//! long-lived stdio JSON-RPC process needs lifecycle, cancellation, document
//! versioning and shutdown semantics and is built on top of this contract.

use std::env;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageServerAvailability {
    Available,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageServerSource {
    ProjectLocal,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageServerCapabilities {
    pub diagnostics: bool,
    pub hover: bool,
    pub definition: bool,
    pub references: bool,
    pub completion: bool,
    pub rename: bool,
    pub formatting: bool,
    pub document_symbols: bool,
}

impl LanguageServerCapabilities {
    fn full() -> Self {
        Self {
            diagnostics: true,
            hover: true,
            definition: true,
            references: true,
            completion: true,
            rename: true,
            formatting: true,
            document_symbols: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageServerDescriptor {
    pub id: String,
    pub label: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub languages: Vec<String>,
    pub availability: LanguageServerAvailability,
    pub source: Option<LanguageServerSource>,
    pub capabilities: LanguageServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageIntelligenceSnapshot {
    pub project: String,
    pub primary_language: Option<String>,
    pub servers: Vec<LanguageServerDescriptor>,
    pub available_count: usize,
    pub generated_at: DateTime<Utc>,
}

/// LSP protocol coordinates are zero-based. Keeping that invariant explicit in
/// the core contract prevents the UI's one-based line numbers from leaking into
/// JSON-RPC messages later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageDiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDiagnostic {
    pub server_id: String,
    pub path: String,
    pub range: LspRange,
    pub severity: LanguageDiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Copy)]
struct ServerSpec {
    id: &'static str,
    label: &'static str,
    executable: &'static str,
    arguments: &'static [&'static str],
    languages: &'static [&'static str],
    project_local: bool,
}

const SERVER_SPECS: &[ServerSpec] = &[
    ServerSpec {
        id: "rust-analyzer",
        label: "rust-analyzer",
        executable: "rust-analyzer",
        arguments: &[],
        languages: &["rust"],
        project_local: false,
    },
    ServerSpec {
        id: "typescript-language-server",
        label: "TypeScript Language Server",
        executable: "typescript-language-server",
        arguments: &["--stdio"],
        languages: &["typescript", "javascript"],
        project_local: true,
    },
    ServerSpec {
        id: "pyright",
        label: "Pyright",
        executable: "pyright-langserver",
        arguments: &["--stdio"],
        languages: &["python"],
        project_local: true,
    },
    ServerSpec {
        id: "gopls",
        label: "gopls",
        executable: "gopls",
        arguments: &[],
        languages: &["go"],
        project_local: false,
    },
    ServerSpec {
        id: "clangd",
        label: "clangd",
        executable: "clangd",
        arguments: &[],
        languages: &["c", "cpp"],
        project_local: false,
    },
    ServerSpec {
        id: "jdtls",
        label: "Eclipse JDT Language Server",
        executable: "jdtls",
        arguments: &[],
        languages: &["java"],
        project_local: false,
    },
    ServerSpec {
        id: "kotlin-language-server",
        label: "Kotlin Language Server",
        executable: "kotlin-language-server",
        arguments: &[],
        languages: &["kotlin"],
        project_local: false,
    },
    ServerSpec {
        id: "sourcekit-lsp",
        label: "SourceKit-LSP",
        executable: "sourcekit-lsp",
        arguments: &[],
        languages: &["swift"],
        project_local: false,
    },
    ServerSpec {
        id: "bash-language-server",
        label: "Bash Language Server",
        executable: "bash-language-server",
        arguments: &["start"],
        languages: &["shell"],
        project_local: true,
    },
    ServerSpec {
        id: "json-language-server",
        label: "JSON Language Server",
        executable: "vscode-json-language-server",
        arguments: &["--stdio"],
        languages: &["json"],
        project_local: true,
    },
    ServerSpec {
        id: "yaml-language-server",
        label: "YAML Language Server",
        executable: "yaml-language-server",
        arguments: &["--stdio"],
        languages: &["yaml"],
        project_local: true,
    },
    ServerSpec {
        id: "taplo",
        label: "Taplo",
        executable: "taplo",
        arguments: &["lsp", "stdio"],
        languages: &["toml"],
        project_local: false,
    },
];

pub fn active_language_intelligence_snapshot() -> RepoDeskResult<LanguageIntelligenceSnapshot> {
    let project = get_active_project()?;
    let servers = SERVER_SPECS
        .iter()
        .map(|spec| descriptor_for(spec, &project.path))
        .collect::<Vec<_>>();
    let available_count = servers
        .iter()
        .filter(|server| server.availability == LanguageServerAvailability::Available)
        .count();

    Ok(LanguageIntelligenceSnapshot {
        project: project.name,
        primary_language: project.main_language,
        servers,
        available_count,
        generated_at: Utc::now(),
    })
}

pub fn preferred_server_for_language<'a>(
    snapshot: &'a LanguageIntelligenceSnapshot,
    language: &str,
) -> Option<&'a LanguageServerDescriptor> {
    snapshot
        .servers
        .iter()
        .filter(|server| server.languages.iter().any(|value| value == language))
        .min_by_key(|server| match server.availability {
            LanguageServerAvailability::Available => 0,
            LanguageServerAvailability::Missing => 1,
        })
}

fn descriptor_for(spec: &ServerSpec, project_path: &Path) -> LanguageServerDescriptor {
    let source = resolve_executable_source(spec, project_path);
    LanguageServerDescriptor {
        id: spec.id.to_string(),
        label: spec.label.to_string(),
        executable: spec.executable.to_string(),
        arguments: spec.arguments.iter().map(|value| (*value).to_string()).collect(),
        languages: spec.languages.iter().map(|value| (*value).to_string()).collect(),
        availability: if source.is_some() {
            LanguageServerAvailability::Available
        } else {
            LanguageServerAvailability::Missing
        },
        source,
        capabilities: LanguageServerCapabilities::full(),
    }
}

fn resolve_executable_source(spec: &ServerSpec, project_path: &Path) -> Option<LanguageServerSource> {
    if spec.project_local && executable_in_directory(&project_path.join("node_modules/.bin"), spec.executable) {
        return Some(LanguageServerSource::ProjectLocal);
    }

    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .any(|directory| executable_in_directory(&directory, spec.executable))
            .then_some(LanguageServerSource::Path)
    })
}

fn executable_in_directory(directory: &Path, executable: &str) -> bool {
    executable_variants(directory, executable)
        .iter()
        .any(|candidate| candidate.is_file())
}

fn executable_variants(directory: &Path, executable: &str) -> Vec<PathBuf> {
    let base = directory.join(executable);
    if cfg!(windows) {
        vec![
            base.clone(),
            base.with_extension("exe"),
            base.with_extension("cmd"),
            base.with_extension("bat"),
        ]
    } else {
        vec![base]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_maps_primary_languages_to_expected_servers() {
        let rust = SERVER_SPECS
            .iter()
            .find(|server| server.languages.contains(&"rust"))
            .expect("rust server");
        assert_eq!(rust.id, "rust-analyzer");

        let typescript = SERVER_SPECS
            .iter()
            .find(|server| server.languages.contains(&"typescript"))
            .expect("typescript server");
        assert_eq!(typescript.id, "typescript-language-server");
    }

    #[test]
    fn preferred_server_favors_available_server() {
        let missing = LanguageServerDescriptor {
            id: "missing".into(),
            label: "Missing".into(),
            executable: "missing".into(),
            arguments: Vec::new(),
            languages: vec!["rust".into()],
            availability: LanguageServerAvailability::Missing,
            source: None,
            capabilities: LanguageServerCapabilities::full(),
        };
        let available = LanguageServerDescriptor {
            id: "available".into(),
            label: "Available".into(),
            executable: "available".into(),
            arguments: Vec::new(),
            languages: vec!["rust".into()],
            availability: LanguageServerAvailability::Available,
            source: Some(LanguageServerSource::Path),
            capabilities: LanguageServerCapabilities::full(),
        };
        let snapshot = LanguageIntelligenceSnapshot {
            project: "demo".into(),
            primary_language: Some("rust".into()),
            servers: vec![missing, available],
            available_count: 1,
            generated_at: Utc::now(),
        };

        assert_eq!(
            preferred_server_for_language(&snapshot, "rust").map(|server| server.id.as_str()),
            Some("available")
        );
    }

    #[test]
    fn lsp_coordinates_are_explicitly_zero_based() {
        let range = LspRange {
            start: LspPosition { line: 0, character: 0 },
            end: LspPosition { line: 0, character: 4 },
        };
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
    }
}
