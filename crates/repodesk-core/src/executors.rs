//! Safe command specifications for coding-agent executors.
//!
//! This module defines canonical executor ids, passive PATH availability,
//! argv-only command specs, and the low-level process runner used after the
//! orchestrator has explicit human approval. No `sh -c`.

use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::git_workspace::{self, GitFileChange};

mod process;
use process::read_bounded;

/// Maximum diff size kept on a [`CodingAgentExecution`] / written to a receipt.
/// A larger diff is truncated with a trailing marker so the run record never
/// balloons; the full working tree is still on disk for the human to inspect.
const MAX_DIFF_BYTES: usize = 200 * 1024;

/// Caps on the agent output kept on a [`CodingAgentExecution`]. The raw streams
/// are still on disk (restrictive perms); these bound what enters a run record
/// and the Memory Brain so a runaway log can't balloon state or exfil secrets.
const MAX_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 1024 * 1024;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodingAgentExecution {
    pub executor_id: String,
    pub command_preview: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    /// Bounded ([`MAX_STDOUT_BYTES`]) and secret-redacted stdout. The full raw
    /// stream is at `stdout_path` with restrictive permissions.
    pub stdout: String,
    /// Bounded ([`MAX_STDERR_BYTES`]) and secret-redacted stderr.
    pub stderr: String,
    pub stdout_path: String,
    pub stderr_path: String,
    /// Whether the in-record stdout/stderr were truncated to their byte caps.
    #[serde(default)]
    pub stdout_truncated: bool,
    #[serde(default)]
    pub stderr_truncated: bool,
    /// Distinct secret kinds redacted out of the in-record output, if any.
    #[serde(default)]
    pub secrets_redacted: Vec<String>,
    pub timed_out: bool,
    /// Files the run changed, as a porcelain delta (post-run minus pre-run git
    /// status). Empty when the working dir is not a git repo or nothing changed.
    #[serde(default)]
    pub changed_files: Vec<GitFileChange>,
    /// Unified diff (staged + unstaged vs the index/HEAD) of the tracked changes
    /// the run produced, size-limited to [`MAX_DIFF_BYTES`]. Untracked new files
    /// appear in `changed_files` but not here (their content is on disk).
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

/// Passive availability: a PATH lookup only. No process is spawned, so this is
/// cheap and safe to call for listing. `version`/`authenticated` are left
/// `None`; use [`coding_agent_availability_probed`] to additionally run the
/// CLI's own `--version`.
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

/// Active availability: passive PATH lookup *plus* bounded CLI status probes.
/// Running the CLI's own version command confirms the executable is actually
/// runnable (not just present) and surfaces its version for the desktop "Coding
/// agents" panel. Auth probing uses documented non-interactive status commands
/// where available (`codex login status`, `claude auth status --json`), then
/// falls back to local auth-artifact existence if the command is inconclusive.
/// Probes are argv-only with a short timeout and never use `sh -c`.
///
/// `authenticated` remains a compatibility tri-state: `Some(true)`/`Some(false)`
/// only when a status command or artifact fallback supports that conclusion,
/// otherwise `None` (unknown). RepoDesk never parses credential files and never
/// treats API-key env vars as proof a CLI is authenticated.
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
            // Present on PATH but `--version` did not return cleanly. Keep it
            // listed as available (it may still run) but flag the probe.
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

struct AuthProbeResult {
    authenticated: Option<bool>,
    status: ExecutorAuthStatus,
    source: Option<String>,
    detail: Option<String>,
    notes: Vec<String>,
}

impl AuthProbeResult {
    fn authenticated(source: &str, detail: Option<String>, note: String) -> Self {
        Self {
            authenticated: Some(true),
            status: ExecutorAuthStatus::Authenticated,
            source: Some(source.to_string()),
            detail,
            notes: vec![note],
        }
    }

    fn unauthenticated(source: &str, detail: Option<String>, note: String) -> Self {
        Self {
            authenticated: Some(false),
            status: ExecutorAuthStatus::Unauthenticated,
            source: Some(source.to_string()),
            detail,
            notes: vec![note],
        }
    }

    fn unknown(note: String) -> Self {
        Self {
            authenticated: None,
            status: ExecutorAuthStatus::Unknown,
            source: None,
            detail: None,
            notes: vec![note],
        }
    }

    fn is_unknown(&self) -> bool {
        self.status == ExecutorAuthStatus::Unknown
    }
}

/// Known local authentication artifacts for a coding-agent CLI, relative to the
/// user's home directory. Existence is checked, never contents.
fn auth_artifacts(executor_id: &str) -> &'static [&'static str] {
    match executor_id {
        "codex_cli" => &[".codex/auth.json"],
        "claude_code_cli" => &[
            ".claude/.credentials.json",
            ".config/claude/.credentials.json",
        ],
        _ => &[],
    }
}

/// Prefer documented, side-effect-free CLI auth status commands. Fall back to a
/// local artifact existence check only when the status command is unavailable or
/// inconclusive. Credential files are never opened or parsed.
fn detect_authentication(
    executor_id: &str,
    binary: &str,
    home: Option<PathBuf>,
) -> AuthProbeResult {
    let cli_probe = match executor_id {
        "codex_cli" => probe_codex_auth(binary),
        "claude_code_cli" => probe_claude_auth(binary),
        _ => AuthProbeResult::unknown(format!(
            "No auth probe is registered for executor {executor_id}."
        )),
    };
    if !cli_probe.is_unknown() {
        return cli_probe;
    }

    let mut artifact_probe = detect_auth_artifact(executor_id, home);
    if artifact_probe.is_unknown() {
        artifact_probe.notes.extend(cli_probe.notes);
    }
    artifact_probe
}

fn detect_auth_artifact(executor_id: &str, home: Option<PathBuf>) -> AuthProbeResult {
    let Some(home) = home else {
        return AuthProbeResult::unknown(
            "No documented CLI auth status or known local auth artifact found.".to_string(),
        );
    };
    let found = auth_artifacts(executor_id)
        .iter()
        .any(|relative| home.join(relative).exists());
    if found {
        AuthProbeResult::authenticated(
            "local auth artifact",
            Some("known local auth artifact exists; contents not read".to_string()),
            "Local auth artifact found (existence only; contents not read).".to_string(),
        )
    } else {
        AuthProbeResult::unknown(
            "No documented CLI auth status or known local auth artifact found.".to_string(),
        )
    }
}

fn probe_codex_auth(binary: &str) -> AuthProbeResult {
    const SOURCE: &str = "codex login status";
    match run_probe_command(binary, &["login", "status"]) {
        Some(output) => parse_codex_auth_output(SOURCE, output.status_success, &output.combined()),
        None => {
            AuthProbeResult::unknown("Codex auth status probe could not be started.".to_string())
        }
    }
}

fn parse_codex_auth_output(source: &str, status_success: bool, output: &str) -> AuthProbeResult {
    let detail = safe_status_detail(output);
    let lower = output.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not authenticated")
        || lower.contains("logged out")
    {
        return AuthProbeResult::unauthenticated(
            source,
            detail,
            "Codex auth status reports not logged in.".to_string(),
        );
    }
    if status_success && lower.contains("logged in") {
        return AuthProbeResult::authenticated(
            source,
            detail,
            "Codex auth status reports logged in.".to_string(),
        );
    }
    AuthProbeResult::unknown("Codex auth status output was inconclusive.".to_string())
}

fn probe_claude_auth(binary: &str) -> AuthProbeResult {
    const SOURCE: &str = "claude auth status --json";
    match run_probe_command(binary, &["auth", "status", "--json"]) {
        Some(output) => {
            let text = if output.stdout.trim_start().starts_with('{') {
                output.stdout.clone()
            } else {
                output.combined()
            };
            parse_claude_auth_output(SOURCE, output.status_success, &text)
        }
        None => {
            AuthProbeResult::unknown("Claude auth status probe could not be started.".to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClaudeAuthStatusOutput {
    #[serde(rename = "loggedIn")]
    logged_in: Option<bool>,
    #[serde(rename = "authMethod")]
    auth_method: Option<String>,
    #[serde(rename = "apiProvider")]
    api_provider: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

fn parse_claude_auth_output(source: &str, status_success: bool, output: &str) -> AuthProbeResult {
    if let Ok(parsed) = serde_json::from_str::<ClaudeAuthStatusOutput>(output) {
        let detail = claude_auth_detail(&parsed);
        return match parsed.logged_in {
            Some(true) => AuthProbeResult::authenticated(
                source,
                detail,
                "Claude auth status reports logged in.".to_string(),
            ),
            Some(false) => AuthProbeResult::unauthenticated(
                source,
                detail,
                "Claude auth status reports not logged in.".to_string(),
            ),
            None => AuthProbeResult::unknown(
                "Claude auth status JSON did not include loggedIn.".to_string(),
            ),
        };
    }

    let lower = output.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not authenticated")
        || lower.contains("logged out")
    {
        return AuthProbeResult::unauthenticated(
            source,
            safe_status_detail(output),
            "Claude auth status reports not logged in.".to_string(),
        );
    }
    if status_success && (lower.contains("logged in") || lower.contains("authenticated")) {
        return AuthProbeResult::authenticated(
            source,
            safe_status_detail(output),
            "Claude auth status reports logged in.".to_string(),
        );
    }
    AuthProbeResult::unknown("Claude auth status output was inconclusive.".to_string())
}

fn claude_auth_detail(parsed: &ClaudeAuthStatusOutput) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(method) = non_secret_label(parsed.auth_method.as_deref()) {
        parts.push(format!("method {method}"));
    }
    if let Some(provider) = non_secret_label(parsed.api_provider.as_deref()) {
        parts.push(format!("provider {provider}"));
    }
    if let Some(subscription) = non_secret_label(parsed.subscription_type.as_deref()) {
        parts.push(format!("subscription {subscription}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

fn non_secret_label(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let safe = !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'));
    safe.then(|| value.to_string())
}

struct ProbeCommandOutput {
    status_success: bool,
    stdout: String,
    stderr: String,
}

impl ProbeCommandOutput {
    fn combined(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.clone()
        } else if self.stdout.trim().is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

fn run_probe_command(binary: &str, args: &[&str]) -> Option<ProbeCommandOutput> {
    if validate_token("program", binary).is_err()
        || args
            .iter()
            .any(|arg| validate_token("argument", arg).is_err())
    {
        return None;
    }
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_sanitized_env(&mut command);
    let mut child = command.spawn().ok()?;
    let status_success = match child.wait_timeout(Duration::from_secs(5)).ok()? {
        Some(status) => status.success(),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let output = child.wait_with_output().ok()?;
    let (stdout, _) = truncate_to_bytes(&String::from_utf8_lossy(&output.stdout), 8_000);
    let (stderr, _) = truncate_to_bytes(&String::from_utf8_lossy(&output.stderr), 8_000);
    let (stdout, _) = crate::security::redact_secrets(&stdout);
    let (stderr, _) = crate::security::redact_secrets(&stderr);
    Some(ProbeCommandOutput {
        status_success,
        stdout,
        stderr,
    })
}

fn safe_status_detail(output: &str) -> Option<String> {
    first_meaningful_line(output).and_then(|line| {
        let (redacted, _) = crate::security::redact_secrets(&line);
        if looks_like_personal_identifier(&redacted) {
            None
        } else {
            Some(redacted.chars().take(160).collect())
        }
    })
}

fn looks_like_personal_identifier(value: &str) -> bool {
    value.contains('@')
        || value.to_ascii_lowercase().contains("email")
        || value.to_ascii_lowercase().contains("orgid")
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Run `<binary> --version` argv-only with a short timeout and return the first
/// non-empty trimmed line of stdout (falling back to stderr). Any failure —
/// missing binary, non-zero exit, timeout, empty output — yields `None`.
fn probe_version(binary: &str) -> Option<String> {
    if validate_token("program", binary).is_err() {
        return None;
    }
    let mut child = Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    match child.wait_timeout(Duration::from_secs(5)).ok()? {
        Some(status) if status.success() => {}
        Some(_) => return None,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    }

    let output = child.wait_with_output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    first_meaningful_line(&stdout).or_else(|| first_meaningful_line(&stderr))
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
    validate_command_spec(command)?;
    fs::create_dir_all(output_dir)?;

    let started = Instant::now();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let safe_id = command
        .executor_id
        .replace(|ch: char| !ch.is_ascii_alphanumeric(), "_");
    let stdout_path = output_dir.join(format!("{safe_id}-{stamp}.stdout.log"));
    let stderr_path = output_dir.join(format!("{safe_id}-{stamp}.stderr.log"));
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;
    // The raw streams may contain whatever the agent printed; keep them
    // owner-only so a debug bundle / backup doesn't expose them broadly.
    restrict_permissions(&stdout_path);
    restrict_permissions(&stderr_path);

    // Snapshot the working tree before the run so we can attribute exactly what
    // the agent changed (post-run minus pre-run). `None` when `cwd` is not a repo.
    let pre_status = git_porcelain(cwd);

    let mut builder = Command::new(&command.program);
    builder
        .args(&command.args)
        .current_dir(cwd)
        .stdin(if command.stdin_required {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    // Start from an empty environment and forward only non-secret vars, so the
    // child never inherits RepoDesk's provider keys or other secrets.
    apply_sanitized_env(&mut builder);
    let mut child = builder
        .spawn()
        .map_err(|error| RepoDeskError::ProviderUnavailable {
            provider: command.executor_id.clone(),
            detail: format!("failed to start {}: {error}", command.program),
        })?;

    if command.stdin_required
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin.write_all(prompt.as_bytes())?;
    }

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let (status_label, exit_code, timed_out) = match child.wait_timeout(timeout)? {
        Some(status) => (
            if status.success() { "ok" } else { "failed" },
            status.code(),
            false,
        ),
        None => {
            let _ = child.kill();
            let status = child.wait()?;
            ("timed_out", status.code(), true)
        }
    };

    // Capture the changeset the run produced (cheap; empty when not a git repo
    // or nothing changed). Always done — a read-only run that changed nothing is
    // itself a useful "no writes" receipt.
    let changeset = capture_changeset(cwd, output_dir, &safe_id, stamp, pre_status.as_deref());

    // Bound then secret-redact the streams before they enter the run record /
    // Memory Brain. The raw, unredacted streams remain at *_path on disk.
    let (raw_stdout, stdout_truncated) = read_bounded(&stdout_path, MAX_STDOUT_BYTES)?;
    let (raw_stderr, stderr_truncated) = read_bounded(&stderr_path, MAX_STDERR_BYTES)?;
    let (stdout, mut secrets_redacted) = crate::security::redact_secrets(&raw_stdout);
    let (stderr, stderr_secrets) = crate::security::redact_secrets(&raw_stderr);
    secrets_redacted.extend(stderr_secrets);
    secrets_redacted.sort();
    secrets_redacted.dedup();

    Ok(CodingAgentExecution {
        executor_id: command.executor_id.clone(),
        command_preview: command.command_preview.clone(),
        status: status_label.to_string(),
        exit_code,
        duration_ms: started.elapsed().as_millis(),
        stdout,
        stderr,
        stdout_path: stdout_path.display().to_string(),
        stderr_path: stderr_path.display().to_string(),
        stdout_truncated,
        stderr_truncated,
        secrets_redacted,
        timed_out,
        changed_files: changeset.changed_files,
        diff: changeset.diff,
        diff_truncated: changeset.diff_truncated,
        diff_path: changeset.diff_path,
    })
}

/// The working-tree changeset a run produced.
struct Changeset {
    changed_files: Vec<GitFileChange>,
    diff: String,
    diff_truncated: bool,
    diff_path: Option<String>,
}

/// Compare the post-run git status to the pre-run snapshot, capture the unified
/// diff of the tracked changes, and write it to a `.diff` receipt. Returns an
/// empty changeset when `cwd` is not a git repo or nothing changed.
fn capture_changeset(
    cwd: &Path,
    output_dir: &Path,
    safe_id: &str,
    stamp: u128,
    pre_status: Option<&str>,
) -> Changeset {
    let empty = Changeset {
        changed_files: Vec::new(),
        diff: String::new(),
        diff_truncated: false,
        diff_path: None,
    };
    let Some(pre) = pre_status else {
        return empty;
    };
    let post = git_workspace::run_git_captured(cwd, &["status", "--porcelain=v1"]);
    let changed_files = changed_since(pre, &post);
    if changed_files.is_empty() {
        return empty;
    }

    // Staged + unstaged tracked diffs; neither needs an existing HEAD, so this
    // works in a brand-new repo too. Untracked files are listed in
    // `changed_files` but their content is not inlined here.
    let mut raw_diff = git_workspace::run_git_captured(cwd, &["diff", "--no-color"]);
    let cached = git_workspace::run_git_captured(cwd, &["diff", "--cached", "--no-color"]);
    if !cached.trim().is_empty() {
        if !raw_diff.trim().is_empty() {
            raw_diff.push('\n');
        }
        raw_diff.push_str(&cached);
    }

    let (diff, diff_truncated) = truncate_to_bytes(&raw_diff, MAX_DIFF_BYTES);
    let diff_path = {
        let path = output_dir.join(format!("{safe_id}-{stamp}.diff"));
        match fs::write(&path, diff.as_bytes()) {
            Ok(()) => Some(path.display().to_string()),
            Err(_) => None,
        }
    };

    Changeset {
        changed_files,
        diff,
        diff_truncated,
        diff_path,
    }
}

/// Porcelain status of `cwd`, or `None` when it is not inside a git work tree.
fn git_porcelain(cwd: &Path) -> Option<String> {
    let inside = git_workspace::run_git_captured(cwd, &["rev-parse", "--is-inside-work-tree"]);
    if inside.trim() != "true" {
        return None;
    }
    Some(git_workspace::run_git_captured(
        cwd,
        &["status", "--porcelain=v1"],
    ))
}

/// Porcelain lines present after the run but not before it — the files the run
/// added or whose status it changed.
fn changed_since(pre: &str, post: &str) -> Vec<GitFileChange> {
    let pre_lines: HashSet<&str> = pre.lines().collect();
    post.lines()
        .filter(|line| !pre_lines.contains(*line))
        .filter_map(git_workspace::parse_porcelain_line)
        .collect()
}

/// Truncate `text` to at most `max` bytes on a char boundary, appending a marker
/// when truncated. Returns `(text, was_truncated)`.
fn truncate_to_bytes(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_string(), false);
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}\n[diff truncated]", &text[..end]), true)
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
/// don't emit ANSI control sequences into the captured logs.
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

/// Best-effort restrict a file to owner read/write (`0o600`) on Unix. A no-op
/// elsewhere; failures are ignored since the bounded/redacted in-record copy is
/// the security-relevant one.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
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
        assert_eq!(
            command.args,
            vec![
                "exec",
                "--sandbox",
                "workspace-write",
                "--color",
                "never",
                "-"
            ]
        );
        assert!(command.stdin_required);
        assert!(command.writes_allowed);
        assert_eq!(
            command.command_preview,
            "codex exec --sandbox workspace-write --color never - [stdin: bounded prompt]"
        );
    }

    #[test]
    fn claude_readonly_command_uses_plan_mode() {
        let command = build_coding_agent_command("claude", false).unwrap();
        assert_eq!(command.program, "claude");
        assert_eq!(
            command.args,
            vec![
                "--print",
                "--input-format",
                "text",
                "--output-format",
                "text",
                "--permission-mode",
                "plan"
            ]
        );
        assert!(!command.writes_allowed);
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

    #[cfg(unix)]
    fn fake_binary(body: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("fake-agent");
        std::fs::write(&script, body).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (dir, script.display().to_string())
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
                .any(|note| note.contains("explicit orchestrator approval"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_command_captures_stdout_and_stderr() {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("agent");
        std::fs::write(&script, "#!/bin/sh\ncat\necho stderr-line >&2\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let command = CodingAgentCommandSpec {
            executor_id: "codex_cli".to_string(),
            label: "Test Agent".to_string(),
            program: script.display().to_string(),
            args: Vec::new(),
            stdin_required: true,
            cwd_required: true,
            writes_allowed: true,
            command_preview: "agent [stdin: bounded prompt]".to_string(),
        };

        let result =
            run_coding_agent_command(&command, "hello prompt", dir.path(), dir.path(), 5).unwrap();
        assert_eq!(result.status, "ok");
        assert_eq!(result.stdout, "hello prompt");
        assert!(result.stderr.contains("stderr-line"));
        assert!(std::path::Path::new(&result.stdout_path).exists());
        assert!(std::path::Path::new(&result.stderr_path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn probe_version_reads_first_stdout_line() {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("fake-agent");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'fake-agent 1.2.3'; fi\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let version = probe_version(&script.display().to_string());
        assert_eq!(version.as_deref(), Some("fake-agent 1.2.3"));
    }

    #[cfg(unix)]
    #[test]
    fn probe_version_returns_none_on_nonzero_exit() {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("broken-agent");
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(probe_version(&script.display().to_string()), None);
    }

    #[test]
    fn detect_authentication_falls_back_to_artifact_existence() {
        let home = tempfile::TempDir::new().unwrap();
        // No artifact yet → unknown.
        assert_eq!(
            detect_authentication(
                "codex_cli",
                "missing-codex-binary",
                Some(home.path().to_path_buf())
            )
            .authenticated,
            None
        );
        // Create the known artifact → Some(true), without reading contents.
        std::fs::create_dir_all(home.path().join(".codex")).unwrap();
        std::fs::write(home.path().join(".codex/auth.json"), "{}").unwrap();
        let probe = detect_authentication(
            "codex_cli",
            "missing-codex-binary",
            Some(home.path().to_path_buf()),
        );
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(probe.source.as_deref(), Some("local auth artifact"));
        // Never proves absence for the other executor.
        assert_eq!(
            detect_authentication(
                "claude_code_cli",
                "missing-claude-binary",
                Some(home.path().to_path_buf())
            )
            .authenticated,
            None
        );
        // No home → unknown.
        assert_eq!(
            detect_authentication("codex_cli", "missing-codex-binary", None).authenticated,
            None
        );
    }

    #[test]
    fn codex_auth_parser_reports_true_and_false() {
        let logged_in =
            parse_codex_auth_output("codex login status", true, "Logged in using ChatGPT");
        assert_eq!(logged_in.authenticated, Some(true));
        assert_eq!(logged_in.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(logged_in.detail.as_deref(), Some("Logged in using ChatGPT"));

        let logged_out = parse_codex_auth_output("codex login status", false, "Not logged in");
        assert_eq!(logged_out.authenticated, Some(false));
        assert_eq!(logged_out.status, ExecutorAuthStatus::Unauthenticated);
    }

    #[test]
    fn claude_auth_parser_redacts_account_identifiers() {
        let raw = r#"{
          "loggedIn": true,
          "authMethod": "claude.ai",
          "apiProvider": "firstParty",
          "email": "person@example.com",
          "orgId": "abc-123",
          "subscriptionType": "pro"
        }"#;
        let probe = parse_claude_auth_output("claude auth status --json", true, raw);
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        let detail = probe.detail.unwrap();
        assert!(detail.contains("method claude.ai"));
        assert!(detail.contains("provider firstParty"));
        assert!(detail.contains("subscription pro"));
        assert!(!detail.contains("person@example.com"));
        assert!(!detail.contains("abc-123"));
    }

    #[cfg(unix)]
    #[test]
    fn detect_authentication_uses_codex_status_command() {
        let (_dir, binary) = fake_binary(
            "#!/bin/sh\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then echo 'Logged in using ChatGPT'; exit 0; fi\nexit 2\n",
        );

        let probe = detect_authentication("codex_cli", &binary, None);
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(probe.source.as_deref(), Some("codex login status"));
    }

    #[cfg(unix)]
    #[test]
    fn detect_authentication_uses_claude_status_command() {
        let (_dir, binary) = fake_binary(
            "#!/bin/sh\nif [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ] && [ \"$3\" = \"--json\" ]; then printf '{\"loggedIn\":false}\\n'; exit 0; fi\nexit 2\n",
        );

        let probe = detect_authentication("claude_code_cli", &binary, None);
        assert_eq!(probe.authenticated, Some(false));
        assert_eq!(probe.status, ExecutorAuthStatus::Unauthenticated);
        assert_eq!(probe.source.as_deref(), Some("claude auth status --json"));
    }

    #[test]
    fn probed_availability_for_missing_binary_skips_probe() {
        // `codex`/`claude` are unlikely to exist on a CI PATH; if they do the
        // probe may add version/auth metadata from local artifacts. Either way
        // the call must not error.
        let availability = coding_agent_availability_probed("codex_cli").unwrap();
        assert_eq!(availability.executor_id, "codex_cli");
        if !availability.available {
            assert_eq!(availability.version, None);
            assert_eq!(availability.authenticated, None);
            assert_eq!(availability.status, "missing");
        } else {
            assert!(matches!(
                availability.authenticated,
                None | Some(true) | Some(false)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_command_times_out_and_kills_child() {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("agent");
        std::fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let command = CodingAgentCommandSpec {
            executor_id: "codex_cli".to_string(),
            label: "Test Agent".to_string(),
            program: script.display().to_string(),
            args: Vec::new(),
            stdin_required: true,
            cwd_required: true,
            writes_allowed: true,
            command_preview: "agent [stdin: bounded prompt]".to_string(),
        };

        let result = run_coding_agent_command(&command, "prompt", dir.path(), dir.path(), 1)
            .expect("timeout result");
        assert_eq!(result.status, "timed_out");
        assert!(result.timed_out);
    }

    #[test]
    fn truncate_to_bytes_marks_when_over_limit() {
        assert_eq!(
            truncate_to_bytes("short", 100),
            ("short".to_string(), false)
        );
        let (text, truncated) = truncate_to_bytes("abcdefgh", 4);
        assert!(truncated);
        assert!(text.starts_with("abcd"));
        assert!(text.contains("[diff truncated]"));
    }

    #[test]
    fn changed_since_reports_only_new_status_lines() {
        let pre = " M seed.txt\n";
        let post = " M seed.txt\n M other.rs\n?? added.txt\n";
        let changed = changed_since(pre, post);
        let paths: Vec<&str> = changed.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["other.rs", "added.txt"]);
    }

    #[cfg(unix)]
    #[test]
    fn run_command_captures_git_changeset() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            assert!(ok, "git {args:?} failed");
        };
        if Command::new("git").arg("--version").output().is_err() {
            return; // git not installed on this runner
        }
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "init"]);

        // Fake agent: consume stdin, modify a tracked file, create a new file.
        let out = tempfile::TempDir::new().unwrap();
        let script = out.path().join("agent");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat > /dev/null\necho changed >> seed.txt\necho new > added.txt\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let command = CodingAgentCommandSpec {
            executor_id: "codex_cli".to_string(),
            label: "Test Agent".to_string(),
            program: script.display().to_string(),
            args: Vec::new(),
            stdin_required: true,
            cwd_required: true,
            writes_allowed: true,
            command_preview: "agent [stdin: bounded prompt]".to_string(),
        };

        let result =
            run_coding_agent_command(&command, "prompt", repo.path(), out.path(), 5).unwrap();
        assert_eq!(result.status, "ok");
        let paths: Vec<&str> = result
            .changed_files
            .iter()
            .map(|c| c.path.as_str())
            .collect();
        assert!(paths.contains(&"seed.txt"), "tracked change: {paths:?}");
        assert!(paths.contains(&"added.txt"), "new file: {paths:?}");
        // The tracked modification appears in the unified diff; the receipt exists.
        assert!(result.diff.contains("seed.txt"));
        assert!(!result.diff_truncated);
        let diff_path = result.diff_path.expect("diff receipt written");
        assert!(std::path::Path::new(&diff_path).exists());
    }
}
