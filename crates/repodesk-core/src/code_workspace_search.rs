//! Whole-repository Quick Open search for the Code workspace.
//!
//! The frontend must not preload thousands of file commands merely to perform
//! fuzzy matching. This module keeps a short-lived, project-scoped index of the
//! same guarded Code Workspace file metadata and returns only the best matches.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::code_workspace::{
    CodeWorkspaceFile, CodeWorkspaceFileStatus, load_code_workspace,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;

const QUICK_OPEN_INDEX_TTL: Duration = Duration::from_secs(2);
pub const MAX_QUICK_OPEN_QUERY_CHARS: usize = 256;
pub const MAX_QUICK_OPEN_RESULTS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeQuickOpenResult {
    pub path: String,
    pub name: String,
    pub language: String,
    pub status: CodeWorkspaceFileStatus,
}

#[derive(Clone)]
struct QuickOpenIndexEntry {
    observed_at: Instant,
    project: String,
    files: Vec<CodeWorkspaceFile>,
}

static QUICK_OPEN_INDEX: OnceLock<Mutex<BTreeMap<PathBuf, QuickOpenIndexEntry>>> = OnceLock::new();

pub fn search_active_code_workspace(
    query: &str,
    limit: usize,
) -> RepoDeskResult<Vec<CodeQuickOpenResult>> {
    let query = query.trim();
    if query.chars().count() > MAX_QUICK_OPEN_QUERY_CHARS {
        return Err(RepoDeskError::Api(format!(
            "Quick Open query exceeds the {MAX_QUICK_OPEN_QUERY_CHARS} character limit"
        )));
    }
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let project = get_active_project()?;
    let root = project.path.canonicalize()?;
    let files = indexed_files(&project.name, &root)?;
    Ok(rank_quick_open_results(
        &files,
        query,
        limit.clamp(1, MAX_QUICK_OPEN_RESULTS),
    ))
}

pub fn invalidate_active_quick_open_index() {
    let Ok(project) = get_active_project() else {
        return;
    };
    let Ok(root) = project.path.canonicalize() else {
        return;
    };
    invalidate_quick_open_index(&root);
}

fn quick_open_index() -> &'static Mutex<BTreeMap<PathBuf, QuickOpenIndexEntry>> {
    QUICK_OPEN_INDEX.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn indexed_files(project_name: &str, root: &Path) -> RepoDeskResult<Vec<CodeWorkspaceFile>> {
    if let Ok(cache) = quick_open_index().lock()
        && let Some(entry) = cache.get(root)
        && entry.project == project_name
        && entry.observed_at.elapsed() <= QUICK_OPEN_INDEX_TTL
    {
        return Ok(entry.files.clone());
    }

    let snapshot = load_code_workspace(project_name, root)?;
    let files = snapshot
        .files
        .into_iter()
        .filter(|file| !file.blocked)
        .collect::<Vec<_>>();

    if let Ok(mut cache) = quick_open_index().lock() {
        cache.insert(
            root.to_path_buf(),
            QuickOpenIndexEntry {
                observed_at: Instant::now(),
                project: project_name.to_string(),
                files: files.clone(),
            },
        );
    }
    Ok(files)
}

fn invalidate_quick_open_index(root: &Path) {
    if let Ok(mut cache) = quick_open_index().lock() {
        cache.remove(root);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MatchRank {
    class: u8,
    subsequence_gap: usize,
    changed_penalty: u8,
    path_len: usize,
}

fn rank_quick_open_results(
    files: &[CodeWorkspaceFile],
    query: &str,
    limit: usize,
) -> Vec<CodeQuickOpenResult> {
    let query = normalize(query);
    if query.is_empty() {
        return Vec::new();
    }

    let mut ranked = files
        .iter()
        .filter(|file| !file.blocked)
        .filter_map(|file| match_rank(file, &query).map(|rank| (file, rank)))
        .collect::<Vec<_>>();

    ranked.sort_by(|(left_file, left_rank), (right_file, right_rank)| {
        compare_rank(*left_rank, *right_rank)
            .then_with(|| left_file.path.to_ascii_lowercase().cmp(&right_file.path.to_ascii_lowercase()))
            .then_with(|| left_file.path.cmp(&right_file.path))
    });

    ranked
        .into_iter()
        .take(limit)
        .map(|(file, _)| CodeQuickOpenResult {
            path: file.path.clone(),
            name: file.name.clone(),
            language: file.language.clone(),
            status: file.status,
        })
        .collect()
}

fn compare_rank(left: MatchRank, right: MatchRank) -> Ordering {
    left.class
        .cmp(&right.class)
        .then_with(|| left.subsequence_gap.cmp(&right.subsequence_gap))
        .then_with(|| left.changed_penalty.cmp(&right.changed_penalty))
        .then_with(|| left.path_len.cmp(&right.path_len))
}

fn match_rank(file: &CodeWorkspaceFile, query: &str) -> Option<MatchRank> {
    let path = normalize(&file.path);
    let name = normalize(&file.name);
    let changed_penalty = if file.status == CodeWorkspaceFileStatus::Clean {
        1
    } else {
        0
    };

    let (class, subsequence_gap) = if path == query {
        (0, 0)
    } else if name == query {
        (1, 0)
    } else if name.starts_with(query) {
        (2, 0)
    } else if path.starts_with(query) {
        (3, 0)
    } else if name.contains(query) {
        (4, 0)
    } else if path.contains(query) {
        (5, 0)
    } else if let Some(gap) = subsequence_gap(&name, query) {
        (6, gap)
    } else if let Some(gap) = subsequence_gap(&path, query) {
        (7, gap)
    } else {
        return None;
    };

    Some(MatchRank {
        class,
        subsequence_gap,
        changed_penalty,
        path_len: path.len(),
    })
}

fn normalize(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn subsequence_gap(text: &str, query: &str) -> Option<usize> {
    let mut cursor = 0;
    let mut first = None;
    let mut last = 0;

    for query_char in query.chars() {
        let suffix = text.get(cursor..)?;
        let relative = suffix.find(query_char)?;
        let absolute = cursor + relative;
        first.get_or_insert(absolute);
        last = absolute;
        cursor = absolute + query_char.len_utf8();
    }

    Some(last.saturating_sub(first.unwrap_or(last)) + 1 - query.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, status: CodeWorkspaceFileStatus) -> CodeWorkspaceFile {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        CodeWorkspaceFile {
            path: path.to_string(),
            name,
            extension: Some("rs".into()),
            language: "rust".into(),
            bytes: 42,
            status,
            blocked: false,
        }
    }

    #[test]
    fn quick_open_search_is_not_limited_by_input_position() {
        let mut files = (0..220)
            .map(|index| file(&format!("src/generated/file-{index:03}.rs"), CodeWorkspaceFileStatus::Clean))
            .collect::<Vec<_>>();
        files.push(file("src/important/session_manager.rs", CodeWorkspaceFileStatus::Modified));

        let results = rank_quick_open_results(&files, "session", 50);
        assert_eq!(results.first().map(|result| result.path.as_str()), Some("src/important/session_manager.rs"));
    }

    #[test]
    fn exact_and_filename_matches_beat_loose_subsequences() {
        let files = vec![
            file("src/auth/session.rs", CodeWorkspaceFileStatus::Clean),
            file("src/session.rs", CodeWorkspaceFileStatus::Clean),
            file("src/service/session_registry.rs", CodeWorkspaceFileStatus::Modified),
        ];

        let results = rank_quick_open_results(&files, "session.rs", 10);
        assert_eq!(results[0].path, "src/session.rs");
        assert_eq!(results[1].path, "src/auth/session.rs");
    }

    #[test]
    fn changed_file_is_a_tie_breaker_not_a_relevance_override() {
        let files = vec![
            file("src/auth/token.rs", CodeWorkspaceFileStatus::Clean),
            file("tests/token.rs", CodeWorkspaceFileStatus::Modified),
        ];

        let results = rank_quick_open_results(&files, "token.rs", 10);
        assert_eq!(results[0].path, "tests/token.rs");
    }

    #[test]
    fn blocked_files_never_surface() {
        let mut secret = file("config/.env", CodeWorkspaceFileStatus::Modified);
        secret.blocked = true;
        let results = rank_quick_open_results(&[secret], "env", 10);
        assert!(results.is_empty());
    }
}
