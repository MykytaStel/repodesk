use serde::{Deserialize, Serialize};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod diagnostic;
pub mod journal;
pub mod memory;
pub mod models;
pub mod project;
pub mod routing;
pub mod settings;
pub mod system;
pub mod task;
pub mod tokens;
pub mod workflow;

pub use models::*;
pub use routing::*;
pub use system::*;
pub use tokens::*;
pub use workflow::*;

pub use diagnostic::*;
pub use journal::*;
pub use memory::*;
pub use project::*;
pub use settings::*;
pub use task::*;

pub use repodesk_core::workflow::CommandResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAddInput {
    pub name: String,
    pub path: String,
    pub project_type: String,
    pub main_language: Option<String>,
}

pub(crate) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(crate) fn workspace_root() -> PathBuf {
    let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for _ in 0..8 {
        if current.join("Cargo.toml").exists() && current.join("crates/repodesk-cli").exists() {
            return current;
        }

        if !current.pop() {
            break;
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(workspace_root)
}

pub(crate) fn history_file() -> PathBuf {
    home_dir()
        .join(".repodesk")
        .join("desktop")
        .join("action-history.jsonl")
}

pub(crate) fn truncate_text(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push_str("\n\n[RepoDesk truncated output to keep the UI responsive]");
    truncated
}

pub(crate) fn validate_short_id(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    if trimmed.len() > 80 {
        return Err(format!("{label} is too long"));
    }

    let safe = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'));

    if !safe {
        return Err(format!(
            "{label} may only contain letters, numbers, dash, underscore, dot or slash"
        ));
    }

    Ok(())
}

pub(crate) fn validate_text(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    if trimmed.len() > max_len {
        return Err(format!("{label} is too long"));
    }

    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(format!("{label} contains unsupported characters"));
    }

    Ok(())
}

pub(crate) fn validate_path(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Path cannot be empty".into());
    }

    if trimmed.len() > 512 {
        return Err("Path is too long".into());
    }

    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("Path contains unsupported characters".into());
    }

    Ok(())
}

// DEPRECATED: Replace in-process CLI dispatch with a proper service layer instead of calling CLI commands from desktop.
// Calling this does not capture standard `print!` output.
pub(crate) fn run_cli(args: &[String]) -> CommandResult {
    use clap::Parser;
    let mut cli_args = vec!["repodesk".to_string()];
    cli_args.extend(args.iter().cloned());
    let parsed = repodesk_cli::cli::Cli::try_parse_from(cli_args);

    let (ok, stdout, stderr) = match parsed {
        Ok(cli) => {
            // Because CLI commands typically print to stdout/stderr using print!/println!,
            // let's use standard thread redirection or execute dispatch directly.
            // Wait, we can redirect or capture print outputs if we wrap dispatch.
            // But since this is a desktop app calling its own Rust core logic, we can also execute it.
            // Let's call dispatch and return stdout/stderr if captured.
            // Note: Since standard library doesn't easily capture println! in stable rust without a helper,
            // we can intercept the most common commands or run dispatch.
            // Let's compile and see if we can get basic dispatch working.
            match repodesk_cli::commands::dispatch(cli) {
                Ok(_) => (
                    true,
                    "Command executed successfully in-process.".to_string(),
                    String::new(),
                ),
                Err(error) => (false, String::new(), error.to_string()),
            }
        }
        Err(e) => (false, String::new(), e.to_string()),
    };

    CommandResult {
        ok,
        command: format!("repodesk {}", args.join(" ")),
        stdout,
        stderr,
        exit_code: if ok { Some(0) } else { Some(1) },
    }
}

pub(crate) fn run_cli_str(args: &[&str]) -> CommandResult {
    let owned = args
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    run_cli(&owned)
}

pub(crate) fn validate_model_name(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if trimmed.len() > 160 || trimmed.contains('\0') || trimmed.contains('\n') {
        return Err(format!("{label} is not safe"));
    }

    let safe = trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '@' | '+')
    });

    if !safe {
        return Err(format!("{label} contains unsupported characters"));
    }

    Ok(())
}

pub(crate) fn validate_optional_notes(value: &Option<String>) -> Result<(), String> {
    if let Some(notes) = value {
        if notes.len() > 1_000 || notes.contains('\0') {
            return Err("Notes are too long or unsafe".into());
        }

        let lower = notes.to_lowercase();
        if notes.contains("-----BEGIN") || lower.contains("api_key") || lower.contains("token=") {
            return Err("Notes must not contain secrets".into());
        }
    }

    Ok(())
}

pub(crate) fn has_block_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("block") || text.contains("secret") || text.contains("private key")
}

pub(crate) fn has_warn_signal(result: &CommandResult) -> bool {
    let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    text.contains("warn") || text.contains("warning") || text.contains("risk")
}
