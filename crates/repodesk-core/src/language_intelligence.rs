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
use crate::language_tools::managed_executable_path;
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
    Managed,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageServerProfileState {
    Active,
    DiscoveryOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageServerInitializationProfile {
    Default,
    Taplo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    const fn full() -> Self {
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

    /// Servers with no real "go to definition"/"find references" concept for
    /// their language (markup/config formats): hover, completion, formatting
    /// and outline still work, but definition/references/rename don't apply.
    const fn markup() -> Self {
        Self {
            diagnostics: true,
            hover: true,
            definition: false,
            references: false,
            completion: true,
            rename: false,
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
    pub profile_state: LanguageServerProfileState,
    pub initialization_profile: LanguageServerInitializationProfile,
    pub install_recipe_id: Option<String>,
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
    profile_state: LanguageServerProfileState,
    initialization_profile: LanguageServerInitializationProfile,
    install_recipe_id: Option<&'static str>,
    /// What this specific server binary actually implements. Must not default
    /// to "supports everything" — the frontend uses these flags to decide
    /// which LSP requests to send, and a server that doesn't implement a
    /// method replies with a raw JSON-RPC "Method not found" that would
    /// otherwise leak straight into the UI (see RD language-server audit).
    capabilities: LanguageServerCapabilities,
}

const SERVER_SPECS: &[ServerSpec] = &[
    ServerSpec {
        id: "rust-analyzer",
        label: "rust-analyzer",
        executable: "rust-analyzer",
        arguments: &[],
        languages: &["rust"],
        project_local: false,
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("rust-analyzer"),
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "typescript-language-server",
        label: "TypeScript Language Server",
        executable: "typescript-language-server",
        arguments: &["--stdio"],
        languages: &["typescript", "javascript"],
        project_local: true,
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("typescript-language-server"),
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "pyright",
        label: "Pyright",
        executable: "pyright-langserver",
        arguments: &["--stdio"],
        languages: &["python"],
        project_local: true,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        // Pyright has no rename or formatting provider (formatting is left to
        // black/ruff); hover/definition/references/completion/symbols work.
        capabilities: LanguageServerCapabilities {
            rename: false,
            formatting: false,
            ..LanguageServerCapabilities::full()
        },
    },
    ServerSpec {
        id: "gopls",
        label: "gopls",
        executable: "gopls",
        arguments: &[],
        languages: &["go"],
        project_local: false,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "clangd",
        label: "clangd",
        executable: "clangd",
        arguments: &[],
        languages: &["c", "cpp"],
        project_local: false,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "jdtls",
        label: "Eclipse JDT Language Server",
        executable: "jdtls",
        arguments: &[],
        languages: &["java"],
        project_local: false,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "kotlin-language-server",
        label: "Kotlin Language Server",
        executable: "kotlin-language-server",
        arguments: &[],
        languages: &["kotlin"],
        project_local: false,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        capabilities: LanguageServerCapabilities::full(),
    },
    ServerSpec {
        id: "sourcekit-lsp",
        label: "SourceKit-LSP",
        executable: "sourcekit-lsp",
        arguments: &[],
        languages: &["swift"],
        project_local: false,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        // SourceKit-LSP has no built-in documentFormatting provider (Xcode
        // shells out to swift-format separately).
        capabilities: LanguageServerCapabilities {
            formatting: false,
            ..LanguageServerCapabilities::full()
        },
    },
    ServerSpec {
        id: "bash-language-server",
        label: "Bash Language Server",
        executable: "bash-language-server",
        arguments: &["start"],
        languages: &["shell"],
        project_local: true,
        profile_state: LanguageServerProfileState::DiscoveryOnly,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: None,
        // bash-language-server has no references/rename provider.
        capabilities: LanguageServerCapabilities {
            references: false,
            rename: false,
            ..LanguageServerCapabilities::full()
        },
    },
    ServerSpec {
        id: "json-language-server",
        label: "JSON Language Server",
        executable: "vscode-json-language-server",
        arguments: &["--stdio"],
        languages: &["json"],
        project_local: true,
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("json-language-server"),
        // JSON has no cross-file "go to definition"/"find references"/rename.
        capabilities: LanguageServerCapabilities::markup(),
    },
    ServerSpec {
        id: "yaml-language-server",
        label: "YAML Language Server",
        executable: "yaml-language-server",
        arguments: &["--stdio"],
        languages: &["yaml"],
        project_local: true,
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("yaml-language-server"),
        capabilities: LanguageServerCapabilities::markup(),
    },
    ServerSpec {
        id: "taplo",
        label: "Taplo",
        executable: "taplo",
        arguments: &["lsp", "stdio"],
        languages: &["toml"],
        project_local: false,
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Taplo,
        install_recipe_id: Some("taplo"),
        // Taplo implements hover/completion/formatting/documentSymbol but no
        // definition/references/rename — this is the server that surfaced the
        // "Method not found" bug when the frontend assumed full support.
        capabilities: LanguageServerCapabilities::markup(),
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
    let resolved = resolve_executable(spec, project_path);
    let availability = if resolved.is_some() {
        LanguageServerAvailability::Available
    } else {
        LanguageServerAvailability::Missing
    };
    let (executable, source) = resolved
        .map(|(path, source)| (path.to_string_lossy().into_owned(), Some(source)))
        .unwrap_or_else(|| (spec.executable.to_string(), None));

    LanguageServerDescriptor {
        id: spec.id.to_string(),
        label: spec.label.to_string(),
        executable,
        arguments: spec
            .arguments
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        languages: spec
            .languages
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        availability,
        source,
        capabilities: spec.capabilities,
        profile_state: spec.profile_state,
        initialization_profile: spec.initialization_profile,
        install_recipe_id: spec.install_recipe_id.map(str::to_string),
    }
}

fn resolve_executable(
    spec: &ServerSpec,
    project_path: &Path,
) -> Option<(PathBuf, LanguageServerSource)> {
    if spec.project_local
        && let Some(path) =
            executable_in_directory(&project_path.join("node_modules/.bin"), spec.executable)
    {
        return Some((path, LanguageServerSource::ProjectLocal));
    }

    if let Some(recipe_id) = spec.install_recipe_id
        && let Some(path) = managed_executable_path(recipe_id)
    {
        return Some((path, LanguageServerSource::Managed));
    }

    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path).find_map(|directory| {
            executable_in_directory(&directory, spec.executable)
                .map(|path| (path, LanguageServerSource::Path))
        })
    })
}

fn executable_in_directory(directory: &Path, executable: &str) -> Option<PathBuf> {
    executable_variants(directory, executable)
        .into_iter()
        .find(|candidate| candidate.is_file())
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
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn registry_marks_first_wave_profiles_active() {
        let active = SERVER_SPECS
            .iter()
            .filter(|server| server.profile_state == LanguageServerProfileState::Active)
            .map(|server| server.id)
            .collect::<Vec<_>>();
        assert_eq!(
            active,
            vec![
                "rust-analyzer",
                "typescript-language-server",
                "json-language-server",
                "yaml-language-server",
                "taplo",
            ]
        );

        let taplo = SERVER_SPECS
            .iter()
            .find(|server| server.id == "taplo")
            .expect("taplo profile");
        assert_eq!(
            taplo.initialization_profile,
            LanguageServerInitializationProfile::Taplo
        );
        assert_eq!(taplo.install_recipe_id, Some("taplo"));
        // Regression: taplo has no definition/references/rename provider, so
        // these must be false even though the server is otherwise "active" —
        // a blanket `full()` here is exactly what caused the reported
        // "Method not found" bug (the frontend assumed definition support).
        assert!(!taplo.capabilities.definition);
        assert!(!taplo.capabilities.references);
        assert!(!taplo.capabilities.rename);
        assert!(taplo.capabilities.hover);
        assert!(taplo.capabilities.formatting);
        assert!(taplo.capabilities.document_symbols);

        for markup_id in ["taplo", "json-language-server", "yaml-language-server"] {
            let spec = SERVER_SPECS
                .iter()
                .find(|server| server.id == markup_id)
                .unwrap_or_else(|| panic!("{markup_id} profile"));
            assert!(
                !spec.capabilities.definition
                    && !spec.capabilities.references
                    && !spec.capabilities.rename,
                "{markup_id} must not advertise definition/references/rename support"
            );
        }

        let typescript = SERVER_SPECS
            .iter()
            .find(|server| server.id == "typescript-language-server")
            .expect("typescript profile");
        assert_eq!(typescript.languages, &["typescript", "javascript"]);
        assert_eq!(
            typescript.install_recipe_id,
            Some("typescript-language-server")
        );

        let discovery_only = SERVER_SPECS
            .iter()
            .filter(|server| server.profile_state == LanguageServerProfileState::DiscoveryOnly)
            .map(|server| server.id)
            .collect::<Vec<_>>();
        assert_eq!(
            discovery_only,
            vec![
                "pyright",
                "gopls",
                "clangd",
                "jdtls",
                "kotlin-language-server",
                "sourcekit-lsp",
                "bash-language-server",
            ]
        );
    }

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
    fn project_local_descriptor_uses_resolved_executable_path() {
        let project = TempDir::new().expect("project tempdir");
        let bin = project.path().join("node_modules/.bin");
        fs::create_dir_all(&bin).expect("project bin");
        let executable = if cfg!(windows) {
            bin.join("typescript-language-server.cmd")
        } else {
            bin.join("typescript-language-server")
        };
        fs::write(&executable, "fake").expect("fake executable");

        let spec = SERVER_SPECS
            .iter()
            .find(|server| server.id == "typescript-language-server")
            .expect("typescript profile");
        let descriptor = descriptor_for(spec, project.path());

        assert_eq!(
            descriptor.availability,
            LanguageServerAvailability::Available
        );
        assert_eq!(descriptor.source, Some(LanguageServerSource::ProjectLocal));
        assert_eq!(PathBuf::from(descriptor.executable), executable);
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
            profile_state: LanguageServerProfileState::Active,
            initialization_profile: LanguageServerInitializationProfile::Default,
            install_recipe_id: None,
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
            profile_state: LanguageServerProfileState::Active,
            initialization_profile: LanguageServerInitializationProfile::Default,
            install_recipe_id: None,
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
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 4,
            },
        };
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
    }
}
