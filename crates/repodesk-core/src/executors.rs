//! Safe command specifications for coding-agent executors.
//!
//! This module deliberately does not execute agents. It defines canonical
//! executor ids, passive PATH availability, and argv previews that future
//! executor code can feed to `std::process::Command` directly. No `sh -c`.

use std::env;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentSpec {
    pub id: String,
    pub label: String,
    pub binary: String,
    pub requires_paid_account: bool,
    pub supports_writes: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutorAvailability {
    pub executor_id: String,
    pub label: String,
    pub binary: String,
    pub available: bool,
    pub executable_path: Option<String>,
    pub status: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentCommandSpec {
    pub executor_id: String,
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    /// The bounded prompt is supplied via stdin in the future executor. Keeping
    /// it out of argv prevents accidental shell quoting or command-history leaks.
    pub stdin_required: bool,
    pub cwd_required: bool,
    pub writes_allowed: bool,
    pub command_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentHandoff {
    pub availability: ExecutorAvailability,
    pub command: CodingAgentCommandSpec,
    pub notes: Vec<String>,
}

pub fn coding_agent_specs() -> Vec<CodingAgentSpec> {
    vec![
        CodingAgentSpec {
            id: "codex_cli".to_string(),
            label: "Codex CLI".to_string(),
            binary: "codex".to_string(),
            requires_paid_account: true,
            supports_writes: true,
            notes: vec![
                "Use only after context, budget, safety scan, and judge verdict.".to_string(),
                "Prompt transport is stdin; future execution must use argv, never sh -c."
                    .to_string(),
            ],
        },
        CodingAgentSpec {
            id: "claude_code_cli".to_string(),
            label: "Claude Code CLI".to_string(),
            binary: "claude".to_string(),
            requires_paid_account: true,
            supports_writes: true,
            notes: vec![
                "Use only after context, budget, safety scan, and judge verdict.".to_string(),
                "Prompt transport is stdin; future execution must use argv, never sh -c."
                    .to_string(),
            ],
        },
    ]
}

pub fn canonical_coding_agent_id(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" | "codex_cli" => Some("codex_cli"),
        "claude" | "claude_code" | "claude_code_cli" => Some("claude_code_cli"),
        _ => None,
    }
}

pub fn coding_agent_spec(value: &str) -> RepoDeskResult<CodingAgentSpec> {
    let Some(id) = canonical_coding_agent_id(value) else {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("unknown coding-agent executor '{value}'"),
        });
    };
    coding_agent_specs()
        .into_iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: format!("coding-agent executor '{id}' is not registered"),
        })
}

pub fn coding_agent_availability(value: &str) -> RepoDeskResult<ExecutorAvailability> {
    let spec = coding_agent_spec(value)?;
    let executable_path = find_executable(&spec.binary);
    let available = executable_path.is_some();
    let mut notes = spec.notes.clone();
    if available {
        notes.push("Executable was found on PATH by passive lookup.".to_string());
    } else {
        notes.push("Executable was not found on PATH; RepoDesk will not launch it.".to_string());
    }

    Ok(ExecutorAvailability {
        executor_id: spec.id,
        label: spec.label,
        binary: spec.binary,
        available,
        executable_path: executable_path.map(|path| path.display().to_string()),
        status: if available { "available" } else { "missing" }.to_string(),
        notes,
    })
}

pub fn build_coding_agent_command(
    value: &str,
    writes_allowed: bool,
) -> RepoDeskResult<CodingAgentCommandSpec> {
    let spec = coding_agent_spec(value)?;
    let command = CodingAgentCommandSpec {
        executor_id: spec.id,
        label: spec.label,
        program: spec.binary,
        args: Vec::new(),
        stdin_required: true,
        cwd_required: true,
        writes_allowed: writes_allowed && spec.supports_writes,
        command_preview: String::new(),
    };
    validate_command_spec(&command)?;
    Ok(CodingAgentCommandSpec {
        command_preview: format_command_preview(&command),
        ..command
    })
}

pub fn preview_coding_agent_handoff(
    value: &str,
    writes_allowed: bool,
) -> RepoDeskResult<CodingAgentHandoff> {
    let availability = coding_agent_availability(value)?;
    let command = build_coding_agent_command(value, writes_allowed)?;
    let mut notes = Vec::new();
    notes.push(format!(
        "executor {} is prepared as argv only; no shell will be used",
        command.executor_id
    ));
    notes.push(format!("command preview: {}", command.command_preview));
    notes.push(
        "bounded prompt transport: stdin; prompt content is not included in the command line"
            .to_string(),
    );
    notes.push("automatic CLI execution is not enabled in this PR".to_string());
    notes.extend(availability.notes.clone());

    Ok(CodingAgentHandoff {
        availability,
        command,
        notes,
    })
}

pub fn validate_command_spec(command: &CodingAgentCommandSpec) -> RepoDeskResult<()> {
    validate_token("program", &command.program)?;
    for arg in &command.args {
        validate_token("argument", arg)?;
    }
    Ok(())
}

fn validate_token(label: &str, value: &str) -> RepoDeskResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RepoDeskError::SandboxBlocked {
            command: value.to_string(),
            reason: format!("{label} is empty"),
        });
    }
    if trimmed.chars().any(is_shell_metachar) {
        return Err(RepoDeskError::SandboxBlocked {
            command: value.to_string(),
            reason: format!("{label} contains a shell metacharacter"),
        });
    }
    Ok(())
}

fn is_shell_metachar(ch: char) -> bool {
    matches!(ch, ';' | '&' | '|' | '<' | '>' | '$' | '`' | '\n' | '\r')
}

fn format_command_preview(command: &CodingAgentCommandSpec) -> String {
    let mut parts = Vec::with_capacity(command.args.len() + 2);
    parts.push(command.program.clone());
    parts.extend(command.args.iter().cloned());
    if command.stdin_required {
        parts.push("[stdin: bounded prompt]".to_string());
    }
    parts.join(" ")
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        env::var_os("PATHEXT")
            .map(|value| {
                env::split_paths(&value)
                    .map(|p| p.display().to_string())
                    .collect()
            })
            .unwrap_or_else(|| vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()])
    } else {
        vec![String::new()]
    };

    for dir in env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{}{}", name, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_coding_agent_aliases() {
        assert_eq!(canonical_coding_agent_id("codex"), Some("codex_cli"));
        assert_eq!(canonical_coding_agent_id("claude"), Some("claude_code_cli"));
        assert_eq!(canonical_coding_agent_id("openai_api"), None);
    }

    #[test]
    fn command_preview_uses_argv_and_stdin() {
        let command = build_coding_agent_command("codex_cli", true).unwrap();
        assert_eq!(command.program, "codex");
        assert!(command.args.is_empty());
        assert!(command.stdin_required);
        assert!(command.writes_allowed);
        assert_eq!(command.command_preview, "codex [stdin: bounded prompt]");
    }

    #[test]
    fn command_spec_rejects_shell_metacharacters() {
        let command = CodingAgentCommandSpec {
            executor_id: "codex_cli".to_string(),
            label: "Codex CLI".to_string(),
            program: "codex;rm".to_string(),
            args: Vec::new(),
            stdin_required: true,
            cwd_required: true,
            writes_allowed: true,
            command_preview: String::new(),
        };
        assert!(matches!(
            validate_command_spec(&command),
            Err(RepoDeskError::SandboxBlocked { .. })
        ));
    }

    #[test]
    fn handoff_does_not_require_executable_to_build_preview() {
        let handoff = preview_coding_agent_handoff("claude_code_cli", false).unwrap();
        assert_eq!(handoff.command.executor_id, "claude_code_cli");
        assert!(!handoff.command.writes_allowed);
        assert!(
            handoff
                .notes
                .iter()
                .any(|note| note.contains("automatic CLI execution is not enabled"))
        );
    }
}
