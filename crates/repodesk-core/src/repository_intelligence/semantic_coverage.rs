use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::code_workspace::{CodeWorkspaceFile, CodeWorkspaceFileStatus};

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

#[derive(Default)]
struct MutableLanguageCoverage {
    visible_files: usize,
    semantic_files_indexed: usize,
    semantic_bytes_indexed: u64,
}

pub(super) fn build_semantic_coverage(
    files: &[CodeWorkspaceFile],
    dependencies: &BTreeMap<String, Vec<RepositoryRelation>>,
    semantic_truncated: bool,
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
            let evidence_level = language_evidence_level(strategy, &counts, semantic_truncated);
            RepositoryLanguageCoverage {
                limitations: limitations_for_strategy(strategy, semantic_truncated),
                language,
                visible_files: counts.visible_files,
                semantic_files_indexed: counts.semantic_files_indexed,
                semantic_bytes_indexed: counts.semantic_bytes_indexed,
                strategy,
                evidence_level,
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
    semantic_truncated: bool,
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
            limitations: limitations_for_strategy(strategy, semantic_truncated),
        };
    }

    if !dependencies.contains_key(focus) {
        let mut limitations = limitations_for_strategy(strategy, semantic_truncated);
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

    let (level, reason) = match strategy {
        RepositorySemanticStrategy::RustAst if !semantic_truncated => (
            RepositoryEvidenceLevel::Strong,
            "The focus file was parsed with the Rust AST index.".to_string(),
        ),
        RepositorySemanticStrategy::RustAst => (
            RepositoryEvidenceLevel::Bounded,
            "The focus file was parsed with the Rust AST index, but repository coverage was bounded."
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
        limitations: limitations_for_strategy(strategy, semantic_truncated),
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
    semantic_truncated: bool,
) -> RepositoryEvidenceLevel {
    if strategy == RepositorySemanticStrategy::Unavailable || counts.semantic_files_indexed == 0 {
        return RepositoryEvidenceLevel::Unavailable;
    }

    match strategy {
        RepositorySemanticStrategy::RustAst
            if !semantic_truncated && counts.semantic_files_indexed == counts.visible_files =>
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
    semantic_truncated: bool,
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

    if semantic_truncated {
        limitations.push(
            "The repository semantic index hit a bound, so reverse edges and coverage may be incomplete."
                .to_string(),
        );
    }
    limitations
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let coverage = build_semantic_coverage(&files, &dependencies, false);
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

        let html = coverage
            .languages
            .iter()
            .find(|item| item.language == "html")
            .expect("html coverage");
        assert_eq!(html.strategy, RepositorySemanticStrategy::Unavailable);
        assert_eq!(html.semantic_files_indexed, 0);
    }

    #[test]
    fn bounded_repository_downgrades_rust_graph_evidence() {
        let dependencies = BTreeMap::from([("src/lib.rs".to_string(), Vec::new())]);
        let evidence = graph_evidence_for_focus("src/lib.rs", "rust", &dependencies, true);
        assert!(evidence.indexed);
        assert_eq!(evidence.level, RepositoryEvidenceLevel::Bounded);
        assert!(
            evidence
                .limitations
                .iter()
                .any(|item| item.contains("hit a bound"))
        );
    }

    #[test]
    fn unsupported_language_is_explicitly_unavailable() {
        let evidence = graph_evidence_for_focus(
            "src/index.html",
            "html",
            &BTreeMap::<String, Vec<RepositoryRelation>>::new(),
            false,
        );
        assert!(!evidence.indexed);
        assert_eq!(evidence.level, RepositoryEvidenceLevel::Unavailable);
        assert_eq!(evidence.strategy, RepositorySemanticStrategy::Unavailable);
    }
}
