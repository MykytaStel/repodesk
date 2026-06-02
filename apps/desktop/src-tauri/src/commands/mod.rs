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
pub(crate) fn run_cli(args: &[String]) -> CommandResult {
    if args.is_empty() {
        return CommandResult {
            ok: false,
            command: "repodesk".into(),
            stdout: "".into(),
            stderr: "Empty command arguments".into(),
            exit_code: Some(1),
        };
    }

    let allowed_subcommands = [
        "init",
        "project",
        "task",
        "context",
        "prompt",
        "checks",
        "agents",
        "guard",
        "brain",
        "capabilities",
        "security",
        "doctor",
        "ai",
        "ui",
        "desktop",
        "cost",
        "safety",
        "peripherals",
        "workflow",
        "events",
        "judge",
        "access",
        "modules",
        "session",
        "inspect",
        "smartcontext",
        "receipts",
        "git",
        "dashboard",
        "runtime",
        "sandbox",
        "tokens",
        "budget",
    ];
    let sub = args[0].to_lowercase();
    if !allowed_subcommands.contains(&sub.as_str()) {
        return CommandResult {
            ok: false,
            command: format!("repodesk {}", args.join(" ")),
            stdout: "".into(),
            stderr: format!(
                "Blocked: Subcommand '{}' is not registered or allowed.",
                sub
            ),
            exit_code: Some(1),
        };
    }

    use clap::Parser;
    let mut cli_args = vec!["repodesk".to_string()];
    cli_args.extend(args.iter().cloned());
    let parsed = repodesk_cli::cli::Cli::try_parse_from(cli_args);

    let temp_dir = std::env::temp_dir();
    let now = now_ms();
    let stdout_path = temp_dir.join(format!("repodesk-stdout-{now}.log"));
    let stderr_path = temp_dir.join(format!("repodesk-stderr-{now}.log"));

    // Set up stdout/stderr redirection
    let stdout_guard = stdio_override::StdoutOverride::from_file(&stdout_path).ok();
    let stderr_guard = stdio_override::StderrOverride::from_file(&stderr_path).ok();

    let (ok, mut stdout, mut stderr) = match parsed {
        Ok(cli) => match repodesk_cli::commands::dispatch(cli) {
            Ok(_) => (true, String::new(), String::new()),
            Err(error) => (false, String::new(), error.to_string()),
        },
        Err(e) => (false, String::new(), e.to_string()),
    };

    // Drop guards to flush standard output and close override files
    drop(stdout_guard);
    drop(stderr_guard);

    // Read the logs
    if let Ok(content) = std::fs::read_to_string(&stdout_path)
        && !content.is_empty()
    {
        stdout = content;
    }
    if let Ok(content) = std::fs::read_to_string(&stderr_path)
        && !content.is_empty()
    {
        if stderr.is_empty() {
            stderr = content;
        } else {
            stderr = format!("{stderr}\n{content}");
        }
    }

    // Clean up temp files
    let _ = std::fs::remove_file(&stdout_path);
    let _ = std::fs::remove_file(&stderr_path);

    CommandResult {
        ok,
        command: format!("repodesk {}", args.join(" ")),
        stdout,
        stderr,
        exit_code: if ok { Some(0) } else { Some(1) },
    }
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

pub use repodesk_core::workflow::{has_block_signal, has_warn_signal};
