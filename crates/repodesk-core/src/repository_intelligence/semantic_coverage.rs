use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::code_workspace::CodeWorkspaceFile;

use super::RepositoryRelation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositorySemanticStrategy {
    RustAst,
    ScriptLiteralImports,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryEvidenceLevel {
    Strong,
    Bounded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryLanguageCoverage {
    pub language: String,
    pub visible_files: usize,
    pub semantic_files_indexed: usize,
    pub semantic_bytes_indexed: u64,
    pub strategy: RepositorySemanticStrategy,
    pub evidence_level: RepositoryEvidenceLevel,
    pub truncated: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySemanticCoverage {
    pub semantic_files_eligible: usize,
    pub semantic_files_indexed: usize,
    pub semantic_bytes_indexed: u64,
    pub languages: Vec<RepositoryLanguageCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryGraphEvidence {
    pub strategy: RepositorySemanticStrategy,
    pub level: RepositoryEvidenceLevel,
    pub indexed: bool,
    pub reasons: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SemanticIndexBounds {
    pub(super) workspace: bool,
    pub(super) rust: bool,
    pub(super) scripts: bool,
}

impl SemanticIndexBounds {
    pub(super) fn any(self) -> bool {
        self.workspace || self.rust || self.scripts
    }

    fn truncated_for(self, strategy: RepositorySemanticStrategy) -> bool {
        self.workspace
            || match strategy {
                RepositorySemanticStrategy::RustAst => self.rust,
                RepositorySemanticStrategy::ScriptLiteralImports => self.scripts,
                RepositorySemanticStrategy::Unavailable => false,
            }
    }
}

#[derive(Default)]
struct MutableLanguageCoverage {
    visible_files: usize,
    semantic_files_indexed: usize,
    semantic_bytes_indexed: u64,
}

pub(super) fn build_semantic_coverage(
    files: &[CodeWorkspaceFile],
    dependencies: &BTreeMap<String, Vec<RepositoryRelation>>,
    bounds: SemanticIndexBounds,
) -> RepositorySemanticCoverage {
    let mut languages = BTreeMap::<String, MutableLanguageCoverage>::new();
    let mut semantic_files_eligible = 0_usize;
    let mut semantic_files_indexed = 0_usize;
    let mut semantic_bytes_indexed = 0_u64;

    for file in files.iter().filter(|file| !file.blocked) {
        let entry = languages.entry(file.language.clone()).or_default();
        entry.visible_files += 1;

        let strategy = strategy_for_language(&file.language);
        if strategy == RepositorySemanticStrategy::Unavailable {
            continue;
        }
        semantic_files_eligible += 1;

        if dependencies.contains_key(&file.path) {
            entry.semantic_files_indexed += 1;
            entry.semantic_bytes_indexed = entry.semantic_bytes_indexed.saturating_add(file.bytes);
            semantic_files_indexed += 1;
            semantic_bytes_indexed = semantic_bytes_indexed.saturating_add(file.bytes);
        }
    }

    let mut languages = languages
        .into_iter()
        .map(|(language, counts)| {
            let strategy = strategy_for_language(&language);
            let truncated = bounds.truncated_for(strategy);
            let evidence_level = language_evidence_level(strategy, &counts, truncated);
            RepositoryLanguageCoverage {
                limitations: limitations_for_strategy(strategy, bounds),
                language,
                visible_files: counts.visible_files,
                semantic_files_indexed: counts.semantic_files_indexed,
                semantic_bytes_indexed: counts.semantic_bytes_indexed,
                strategy,
                evidence_level,
                truncated,
            }
        })
        .collect::<Vec<_>>();
    languages.sort_by(|left, right| {
        right
            .visible_files
            .cmp(&left.visible_files)
            .then_with(|| left.language.cmp(&right.language))
    });

    RepositorySemanticCoverage {
        semantic_files_eligible,
        semantic_files_indexed,
        semantic_bytes_indexed,
        languages,
    }
}

pub(super) fn graph_evidence_for_focus(
    focus: &str,
    language: &str,
    dependencies: &BTreeMap<String, Vec<RepositoryRelation>>,
    bounds: SemanticIndexBounds,
) -> RepositoryGraphEvidence {
    let strategy = strategy_for_language(language);
    if strategy == RepositorySemanticStrategy::Unavailable {
        return RepositoryGraphEvidence {
            strategy,
            level: RepositoryEvidenceLevel::Unavailable,
            indexed: false,
            reasons: vec![format!(
                "RepoDesk does not yet build dependency edges for {language} files."
            )],
            limitations: limitations_for_strategy(strategy, bounds),
        };
    }

    if !dependencies.contains_key(focus) {
        let mut limitations = limitations_for_strategy(strategy, bounds);
        limitations.push(
            "The focus file was not included in the semantic index; it may be oversized, unreadable, unparsable, or outside a bounded scan."
                .to_string(),
        );
        return RepositoryGraphEvidence {
            strategy,
            level: RepositoryEvidenceLevel::Unavailable,
            indexed: false,
            reasons: vec!["No semantic evidence was produced for the focus file.".to_string()],
            limitations,
        };
    }

    let truncated = bounds.truncated_for(strategy);
    let (level, reason) = match strategy {
        RepositorySemanticStrategy::RustAst if !truncated => (
            RepositoryEvidenceLevel::Strong,
            "The focus file was parsed with the Rust AST index.".to_string(),
        ),
        RepositorySemanticStrategy::RustAst => (
            RepositoryEvidenceLevel::Bounded,
            "The focus file was parsed with the Rust AST index, but Rust graph coverage was bounded."
                .to_string(),
        ),
        RepositorySemanticStrategy::ScriptLiteralImports => (
            RepositoryEvidenceLevel::Bounded,
            "The focus file was scanned for local literal TypeScript/JavaScript import evidence."
                .to_string(),
        ),
        RepositorySemanticStrategy::Unavailable => unreachable!("handled above"),
    };

    RepositoryGraphEvidence {
        strategy,
        level,
        indexed: true,
        reasons: vec![reason],
        limitations: limitations_for_strategy(strategy, bounds),
    }
}

fn strategy_for_language(language: &str) -> RepositorySemanticStrategy {
    match language {
        "rust" => RepositorySemanticStrategy::RustAst,
        "typescript" | "javascript" => RepositorySemanticStrategy::ScriptLiteralImports,
        _ => RepositorySemanticStrategy::Unavailable,
    }
}

fn language_evidence_level(
    strategy: RepositorySemanticStrategy,
    counts: &MutableLanguageCoverage,
    truncated: bool,
) -> RepositoryEvidenceLevel {
    if strategy == RepositorySemanticStrategy::Unavailable || counts.semantic_files_indexed == 0 {
        return RepositoryEvidenceLevel::Unavailable;
    }

    match strategy {
        RepositorySemanticStrategy::RustAst
            if !truncated && counts.semantic_files_indexed == counts.visible_files =>
        {
            RepositoryEvidenceLevel::Strong
        }
        RepositorySemanticStrategy::RustAst | RepositorySemanticStrategy::ScriptLiteralImports => {
            RepositoryEvidenceLevel::Bounded
        }
        RepositorySemanticStrategy::Unavailable => RepositoryEvidenceLevel::Unavailable,
    }
}

fn limitations_for_strategy(
    strategy: RepositorySemanticStrategy,
    bounds: SemanticIndexBounds,
) -> Vec<String> {
    let mut limitations = match strategy {
        RepositorySemanticStrategy::RustAst => vec![
            "Only local Rust module/use relationships are resolved; macros and external crates are not expanded."
                .to_string(),
        ],
        RepositorySemanticStrategy::ScriptLiteralImports => vec![
            "Only relative literal imports are resolved; package imports, aliases, and computed imports remain unknown."
                .to_string(),
        ],
        RepositorySemanticStrategy::Unavailable => vec![
            "Dependency/dependent lists can be empty even when real relationships exist."
                .to_string(),
        ],
    };

    if bounds.workspace {
        limitations.push(
            "The visible workspace listing hit its repository-wide bound, so semantic coverage may omit files of this language."
                .to_string(),
        );
    }
    match strategy {
        RepositorySemanticStrategy::RustAst if bounds.rust => limitations.push(
            "The Rust AST semantic index hit its bound, so Rust reverse edges and coverage may be incomplete."
                .to_string(),
        ),
        RepositorySemanticStrategy::ScriptLiteralImports if bounds.scripts => limitations.push(
            "The TypeScript/JavaScript literal-import semantic index hit its bound, so script reverse edges and coverage may be incomplete."
                .to_string(),
        ),
        _ => {}
    }
    limitations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_workspace::CodeWorkspaceFileStatus;

    fn file(path: &str, language: &str, bytes: u64, blocked: bool) -> CodeWorkspaceFile {
        CodeWorkspaceFile {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            extension: None,
            language: language.to_string(),
            bytes,
            status: CodeWorkspaceFileStatus::Clean,
            blocked,
        }
    }

    #[test]
    fn coverage_distinguishes_semantic_support_from_visible_files() {
        let files = vec![
            file("src/lib.rs", "rust", 100, false),
            file("src/app.ts", "typescript", 200, false),
            file("src/index.html", "html", 300, false),
            file("src/secret.rs", "rust", 400, true),
        ];
        let dependencies = BTreeMap::from([
            ("src/lib.rs".to_string(), Vec::new()),
            ("src/app.ts".to_string(), Vec::new()),
        ]);

        let coverage =
            build_semantic_coverage(&files, &dependencies, SemanticIndexBounds::default());
        assert_eq!(coverage.semantic_files_eligible, 2);
        assert_eq!(coverage.semantic_files_indexed, 2);
        assert_eq!(coverage.semantic_bytes_indexed, 300);

        let rust = coverage
            .languages
            .iter()
            .find(|item| item.language == "rust")
            .expect("rust coverage");
        assert_eq!(rust.visible_files, 1);
        assert_eq!(rust.evidence_level, RepositoryEvidenceLevel::Strong);
        assert!(!rust.truncated);

        let html = coverage
            .languages
            .iter()
            .find(|item| item.language == "html")
            .expect("html coverage");
        assert_eq!(html.strategy, RepositorySemanticStrategy::Unavailable);
        assert_eq!(html.semantic_files_indexed, 0);
    }

    #[test]
    fn script_bound_does_not_downgrade_complete_rust_coverage() {
        let files = vec![
            file("src/lib.rs", "rust", 100, false),
            file("src/app.ts", "typescript", 200, false),
        ];
        let dependencies = BTreeMap::from([
            ("src/lib.rs".to_string(), Vec::new()),
            ("src/app.ts".to_string(), Vec::new()),
        ]);
        let bounds = SemanticIndexBounds {
            scripts: true,
            ..SemanticIndexBounds::default()
        };

        let coverage = build_semantic_coverage(&files, &dependencies, bounds);
        let rust = coverage
            .languages
            .iter()
            .find(|item| item.language == "rust")
            .expect("rust coverage");
        let typescript = coverage
            .languages
            .iter()
            .find(|item| item.language == "typescript")
            .expect("typescript coverage");

        assert_eq!(rust.evidence_level, RepositoryEvidenceLevel::Strong);
        assert!(!rust.truncated);
        assert!(typescript.truncated);
        assert!(
            !rust
                .limitations
                .iter()
                .any(|item| item.contains("literal-import semantic index"))
        );
    }

    #[test]
    fn rust_bound_does_not_contaminate_script_limitations() {
        let dependencies = BTreeMap::from([("src/app.ts".to_string(), Vec::new())]);
        let bounds = SemanticIndexBounds {
            rust: true,
            ..SemanticIndexBounds::default()
        };
        let evidence = graph_evidence_for_focus("src/app.ts", "typescript", &dependencies, bounds);

        assert!(evidence.indexed);
        assert_eq!(evidence.level, RepositoryEvidenceLevel::Bounded);
        assert!(
            !evidence
                .limitations
                .iter()
                .any(|item| item.contains("Rust AST semantic index"))
        );
    }

    #[test]
    fn rust_bound_downgrades_rust_graph_evidence() {
        let dependencies = BTreeMap::from([("src/lib.rs".to_string(), Vec::new())]);
        let bounds = SemanticIndexBounds {
            rust: true,
            ..SemanticIndexBounds::default()
        };
        let evidence = graph_evidence_for_focus("src/lib.rs", "rust", &dependencies, bounds);
        assert!(evidence.indexed);
        assert_eq!(evidence.level, RepositoryEvidenceLevel::Bounded);
        assert!(
            evidence
                .limitations
                .iter()
                .any(|item| item.contains("Rust AST semantic index hit its bound"))
        );
    }

    #[test]
    fn workspace_bound_applies_to_every_supported_language() {
        let files = vec![
            file("src/lib.rs", "rust", 100, false),
            file("src/app.ts", "typescript", 200, false),
        ];
        let dependencies = BTreeMap::from([
            ("src/lib.rs".to_string(), Vec::new()),
            ("src/app.ts".to_string(), Vec::new()),
        ]);
        let bounds = SemanticIndexBounds {
            workspace: true,
            ..SemanticIndexBounds::default()
        };

        let coverage = build_semantic_coverage(&files, &dependencies, bounds);
        assert!(coverage.languages.iter().all(|item| item.truncated));
        let rust = coverage
            .languages
            .iter()
            .find(|item| item.language == "rust")
            .expect("rust coverage");
        assert_eq!(rust.evidence_level, RepositoryEvidenceLevel::Bounded);
    }

    #[test]
    fn unsupported_language_is_explicitly_unavailable() {
        let evidence = graph_evidence_for_focus(
            "src/index.html",
            "html",
            &BTreeMap::<String, Vec<RepositoryRelation>>::new(),
            SemanticIndexBounds::default(),
        );
        assert!(!evidence.indexed);
        assert_eq!(evidence.level, RepositoryEvidenceLevel::Unavailable);
        assert_eq!(evidence.strategy, RepositorySemanticStrategy::Unavailable);
    }
}
