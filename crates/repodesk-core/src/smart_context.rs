use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::fs;

use crate::context::build_context;
use crate::context_pipeline::{ContextPipelineSnapshot, ContextSelectionState};
use crate::embeddings::{EmbeddingProvider, OllamaEmbeddingProvider};
use crate::errors::RepoDeskResult;
use crate::persistence::vector_db;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;
use crate::tokens::{TokenEstimate, format_estimate};

const EMBEDDING_CHUNK_CHARS: usize = 1_000;
const PIPELINE_SNAPSHOT_FILE: &str = "context-pipeline.json";

/// Compatibility projection for the historical `smart-context` CLI.
///
/// RepoDesk no longer maintains a second rendered execution context. The paths
/// point at the canonical Context Pipeline artifacts and `included_files` /
/// `skipped_files` summarize the pipeline's source decisions rather than a
/// separate changed-file scanner.
#[derive(Debug, Clone)]
pub struct SmartContextResult {
    pub context_file: PathBuf,
    pub token_estimate_file: PathBuf,
    pub estimate: TokenEstimate,
    pub included_files: Vec<String>,
    pub skipped_files: Vec<String>,
}

/// Build the canonical Context Pipeline and expose it through the legacy smart
/// context API. This intentionally performs no semantic-network request: Prepare
/// remains deterministic and available when Ollama is offline.
pub async fn build_smart_context() -> RepoDeskResult<SmartContextResult> {
    let result = build_context()?;
    let task = show_active_task()?;
    let snapshot = load_pipeline_snapshot(&task.config.run_dir).await;
    let (included_files, skipped_files) = snapshot
        .as_ref()
        .map(pipeline_source_summary)
        .unwrap_or_default();

    Ok(SmartContextResult {
        context_file: PathBuf::from(result.context_file),
        token_estimate_file: PathBuf::from(result.token_estimate_file),
        estimate: result.estimate,
        included_files,
        skipped_files,
    })
}

/// Incrementally refresh the local semantic retrieval index.
///
/// File fingerprints are compared before any embedding request. Changed files
/// are fully embedded first and only then replace their previous rows in one DB
/// transaction, so an unavailable embedding provider cannot destroy a known-good
/// index. Removed tracked files are deleted from the local index.
pub async fn index_repository() -> RepoDeskResult<()> {
    let project = get_active_project()?;
    let ollama_api =
        std::env::var("OLLAMA_API_BASE").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let provider = OllamaEmbeddingProvider {
        api_base: ollama_api,
        model: "nomic-embed-text".to_string(),
    };

    let files = crate::git_workspace::git_lines(&project.path, &["ls-files"]);
    let tracked: HashSet<String> = files.iter().cloned().collect();

    for relative in files {
        if !is_safe_text_path(&relative) {
            continue;
        }
        let full_path = project.path.join(&relative);
        let content = match fs::read_to_string(&full_path).await {
            Ok(content) => content,
            Err(_) => continue,
        };
        let fingerprint = sha256_hex(content.as_bytes());
        if vector_db::embedding_file_fingerprint(&project.name, &relative)?.as_deref()
            == Some(fingerprint.as_str())
        {
            continue;
        }

        let chars: Vec<char> = content.chars().collect();
        let mut chunks = Vec::new();
        for chunk in chars.chunks(EMBEDDING_CHUNK_CHARS) {
            let chunk_text: String = chunk.iter().collect();
            let embedding = provider.get_embedding(&chunk_text)?;
            chunks.push((chunk_text, embedding));
        }

        vector_db::replace_file_embeddings(
            &project.name,
            &relative,
            &fingerprint,
            &chunks,
        )?;
    }

    for indexed in vector_db::list_indexed_files(&project.name)? {
        if !tracked.contains(&indexed) {
            vector_db::delete_indexed_file(&project.name, &indexed)?;
        }
    }

    Ok(())
}

/// Describe the current canonical pipeline decisions through the old
/// `smart-context sources` command. If Prepare has not built a pipeline yet, the
/// output explains how to create one instead of reconstructing a competing
/// source list.
pub fn list_smart_context_sources() -> RepoDeskResult<String> {
    let task = show_active_task()?;
    let path = task.config.run_dir.join(PIPELINE_SNAPSHOT_FILE);
    if !path.exists() {
        return Ok(
            "Canonical context has not been prepared yet. Run `repodesk context build`; `smart-context` is now a compatibility view of the same Context Pipeline.\n"
                .to_string(),
        );
    }

    let content = std::fs::read_to_string(&path)?;
    let snapshot: ContextPipelineSnapshot = serde_json::from_str(&content)?;
    snapshot.validate()?;
    let (included, excluded) = pipeline_source_summary(&snapshot);

    let mut output = String::from("Canonical Context Pipeline sources:\n");
    for source in included {
        output.push_str(&format!("  - [include] {source}\n"));
    }
    for source in excluded {
        output.push_str(&format!("  - [exclude] {source}\n"));
    }
    Ok(output)
}

pub fn format_smart_context_result(result: &SmartContextResult) -> String {
    format!(
        "Canonical context built (smart-context compatibility view):\n  context file: {}\n  token estimate file: {}\n  included sources: {}\n  excluded sources: {}\n\n{}",
        result.context_file.display(),
        result.token_estimate_file.display(),
        result.included_files.len(),
        result.skipped_files.len(),
        format_estimate(&result.estimate)
    )
}

async fn load_pipeline_snapshot(run_dir: &Path) -> Option<ContextPipelineSnapshot> {
    let content = fs::read_to_string(run_dir.join(PIPELINE_SNAPSHOT_FILE)).await.ok()?;
    let snapshot = serde_json::from_str::<ContextPipelineSnapshot>(&content).ok()?;
    snapshot.validate().ok()?;
    Some(snapshot)
}

fn pipeline_source_summary(snapshot: &ContextPipelineSnapshot) -> (Vec<String>, Vec<String>) {
    let states = snapshot
        .selections
        .iter()
        .map(|selection| (selection.candidate_id.as_str(), &selection.state))
        .collect::<std::collections::HashMap<_, _>>();
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    for candidate in &snapshot.candidates {
        let target = if matches!(states.get(candidate.id.as_str()), Some(ContextSelectionState::Included)) {
            &mut included
        } else {
            &mut excluded
        };
        target.push(candidate.id.clone());
    }
    (included, excluded)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn is_safe_text_path(path: &str) -> bool {
    let lower = path.to_lowercase();

    if lower.contains(".env")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
    {
        return false;
    }

    matches!(
        Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or(""),
        "rs" | "toml"
            | "md"
            | "txt"
            | "json"
            | "yml"
            | "yaml"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "sh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_fingerprints_are_content_stable() {
        assert_eq!(sha256_hex(b"same"), sha256_hex(b"same"));
        assert_ne!(sha256_hex(b"same"), sha256_hex(b"changed"));
    }

    #[test]
    fn unsafe_embedding_paths_remain_blocked() {
        assert!(!is_safe_text_path(".env"));
        assert!(!is_safe_text_path("config/credentials.json"));
        assert!(is_safe_text_path("src/lib.rs"));
    }
}
