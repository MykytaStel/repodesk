//! Guarded repository search primitives for the RepoDesk Code workspace.
//!
//! File-name Quick Open and project-wide text search deliberately share the
//! same short-lived Code Workspace metadata index. Text search still revalidates
//! each file through the editor's path/symlink/security policy before reading it.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::code_workspace::{
    CodeWorkspaceFile, CodeWorkspaceFileStatus, GuardedCodeText, load_code_workspace,
    read_guarded_code_text_from_root,
};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;

const SEARCH_INDEX_TTL: Duration = Duration::from_secs(2);
pub const MAX_QUICK_OPEN_QUERY_CHARS: usize = 256;
pub const MAX_QUICK_OPEN_RESULTS: usize = 100;
pub const MAX_PROJECT_SEARCH_QUERY_CHARS: usize = 256;
pub const MAX_PROJECT_SEARCH_RESULTS: usize = 500;
pub const DEFAULT_PROJECT_SEARCH_RESULTS: usize = 200;
pub const MAX_PROJECT_SEARCH_SCANNED_BYTES: u64 = 64 * 1024 * 1024;
const PROJECT_SEARCH_PREVIEW_CHARS: usize = 220;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeQuickOpenResult {
    pub path: String,
    pub name: String,
    pub language: String,
    pub status: CodeWorkspaceFileStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProjectSearchInput {
    pub query: String,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default = "default_project_search_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProjectSearchMatch {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProjectSearchResult {
    pub query: String,
    pub case_sensitive: bool,
    pub matches: Vec<CodeProjectSearchMatch>,
    pub scanned_files: usize,
    pub scanned_bytes: u64,
    pub skipped_files: usize,
    pub truncated: bool,
    pub workspace_truncated: bool,
}

#[derive(Clone)]
struct SearchIndexEntry {
    observed_at: Instant,
    project: String,
    files: Arc<Vec<CodeWorkspaceFile>>,
    workspace_truncated: bool,
}

#[derive(Clone)]
struct IndexedWorkspace {
    files: Arc<Vec<CodeWorkspaceFile>>,
    workspace_truncated: bool,
}

static SEARCH_INDEX: OnceLock<Mutex<BTreeMap<PathBuf, SearchIndexEntry>>> = OnceLock::new();

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
    let index = indexed_workspace(&project.name, &root)?;
    Ok(rank_quick_open_results(
        index.files.as_ref(),
        query,
        limit.clamp(1, MAX_QUICK_OPEN_RESULTS),
    ))
}

pub fn search_active_code_project(
    input: CodeProjectSearchInput,
) -> RepoDeskResult<CodeProjectSearchResult> {
    let query = validate_project_search_query(&input.query)?;
    let project = get_active_project()?;
    let root = project.path.canonicalize()?;
    let index = indexed_workspace(&project.name, &root)?;
    let matcher = literal_matcher(query, input.case_sensitive)?;
    let limit = input.limit.clamp(1, MAX_PROJECT_SEARCH_RESULTS);

    Ok(search_indexed_workspace(
        index.files.as_ref(),
        index.workspace_truncated,
        query,
        input.case_sensitive,
        limit,
        MAX_PROJECT_SEARCH_SCANNED_BYTES,
        |path| read_guarded_code_text_from_root(&root, path),
        &matcher,
    ))
}

pub fn invalidate_active_quick_open_index() {
    let Ok(project) = get_active_project() else {
        return;
    };
    let Ok(root) = project.path.canonicalize() else {
        return;
    };
    invalidate_search_index(&root);
}

fn default_project_search_limit() -> usize {
    DEFAULT_PROJECT_SEARCH_RESULTS
}

fn validate_project_search_query(query: &str) -> RepoDeskResult<&str> {
    let query = query.trim();
    if query.chars().count() > MAX_PROJECT_SEARCH_QUERY_CHARS {
        return Err(RepoDeskError::Api(format!(
            "Project search query exceeds the {MAX_PROJECT_SEARCH_QUERY_CHARS} character limit"
        )));
    }
    if query.contains(['\0', '\n', '\r']) {
        return Err(RepoDeskError::Api(
            "Project search v1 accepts a single-line literal query".into(),
        ));
    }
    Ok(query)
}

fn literal_matcher(query: &str, case_sensitive: bool) -> RepoDeskResult<Regex> {
    RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .unicode(true)
        .build()
        .map_err(|error| RepoDeskError::Api(format!("Unable to prepare project search: {error}")))
}

fn search_index() -> &'static Mutex<BTreeMap<PathBuf, SearchIndexEntry>> {
    SEARCH_INDEX.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn indexed_workspace(project_name: &str, root: &Path) -> RepoDeskResult<IndexedWorkspace> {
    if let Ok(cache) = search_index().lock()
        && let Some(entry) = cache.get(root)
        && entry.project == project_name
        && entry.observed_at.elapsed() <= SEARCH_INDEX_TTL
    {
        return Ok(IndexedWorkspace {
            files: Arc::clone(&entry.files),
            workspace_truncated: entry.workspace_truncated,
        });
    }

    let snapshot = load_code_workspace(project_name, root)?;
    let workspace_truncated = snapshot.truncated;
    let files = Arc::new(
        snapshot
            .files
            .into_iter()
            .filter(|file| !file.blocked)
            .collect::<Vec<_>>(),
    );

    if let Ok(mut cache) = search_index().lock() {
        cache.insert(
            root.to_path_buf(),
            SearchIndexEntry {
                observed_at: Instant::now(),
                project: project_name.to_string(),
                files: Arc::clone(&files),
                workspace_truncated,
            },
        );
    }

    Ok(IndexedWorkspace {
        files,
        workspace_truncated,
    })
}

fn invalidate_search_index(root: &Path) {
    if let Ok(mut cache) = search_index().lock() {
        cache.remove(root);
    }
}

fn search_indexed_workspace<F>(
    files: &[CodeWorkspaceFile],
    workspace_truncated: bool,
    query: &str,
    case_sensitive: bool,
    limit: usize,
    byte_budget: u64,
    mut read_file: F,
    matcher: &Regex,
) -> CodeProjectSearchResult
where
    F: FnMut(&str) -> RepoDeskResult<GuardedCodeText>,
{
    let mut result = CodeProjectSearchResult {
        query: query.to_string(),
        case_sensitive,
        matches: Vec::new(),
        scanned_files: 0,
        scanned_bytes: 0,
        skipped_files: 0,
        truncated: workspace_truncated,
        workspace_truncated,
    };

    if query.is_empty() {
        return result;
    }

    for file in files {
        if file.blocked
            || file.status == CodeWorkspaceFileStatus::Deleted
            || file.bytes > crate::code_workspace::MAX_EDITABLE_FILE_BYTES
        {
            result.skipped_files += 1;
            continue;
        }

        if file.bytes > 0 && result.scanned_bytes.saturating_add(file.bytes) > byte_budget {
            result.truncated = true;
            break;
        }

        let text = match read_file(&file.path) {
            Ok(text) => text,
            Err(_) => {
                result.skipped_files += 1;
                continue;
            }
        };

        if result.scanned_bytes.saturating_add(text.bytes) > byte_budget {
            result.truncated = true;
            break;
        }

        result.scanned_files += 1;
        result.scanned_bytes += text.bytes;

        for (line_index, line) in text.content.lines().enumerate() {
            for found in matcher.find_iter(line) {
                let start_char = line[..found.start()].chars().count();
                let end_char = line[..found.end()].chars().count();
                result.matches.push(CodeProjectSearchMatch {
                    path: file.path.clone(),
                    line: line_index + 1,
                    column: start_char + 1,
                    end_column: end_char + 1,
                    preview: preview_line(line, start_char, end_char),
                });

                if result.matches.len() >= limit {
                    result.truncated = true;
                    return result;
                }
            }
        }
    }

    result
}

fn preview_line(line: &str, start_char: usize, end_char: usize) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    if chars.len() <= PROJECT_SEARCH_PREVIEW_CHARS {
        return line.to_string();
    }

    let match_len = end_char.saturating_sub(start_char);
    let surrounding = PROJECT_SEARCH_PREVIEW_CHARS.saturating_sub(match_len);
    let before = surrounding / 2;
    let from = start_char.saturating_sub(before);
    let to = (from + PROJECT_SEARCH_PREVIEW_CHARS)
        .max(end_char)
        .min(chars.len());
    let from = to.saturating_sub(PROJECT_SEARCH_PREVIEW_CHARS).min(from);
    let mut preview = chars[from..to].iter().collect::<String>();
    if from > 0 {
        preview.insert(0, '…');
    }
    if to < chars.len() {
        preview.push('…');
    }
    preview
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
        compare_rank(*left_rank, *right_rank).then_with(|| left_file.path.cmp(&right_file.path))
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
    } else {
        let gap = subsequence_gap(&path, query)?;
        (7, gap)
    };

    Some(MatchRank {
        class,
        subsequence_gap,
        changed_penalty,
        path_len: path.chars().count(),
    })
}

fn normalize(value: &str) -> String {
    value.trim().replace('\\', "/").to_lowercase()
}

fn subsequence_gap(text: &str, query: &str) -> Option<usize> {
    let text = text.chars().collect::<Vec<_>>();
    let query = query.chars().collect::<Vec<_>>();
    let mut cursor = 0;
    let mut first = None;
    let mut last = 0;

    for query_char in &query {
        let relative = text
            .get(cursor..)?
            .iter()
            .position(|candidate| candidate == query_char)?;
        let absolute = cursor + relative;
        first.get_or_insert(absolute);
        last = absolute;
        cursor = absolute + 1;
    }

    Some(last.saturating_sub(first.unwrap_or(last)) + 1 - query.len())
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

    fn text(content: &str) -> GuardedCodeText {
        GuardedCodeText {
            bytes: content.len() as u64,
            content: content.to_string(),
        }
    }

    #[test]
    fn quick_open_search_is_not_limited_by_input_position() {
        let mut files = (0..220)
            .map(|index| {
                file(
                    &format!("src/generated/file-{index:03}.rs"),
                    CodeWorkspaceFileStatus::Clean,
                )
            })
            .collect::<Vec<_>>();
        files.push(file(
            "src/important/session_manager.rs",
            CodeWorkspaceFileStatus::Modified,
        ));

        let results = rank_quick_open_results(&files, "session", 50);
        assert_eq!(
            results.first().map(|result| result.path.as_str()),
            Some("src/important/session_manager.rs")
        );
    }

    #[test]
    fn exact_and_filename_matches_beat_loose_subsequences() {
        let files = vec![
            file("src/auth/session.rs", CodeWorkspaceFileStatus::Clean),
            file("src/session.rs", CodeWorkspaceFileStatus::Clean),
            file(
                "src/service/session_registry.rs",
                CodeWorkspaceFileStatus::Modified,
            ),
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
    fn blocked_files_never_surface_in_quick_open() {
        let mut secret = file("config/.env", CodeWorkspaceFileStatus::Modified);
        secret.blocked = true;
        let results = rank_quick_open_results(&[secret], "env", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn quick_open_unicode_search_is_case_insensitive() {
        let files = vec![file(
            "src/Сесія_Користувача.rs",
            CodeWorkspaceFileStatus::Clean,
        )];

        let results = rank_quick_open_results(&files, "СЕСІЯ", 10);
        assert_eq!(results[0].path, "src/Сесія_Користувача.rs");
    }

    #[test]
    fn project_search_reports_line_columns_and_literal_matches() {
        let files = vec![file("src/session.rs", CodeWorkspaceFileStatus::Clean)];
        let matcher = literal_matcher("session.*id", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "session.*id",
            false,
            20,
            1024,
            |_| Ok(text("let session.*id = 1;\nlet sessionXXid = 2;\n")),
            &matcher,
        );

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].line, 1);
        assert_eq!(result.matches[0].column, 5);
        assert_eq!(result.matches[0].end_column, 16);
        assert!(!result.truncated);
    }

    #[test]
    fn project_search_is_unicode_case_insensitive_by_default() {
        let files = vec![file("src/user.rs", CodeWorkspaceFileStatus::Clean)];
        let matcher = literal_matcher("СЕСІЯ", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "СЕСІЯ",
            false,
            20,
            1024,
            |_| Ok(text("let value = \"сесія користувача\";")),
            &matcher,
        );
        assert_eq!(result.matches.len(), 1);
    }

    #[test]
    fn project_search_case_sensitive_mode_is_respected() {
        let files = vec![file("src/user.rs", CodeWorkspaceFileStatus::Clean)];
        let matcher = literal_matcher("Session", true).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "Session",
            true,
            20,
            1024,
            |_| Ok(text("session Session")),
            &matcher,
        );
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].column, 9);
    }

    #[test]
    fn project_search_skips_blocked_and_unreadable_files() {
        let mut blocked = file(".env", CodeWorkspaceFileStatus::Modified);
        blocked.blocked = true;
        let files = vec![
            blocked,
            file("src/binary.rs", CodeWorkspaceFileStatus::Clean),
            file("src/ok.rs", CodeWorkspaceFileStatus::Clean),
        ];
        let matcher = literal_matcher("needle", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "needle",
            false,
            20,
            1024,
            |path| {
                if path.ends_with("binary.rs") {
                    return Err(RepoDeskError::Api("binary".into()));
                }
                Ok(text("needle"))
            },
            &matcher,
        );

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].path, "src/ok.rs");
        assert_eq!(result.skipped_files, 2);
    }

    #[test]
    fn project_search_result_limit_is_explicitly_truncated() {
        let files = vec![file("src/many.rs", CodeWorkspaceFileStatus::Clean)];
        let matcher = literal_matcher("x", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "x",
            false,
            2,
            1024,
            |_| Ok(text("x x x")),
            &matcher,
        );
        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
    }

    #[test]
    fn project_search_scan_budget_is_explicitly_truncated() {
        let mut first = file("src/one.rs", CodeWorkspaceFileStatus::Clean);
        first.bytes = 4;
        let mut second = file("src/two.rs", CodeWorkspaceFileStatus::Clean);
        second.bytes = 4;
        let files = vec![first, second];
        let matcher = literal_matcher("x", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            false,
            "x",
            false,
            20,
            6,
            |_| Ok(text("xxxx")),
            &matcher,
        );
        assert_eq!(result.scanned_files, 1);
        assert_eq!(result.scanned_bytes, 4);
        assert!(result.truncated);
    }

    #[test]
    fn project_search_propagates_workspace_index_truncation() {
        let files = vec![file("src/one.rs", CodeWorkspaceFileStatus::Clean)];
        let matcher = literal_matcher("missing", false).unwrap();
        let result = search_indexed_workspace(
            &files,
            true,
            "missing",
            false,
            20,
            1024,
            |_| Ok(text("nothing here")),
            &matcher,
        );
        assert!(result.workspace_truncated);
        assert!(result.truncated);
    }
}
