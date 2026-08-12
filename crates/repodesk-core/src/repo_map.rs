use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::projects::get_active_project;

const MAX_SCAN_DEPTH: usize = 8;
const MAX_FILES_SCANNED: usize = 2_000;
const HOTSPOT_BYTE_LIMIT: u64 = 80_000;
const MAX_HOTSPOTS: usize = 15;
const MAX_CONTEXT_LANGUAGES: usize = 10;
const MAX_CONTEXT_IMPORTANT_FILES: usize = 20;
const MAX_CONTEXT_HOTSPOTS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMap {
    pub project_name: String,
    pub project_path: PathBuf,
    pub files_scanned: usize,
    pub dirs_scanned: usize,
    pub skipped_dirs: usize,
    pub total_bytes: u64,
    pub languages: Vec<LanguageStat>,
    pub hotspots: Vec<FileHotspot>,
    pub important_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStat {
    pub label: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHotspot {
    pub path: String,
    pub bytes: u64,
    pub reason: String,
}

/// Async adapter for UI/CLI callers. The scanner itself is synchronous so the
/// canonical context builder can reuse exactly the same algorithm; async callers
/// run that bounded local filesystem work on Tokio's blocking pool.
pub async fn build_repo_map() -> RepoDeskResult<RepoMap> {
    let project = get_active_project()?;
    let name = project.name;
    let path = project.path;
    tokio::task::spawn_blocking(move || build_repo_map_for(name, path))
        .await
        .map_err(|error| RepoDeskError::Api(format!("repository map worker failed: {error}")))?
}

pub fn build_repo_map_sync() -> RepoDeskResult<RepoMap> {
    let project = get_active_project()?;
    build_repo_map_for(project.name, project.path)
}

/// Deterministic repository map for a resolved project.
///
/// Directory entries are sorted before traversal, so the 2k-file safety cap
/// produces the same bounded map across filesystems instead of depending on
/// `read_dir` iteration order. Symlinks are not followed.
pub fn build_repo_map_for(project_name: String, project_path: PathBuf) -> RepoDeskResult<RepoMap> {
    let mut scanner = RepoScanner::new(project_name, project_path.clone());
    scanner.scan_dir(&project_path, 0)?;
    Ok(scanner.finish())
}

pub fn format_repo_map(map: &RepoMap) -> String {
    let mut output = String::new();

    output.push_str("Repository map:\n\n");
    output.push_str(&format!("Project: {}\n", map.project_name));
    output.push_str(&format!("Path: {}\n", map.project_path.display()));
    output.push_str(&format!("Files scanned: {}\n", map.files_scanned));
    output.push_str(&format!("Directories scanned: {}\n", map.dirs_scanned));
    output.push_str(&format!("Skipped directories: {}\n", map.skipped_dirs));
    output.push_str(&format!("Total bytes: {}\n\n", map.total_bytes));

    output.push_str("Languages / file groups:\n");
    if map.languages.is_empty() {
        output.push_str("  - none\n");
    } else {
        for item in &map.languages {
            output.push_str(&format!(
                "  - {}: files={}, bytes={}\n",
                item.label, item.files, item.bytes
            ));
        }
    }

    output.push_str("\nImportant files:\n");
    if map.important_files.is_empty() {
        output.push_str("  - none detected\n");
    } else {
        for item in &map.important_files {
            output.push_str(&format!("  - {item}\n"));
        }
    }

    output.push_str("\nHotspots:\n");
    output.push_str(&format_hotspots(map));

    output
}

/// Compact structural projection intended for the Context Pipeline.
///
/// It contains repository metadata and paths only — never source-file bodies.
/// All lists are deterministically bounded so the source remains cheap enough to
/// compete for context budget rather than acting as an implicit repo dump.
pub fn format_repo_context(map: &RepoMap) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Files scanned: {}{}\nDirectories scanned: {}\nSkipped directories: {}\nApproximate bytes: {}\n",
        map.files_scanned,
        if map.files_scanned >= MAX_FILES_SCANNED {
            " (scan cap reached)"
        } else {
            ""
        },
        map.dirs_scanned,
        map.skipped_dirs,
        map.total_bytes,
    ));

    output.push_str("\nPrimary languages / file groups:\n");
    if map.languages.is_empty() {
        output.push_str("- none detected\n");
    } else {
        for language in map.languages.iter().take(MAX_CONTEXT_LANGUAGES) {
            output.push_str(&format!(
                "- {}: {} files, {} bytes\n",
                language.label, language.files, language.bytes
            ));
        }
    }

    output.push_str("\nImportant structural files:\n");
    if map.important_files.is_empty() {
        output.push_str("- none detected\n");
    } else {
        for path in map.important_files.iter().take(MAX_CONTEXT_IMPORTANT_FILES) {
            output.push_str(&format!("- `{path}`\n"));
        }
    }

    output.push_str("\nLarge-file hotspots:\n");
    if map.hotspots.is_empty() {
        output.push_str("- none detected\n");
    } else {
        for hotspot in map.hotspots.iter().take(MAX_CONTEXT_HOTSPOTS) {
            output.push_str(&format!(
                "- `{}`: {} bytes — {}\n",
                hotspot.path, hotspot.bytes, hotspot.reason
            ));
        }
    }

    output
}

pub fn format_hotspots(map: &RepoMap) -> String {
    if map.hotspots.is_empty() {
        return "  - no hotspots detected\n".to_string();
    }

    map.hotspots
        .iter()
        .map(|item| format!("  - {} ({} bytes): {}", item.path, item.bytes, item.reason))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

struct RepoScanner {
    project_name: String,
    project_path: PathBuf,
    files_scanned: usize,
    dirs_scanned: usize,
    skipped_dirs: usize,
    total_bytes: u64,
    languages: BTreeMap<String, (usize, u64)>,
    hotspots: Vec<FileHotspot>,
    important_files: Vec<String>,
}

impl RepoScanner {
    fn new(project_name: String, project_path: PathBuf) -> Self {
        Self {
            project_name,
            project_path,
            files_scanned: 0,
            dirs_scanned: 0,
            skipped_dirs: 0,
            total_bytes: 0,
            languages: BTreeMap::new(),
            hotspots: Vec::new(),
            important_files: Vec::new(),
        }
    }

    fn scan_dir(&mut self, dir: &Path, depth: usize) -> RepoDeskResult<()> {
        if self.files_scanned >= MAX_FILES_SCANNED {
            return Ok(());
        }
        if depth > MAX_SCAN_DEPTH {
            self.skipped_dirs += 1;
            return Ok(());
        }

        self.dirs_scanned += 1;

        let read_dir = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };
        let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            if self.files_scanned >= MAX_FILES_SCANNED {
                break;
            }

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            // Repository structure must never escape the project through a
            // symlink. The map is evidence about this checkout, not arbitrary
            // filesystem reachability.
            if file_type.is_symlink() {
                if path.is_dir() {
                    self.skipped_dirs += 1;
                }
                continue;
            }

            if file_type.is_dir() {
                if should_skip_dir(&name) {
                    self.skipped_dirs += 1;
                    continue;
                }
                self.scan_dir(&path, depth + 1)?;
            } else if file_type.is_file() {
                let bytes = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                self.scan_file(&path, bytes);
            }
        }

        Ok(())
    }

    fn scan_file(&mut self, path: &Path, bytes: u64) {
        let relative = relative_path(&self.project_path, path);

        self.files_scanned += 1;
        self.total_bytes += bytes;

        let label = language_label(path);
        let entry = self.languages.entry(label).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += bytes;

        if is_important_file(&relative) {
            self.important_files.push(relative.clone());
        }

        if bytes > HOTSPOT_BYTE_LIMIT {
            self.hotspots.push(FileHotspot {
                path: relative,
                bytes,
                reason: "large file; avoid sending to paid agents without filtering".to_string(),
            });
        }
    }

    fn finish(mut self) -> RepoMap {
        let mut languages = self
            .languages
            .into_iter()
            .map(|(label, (files, bytes))| LanguageStat {
                label,
                files,
                bytes,
            })
            .collect::<Vec<_>>();

        languages.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.label.cmp(&right.label))
        });

        self.hotspots.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.path.cmp(&right.path))
        });
        self.hotspots.truncate(MAX_HOTSPOTS);
        self.important_files.sort();
        self.important_files.dedup();

        RepoMap {
            project_name: self.project_name,
            project_path: self.project_path,
            files_scanned: self.files_scanned,
            dirs_scanned: self.dirs_scanned,
            skipped_dirs: self.skipped_dirs,
            total_bytes: self.total_bytes,
            languages,
            hotspots: self.hotspots,
            important_files: self.important_files,
        }
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "node_modules"
            | "dist"
            | "build"
            | ".next"
            | ".turbo"
            | "coverage"
            | ".idea"
            | ".vscode"
    )
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn language_label(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "rs" => "rust".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "js" | "jsx" => "javascript".to_string(),
        "py" => "python".to_string(),
        "md" | "mdx" => "markdown".to_string(),
        "toml" => "toml".to_string(),
        "json" => "json".to_string(),
        "yml" | "yaml" => "yaml".to_string(),
        "sh" => "shell".to_string(),
        "css" => "css".to_string(),
        "html" => "html".to_string(),
        "" => "no-extension".to_string(),
        other => other.to_string(),
    }
}

fn is_important_file(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "README.md"
            | "AGENTS.md"
            | "CLAUDE.md"
            | "package.json"
            | "pnpm-lock.yaml"
            | "tsconfig.json"
            | "vite.config.ts"
            | "tauri.conf.json"
    ) || path.ends_with("/Cargo.toml")
        || path.ends_with("/package.json")
        || path.ends_with("/main.rs")
        || path.ends_with("/lib.rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_projection_is_bounded_and_structural() {
        let map = RepoMap {
            project_name: "demo".into(),
            project_path: PathBuf::from("/tmp/demo"),
            files_scanned: 2_000,
            dirs_scanned: 50,
            skipped_dirs: 3,
            total_bytes: 123_456,
            languages: (0..20)
                .map(|index| LanguageStat {
                    label: format!("lang-{index:02}"),
                    files: index + 1,
                    bytes: 20_000 - index as u64,
                })
                .collect(),
            hotspots: (0..15)
                .map(|index| FileHotspot {
                    path: format!("src/hot-{index:02}.bin"),
                    bytes: 90_000 + index as u64,
                    reason: "large".into(),
                })
                .collect(),
            important_files: (0..30)
                .map(|index| format!("crate-{index:02}/Cargo.toml"))
                .collect(),
        };

        let context = format_repo_context(&map);
        assert!(context.contains("scan cap reached"));
        assert!(context.contains("lang-09"));
        assert!(!context.contains("lang-10"));
        assert!(context.contains("crate-19/Cargo.toml"));
        assert!(!context.contains("crate-20/Cargo.toml"));
        assert!(context.contains("src/hot-09.bin"));
        assert!(!context.contains("src/hot-10.bin"));
    }

    #[test]
    fn finish_uses_stable_tie_breakers() {
        let mut scanner = RepoScanner::new("demo".into(), PathBuf::from("/tmp/demo"));
        scanner.languages.insert("zeta".into(), (1, 10));
        scanner.languages.insert("alpha".into(), (1, 10));
        scanner.hotspots = vec![
            FileHotspot {
                path: "z".into(),
                bytes: 100,
                reason: "x".into(),
            },
            FileHotspot {
                path: "a".into(),
                bytes: 100,
                reason: "x".into(),
            },
        ];
        let map = scanner.finish();
        assert_eq!(map.languages[0].label, "alpha");
        assert_eq!(map.hotspots[0].path, "a");
    }
}
