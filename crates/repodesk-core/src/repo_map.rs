use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;

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

pub fn build_repo_map() -> RepoDeskResult<RepoMap> {
    let project = get_active_project()?;
    let mut scanner = RepoScanner::new(project.name.clone(), project.path.clone());
    scanner.scan_dir(&project.path, 0)?;
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
        if depth > 8 {
            self.skipped_dirs += 1;
            return Ok(());
        }

        self.dirs_scanned += 1;

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                if should_skip_dir(&name) {
                    self.skipped_dirs += 1;
                    continue;
                }

                self.scan_dir(&path, depth + 1)?;
            } else if path.is_file() {
                self.scan_file(&path)?;
            }

            if self.files_scanned >= 2_000 {
                break;
            }
        }

        Ok(())
    }

    fn scan_file(&mut self, path: &Path) -> RepoDeskResult<()> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => return Ok(()),
        };

        let bytes = metadata.len();
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

        if bytes > 80_000 {
            self.hotspots.push(FileHotspot {
                path: relative,
                bytes,
                reason: "large file; avoid sending to paid agents without filtering".to_string(),
            });
        }

        Ok(())
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

        languages.sort_by(|a, b| b.bytes.cmp(&a.bytes));

        self.hotspots.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        self.hotspots.truncate(15);
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
