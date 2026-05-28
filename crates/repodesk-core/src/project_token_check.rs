use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;
use crate::tokens::{TokenStatus, estimate_text};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTokenEstimate {
    pub path: String,
    pub bytes: u64,
    pub estimated_tokens: usize,
    pub status: String,
}

pub fn get_project_file_token_estimates() -> RepoDeskResult<Vec<FileTokenEstimate>> {
    let project = get_active_project()?;
    let mut estimates = Vec::new();

    if !project.path.exists() {
        return Ok(estimates);
    }

    scan_directory(
        &project.path,
        &project.path,
        &project.context_ignore,
        &mut estimates,
    )?;

    // Sort by estimated tokens descending
    estimates.sort_by(|a, b| b.estimated_tokens.cmp(&a.estimated_tokens));

    // Cap at top 250 files to prevent rendering bloat
    if estimates.len() > 250 {
        estimates.truncate(250);
    }

    Ok(estimates)
}

fn scan_directory(
    root: &Path,
    dir: &Path,
    ignore_list: &[String],
    estimates: &mut Vec<FileTokenEstimate>,
) -> RepoDeskResult<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        if is_ignored(&relative_path, ignore_list) {
            continue;
        }

        if path.is_dir() {
            // Recurse, ignoring errors for system directories
            let _ = scan_directory(root, &path, ignore_list, estimates);
        } else if path.is_file() {
            if let Some(estimate) = estimate_single_file(&path, &relative_path) {
                estimates.push(estimate);
            }
        }
    }

    Ok(())
}

fn is_ignored(relative_path: &str, ignore_list: &[String]) -> bool {
    let lower_path = relative_path.to_lowercase();
    let normalized = relative_path.replace('\\', "/");

    // Standard high-risk folders
    if normalized.contains(".git/")
        || normalized.starts_with(".git")
        || normalized.contains("node_modules/")
        || normalized.starts_with("node_modules")
        || normalized.contains("target/")
        || normalized.starts_with("target")
        || normalized.contains(".repodesk-debug/")
        || normalized.starts_with(".repodesk-debug")
    {
        return true;
    }

    for ignore_item in ignore_list {
        let trimmed = ignore_item.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower_ignore = trimmed.to_lowercase();

        if lower_ignore.starts_with("*.") {
            let suffix = &lower_ignore[1..]; // e.g. ".pdf" or ".html"
            if lower_path.ends_with(suffix) {
                return true;
            }
        } else if lower_path.contains(&lower_ignore) || normalized.contains(trimmed) {
            return true;
        }
    }

    false
}

fn estimate_single_file(path: &Path, relative_path: &str) -> Option<FileTokenEstimate> {
    let metadata = fs::metadata(path).ok()?;
    let bytes = metadata.len();

    // Skip extremely large files or binary indicators
    if bytes > 10 * 1024 * 1024 {
        return None;
    }

    if !is_text_extension(path) {
        return None;
    }

    let estimated_tokens = match fs::read_to_string(path) {
        Ok(content) => estimate_text(&content).estimated_tokens,
        Err(_) => {
            // Fallback to byte-based estimation if read fails (e.g. invalid UTF-8)
            (bytes as usize / 3).max(1)
        }
    };

    let status = match estimated_tokens {
        0..=3_000 => TokenStatus::Ok.as_label().to_string(),
        3_001..=8_000 => TokenStatus::Medium.as_label().to_string(),
        8_001..=20_000 => TokenStatus::Large.as_label().to_string(),
        _ => TokenStatus::TooLarge.as_label().to_string(),
    };

    Some(FileTokenEstimate {
        path: relative_path.to_string(),
        bytes,
        estimated_tokens,
        status,
    })
}

fn is_text_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|val| val.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        ext.as_str(),
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
            | "css"
            | "html"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "java"
            | "swift"
            | "kt"
            | "gradle"
            | "properties"
    )
}
