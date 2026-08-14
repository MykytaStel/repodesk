//! Safe command specifications and execution facade for coding-agent executors.
//!
//! Registry/command policy stays here; authentication, process probes, changeset
//! capture, and bounded runtime execution live in focused submodules. Executor
//! commands are argv-only and launch only after explicit orchestrator approval.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::git_workspace::GitFileChange;

mod auth;
mod changeset;
mod process;
mod runtime;

use auth::{detect_authentication, home_dir};
use process::probe_version;

/// Environment variable names forwarded to a coding-agent subprocess. The child
/// is otherwise started with a cleared environment, so RepoDesk's own secrets
/// (provider API keys, cloud creds, DB URLs, …) never leak into it. Agents
/// authenticate via their own config under `HOME`, not via these vars.
const FORWARDED_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "LANG",
    "LOGNAME",
    "USER",
    "SHELL",
    "PWD",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
    // Windows equivalents so agents can locate their config/runtime there too.
    "SystemRoot",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PATHEXT",
];

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
    /// Version string reported by the CLI's own `--version`, when probed.
    /// `None` means "not probed" (passive PATH lookup only) or "probe failed".
    #[serde(default)]
    pub version: Option<String>,
    /// Tri-state local authentication: `Some(true)`/`Some(false)` only when a
    /// supported, non-destructive status check is available; `None` is the
    /// honest default — RepoDesk never parses undocumented credential files.
    #[serde(default)]
    pub authenticated: Option<bool>,
    /// Human-facing auth state derived from a documented CLI status command when
    /// available, or from conservative local artifact existence as a fallback.
    #[serde(default)]
    pub auth_status: ExecutorAuthStatus,
    /// The probe used to derive [`auth_status`](Self::auth_status).
    #[serde(default)]
    pub auth_source: Option<String>,
    /// Sanitized non-secret status detail. Never includes account email, org id,
    /// tokens, or credential-file contents.
    #[serde(default)]
    pub auth_detail: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorAuthStatus {
    Authenticated,
    Unauthenticated,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentCommandSpec {
    pub executor_id: String,
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    /// The bounded prompt is supplied via stdin. Keeping it out of argv prevents
    /// accidental shell quoting or command-history leaks.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentExecution {
    pub executor_id: String,
    pub command_preview: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    /// Secret-redacted stdout retained under the runtime's hard record budget.
    pub stdout: String,
    /// Secret-redacted stderr retained under the runtime's hard record budget.
    pub stderr: String,
    /// Bounded raw stdout diagnostic prefix persisted with restrictive perms.
    pub stdout_path: String,
    /// Bounded raw stderr diagnostic prefix persisted with restrictive perms.
    pub stderr_path: String,
    /// Whether the in-record stdout/stderr were truncated to their hard budgets.
    #[serde(default)]
    pub stdout_truncated: bool,
    #[serde(default)]
    pub stderr_truncated: bool,
    /// Whether the persisted raw diagnostic prefixes reached their independent
    /// disk budgets. The child pipes are still drained fully after these caps.
    #[serde(default)]
    pub stdout_log_truncated: bool,
    #[serde(default)]
    pub stderr_log_truncated: bool,
    /// Distinct secret kinds redacted out of the in-record output, if any.
    #[serde(default)]
    pub secrets_redacted: Vec<String>,
    pub timed_out: bool,
    /// Files the run changed, as a porcelain delta (post-run minus pre-run git
    /// status). Empty when the working dir is not a git repo or nothing changed.
    #[serde(default)]
    pub changed_files: Vec<GitFileChange>,
    /// Size-bounded unified diff of the tracked changes the run produced.
    #[serde(default)]
    pub diff: String,
    #[serde(default)]
    pub diff_truncated: bool,
    /// Receipt file holding the captured diff, when one was written.
    #[serde(default)]
    pub diff_path: Option<String>,
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
                "Prompt transport is stdin; execution uses argv, never sh -c.".to_string(),
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
                "Prompt transport is stdin; execution uses argv, never sh -c.".to_string(),
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

/// Passive availability: a PATH lookup only. No process is spawned, so this is
/// cheap and safe to call for listing. `version`/`authenticated` are left
/// `None`; use [`coding_agent_availability_probed`] for active bounded probes.
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
        version: None,
        authenticated: None,
        auth_status: ExecutorAuthStatus::Unknown,
        auth_source: None,
        auth_detail: None,
        notes,
    })
}

/// Active availability: passive PATH lookup plus bounded CLI status probes.
/// Probes are argv-only with a short timeout and never use `sh -c`.
pub fn coding_agent_availability_probed(value: &str) -> RepoDeskResult<ExecutorAvailability> {
    let mut availability = coding_agent_availability(value)?;
    if !availability.available {
        return Ok(availability);
    }

    match probe_version(&availability.binary) {
        Some(version) => {
            availability
                .notes
                .push(format!("Version probe succeeded: {version}"));
            availability.version = Some(version);
        }
        None => {
            availability.status = "present_unverified".to_string();
            availability.notes.push(
                "Executable is on PATH but `--version` did not return a recognizable version."
                    .to_string(),
            );
        }
    }

    let auth = detect_authentication(&availability.executor_id, &availability.binary, home_dir());
    availability.authenticated = auth.authenticated;
    availability.auth_status = auth.status;
    availability.auth_source = auth.source;
    availability.auth_detail = auth.detail;
    availability.notes.extend(auth.notes);

    Ok(availability)
}

fn first_meaningful_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

pub fn build_coding_agent_command(
    value: &str,
    writes_allowed: bool,
) -> RepoDeskResult<CodingAgentCommandSpec> {
    let spec = coding_agent_spec(value)?;
    let args = match spec.id.as_str() {
        "codex_cli" => vec![
            "exec".to_string(),
            "--sandbox".to_string(),
            if writes_allowed {
                "workspace-write".to_string()
            } else {
                "read-only".to_string()
            },
            "--color".to_string(),
            "never".to_string(),
            "-".to_string(),
        ],
        "claude_code_cli" => vec![
            "--print".to_string(),
            "--input-format".to_string(),
            "text".to_string(),
            "--output-format".to_string(),
            "text".to_string(),
            "--permission-mode".to_string(),
            if writes_allowed {
                "acceptEdits".to_string()
            } else {
                "plan".to_string()
            },
        ],
        _ => Vec::new(),
    };
    let command = CodingAgentCommandSpec {
        executor_id: spec.id,
        label: spec.label,
        program: spec.binary,
        args,
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

pub fn run_coding_agent_command(
    command: &CodingAgentCommandSpec,
    prompt: &str,
    cwd: &Path,
    output_dir: &Path,
    timeout_secs: u64,
) -> RepoDeskResult<CodingAgentExecution> {
    runtime::run(command, prompt, cwd, output_dir, timeout_secs)
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
    notes.push("automatic CLI execution requires explicit orchestrator approval".to_string());
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

/// Clear the child's environment and forward only the [`FORWARDED_ENV_VARS`]
/// allowlist (plus `LC_*` locale vars). `TERM` is pinned to `dumb` so agents
/// don't emit ANSI control sequences into captured logs.
fn apply_sanitized_env(builder: &mut Command) {
    builder.env_clear();
    for name in FORWARDED_ENV_VARS {
        if let Some(value) = env::var_os(name) {
            builder.env(name, value);
        }
    }
    for (key, value) in env::vars_os() {
        if key.to_string_lossy().starts_with("LC_") {
            builder.env(key, value);
        }
    }
    builder.env("TERM", "dumb");
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        env::var_os("PATHEXT")
            .map(|value| {
                env::split_paths(&value)
                    .map(|path| path.display().to_string())
                    .collect()
            })
            .unwrap_or_else(|| vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()])
    } else {
        vec![String::new()]
    };

    for dir in env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests;
