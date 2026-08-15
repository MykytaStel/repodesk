use std::fs::{self, File};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wait_timeout::ChildExt;

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::process_io::{BoundedTee, drain_bounded_to_writer};

use super::changeset::{Changeset, capture_changeset, git_porcelain};
use super::{
    CodingAgentCommandSpec, CodingAgentExecution, apply_sanitized_env, validate_command_spec,
};

const OUTPUT_TRUNCATION_MARKER: &str = "\n[output truncated]";

#[derive(Clone, Copy)]
pub(super) struct OutputLimits {
    pub(super) stdout_record_bytes: usize,
    pub(super) stderr_record_bytes: usize,
    pub(super) stdout_log_bytes: usize,
    pub(super) stderr_log_bytes: usize,
}

const DEFAULT_OUTPUT_LIMITS: OutputLimits = OutputLimits {
    stdout_record_bytes: 2 * 1024 * 1024,
    stderr_record_bytes: 1024 * 1024,
    stdout_log_bytes: 16 * 1024 * 1024,
    stderr_log_bytes: 8 * 1024 * 1024,
};

pub(super) fn run(
    command: &CodingAgentCommandSpec,
    prompt: &str,
    cwd: &Path,
    output_dir: &Path,
    timeout_secs: u64,
) -> RepoDeskResult<CodingAgentExecution> {
    run_with_limits(
        command,
        prompt,
        cwd,
        output_dir,
        timeout_secs,
        DEFAULT_OUTPUT_LIMITS,
    )
}

pub(super) fn run_with_limits(
    command: &CodingAgentCommandSpec,
    prompt: &str,
    cwd: &Path,
    output_dir: &Path,
    timeout_secs: u64,
    limits: OutputLimits,
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
    restrict_permissions(&stdout_path);
    restrict_permissions(&stderr_path);

    // Everything through this point is pre-launch and may fail normally. Once
    // spawn succeeds, infrastructure failures become structured receipt issues
    // instead of erasing the fact that the executor ran.
    let pre_status = git_porcelain(cwd)?;

    let mut builder = Command::new(&command.program);
    builder
        .args(&command.args)
        .current_dir(cwd)
        .stdin(if command.stdin_required {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_sanitized_env(&mut builder);

    let mut child = builder
        .spawn()
        .map_err(|error| RepoDeskError::ProviderUnavailable {
            provider: command.executor_id.clone(),
            detail: format!("failed to start {}: {error}", command.program),
        })?;

    let stdout = child
        .stdout
        .take()
        .expect("stdout is piped immediately before spawning the child");
    let stderr = child
        .stderr
        .take()
        .expect("stderr is piped immediately before spawning the child");
    let stdout_reader = spawn_capture(
        stdout,
        stdout_file,
        limits.stdout_record_bytes.saturating_add(1),
        limits.stdout_log_bytes,
    );
    let stderr_reader = spawn_capture(
        stderr,
        stderr_file,
        limits.stderr_record_bytes.saturating_add(1),
        limits.stderr_log_bytes,
    );

    let mut execution_issues = Vec::new();
    let mut force_failed = false;
    if command.stdin_required
        && let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(prompt.as_bytes())
    {
        execution_issues.push(format!("executor stdin write failed after launch: {error}"));
        force_failed = true;
        if let Err(error) = child.kill() {
            execution_issues.push(format!("executor kill after stdin failure failed: {error}"));
        }
    }

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let (mut status_label, exit_code, timed_out) =
        wait_for_child(&mut child, timeout, &mut execution_issues, &mut force_failed);

    let (stdout_capture, stdout_issue) = join_capture(stdout_reader, "stdout");
    if let Some(issue) = stdout_issue {
        execution_issues.push(issue);
        force_failed = true;
    }
    let (stderr_capture, stderr_issue) = join_capture(stderr_reader, "stderr");
    if let Some(issue) = stderr_issue {
        execution_issues.push(issue);
        force_failed = true;
    }

    let stdout_log_truncated = stdout_capture.persisted_truncated;
    let stderr_log_truncated = stderr_capture.persisted_truncated;
    let mut output_capture_issues = Vec::new();
    if let Some(error) = stdout_capture.persist_error.as_deref() {
        output_capture_issues.push(format!("stdout raw log persistence failed: {error}"));
    }
    if let Some(error) = stderr_capture.persist_error.as_deref() {
        output_capture_issues.push(format!("stderr raw log persistence failed: {error}"));
    }
    let (raw_stdout, stdout_truncated) = bounded_text(
        stdout_capture.bytes,
        stdout_capture.retained_truncated,
        limits.stdout_record_bytes,
    );
    let (raw_stderr, stderr_truncated) = bounded_text(
        stderr_capture.bytes,
        stderr_capture.retained_truncated,
        limits.stderr_record_bytes,
    );

    let changeset = match capture_changeset(cwd, output_dir, &safe_id, stamp, pre_status.as_ref()) {
        Ok(changeset) => changeset,
        Err(error) => {
            execution_issues.push(format!("changeset capture failed after launch: {error}"));
            force_failed = true;
            Changeset::empty()
        }
    };

    if force_failed && status_label == "ok" {
        status_label = "failed";
    }

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
        stdout_log_truncated,
        stderr_log_truncated,
        output_capture_issues,
        execution_issues,
        secrets_redacted,
        timed_out,
        changed_files: changeset.changed_files,
        diff: changeset.diff,
        diff_truncated: changeset.diff_truncated,
        diff_path: changeset.diff_path,
    })
}

fn wait_for_child(
    child: &mut Child,
    timeout: Duration,
    issues: &mut Vec<String>,
    force_failed: &mut bool,
) -> (&'static str, Option<i32>, bool) {
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => status_tuple(status),
        Ok(None) => {
            if let Err(error) = child.kill() {
                issues.push(format!("executor kill after timeout failed: {error}"));
                *force_failed = true;
            }
            match child.wait() {
                Ok(status) => ("timed_out", status.code(), true),
                Err(error) => {
                    issues.push(format!("executor reap after timeout failed: {error}"));
                    *force_failed = true;
                    ("timed_out", None, true)
                }
            }
        }
        Err(error) => {
            issues.push(format!("executor wait failed after launch: {error}"));
            *force_failed = true;
            if let Err(kill_error) = child.kill() {
                issues.push(format!("executor kill after wait failure failed: {kill_error}"));
            }
            match child.wait() {
                Ok(status) => ("failed", status.code(), false),
                Err(wait_error) => {
                    issues.push(format!("executor reap after wait failure failed: {wait_error}"));
                    ("failed", None, false)
                }
            }
        }
    }
}

fn status_tuple(status: ExitStatus) -> (&'static str, Option<i32>, bool) {
    (
        if status.success() { "ok" } else { "failed" },
        status.code(),
        false,
    )
}

fn spawn_capture<R>(
    reader: R,
    file: File,
    retain_max: usize,
    persist_max: usize,
) -> JoinHandle<io::Result<BoundedTee>>
where
    R: io::Read + Send + 'static,
{
    thread::spawn(move || drain_bounded_to_writer(reader, file, retain_max, persist_max))
}

fn join_capture(
    handle: JoinHandle<io::Result<BoundedTee>>,
    stream: &str,
) -> (BoundedTee, Option<String>) {
    match handle.join() {
        Ok(Ok(capture)) => (capture, None),
        Ok(Err(error)) => (
            incomplete_capture(),
            Some(format!("executor {stream} capture failed after launch: {error}")),
        ),
        Err(_) => (
            incomplete_capture(),
            Some(format!(
                "executor {stream} capture thread panicked after launch"
            )),
        ),
    }
}

fn incomplete_capture() -> BoundedTee {
    BoundedTee {
        bytes: Vec::new(),
        retained_truncated: true,
        persisted_truncated: true,
        persist_error: None,
    }
}

fn bounded_text(bytes: Vec<u8>, source_truncated: bool, max: usize) -> (String, bool) {
    let decoded = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    };
    let truncated = source_truncated || decoded.len() > max;
    if !truncated {
        return (decoded, false);
    }

    if max < OUTPUT_TRUNCATION_MARKER.len() {
        return (truncate_char_boundary(&decoded, max).to_string(), true);
    }

    let content_budget = max - OUTPUT_TRUNCATION_MARKER.len();
    let content = truncate_char_boundary(&decoded, content_budget);
    let mut output = String::with_capacity(max);
    output.push_str(content);
    output.push_str(OUTPUT_TRUNCATION_MARKER);
    debug_assert!(output.len() <= max);
    (output, true)
}

fn truncate_char_boundary(text: &str, max: usize) -> &str {
    let mut end = max.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_text_keeps_truncation_marker_inside_hard_limit() {
        let (text, truncated) = bounded_text(vec![b'x'; 100], true, 32);
        assert!(truncated);
        assert!(text.ends_with("[output truncated]"));
        assert!(text.len() <= 32);
    }

    #[test]
    fn bounded_text_respects_utf8_boundaries() {
        let input = "💾".repeat(20).into_bytes();
        let (text, truncated) = bounded_text(input, true, 31);
        assert!(truncated);
        assert!(text.is_char_boundary(text.len()));
        assert!(text.len() <= 31);
    }
}
