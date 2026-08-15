use std::fs::{self, File};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
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

    // Snapshot before launch so post-run provenance can attribute exactly the
    // working-tree delta produced by this executor invocation.
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
    configure_process_group(&mut builder);

    let mut child = builder
        .spawn()
        .map_err(|error| RepoDeskError::ProviderUnavailable {
            provider: command.executor_id.clone(),
            detail: format!("failed to start {}: {error}", command.program),
        })?;
    let runtime_started = Instant::now();
    let mut execution_issues = Vec::new();
    let mut force_failed = false;

    // Both pipes are drained concurrently for the entire child lifetime. We keep
    // only a bounded prefix in RAM and persist only a larger bounded prefix to
    // disk; everything beyond both caps is read and discarded to avoid deadlock.
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

    // Prompt delivery is part of executor runtime. A child that stops reading
    // stdin must not be able to block this thread before the timeout starts.
    let stdin_writer = if command.stdin_required {
        child
            .stdin
            .take()
            .map(|stdin| spawn_stdin_delivery(stdin, prompt.as_bytes().to_vec()))
    } else {
        None
    };

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let remaining = timeout.saturating_sub(runtime_started.elapsed());
    let wait_result = child.wait_timeout(remaining);
    let (mut status_label, exit_code, timed_out) = match wait_result {
        Ok(Some(status)) => (
            if status.success() { "ok" } else { "failed" },
            status.code(),
            false,
        ),
        Ok(None) => match terminate_timed_out_child(&mut child) {
            Ok(status) => ("timed_out", status.code(), true),
            Err(error) => {
                push_execution_issue(
                    &mut execution_issues,
                    format!("timeout termination failed: {error}"),
                );
                ("timed_out", None, true)
            }
        },
        Err(error) => {
            push_execution_issue(
                &mut execution_issues,
                format!("executor wait failed: {error}"),
            );
            force_failed = true;
            let exit_code = match terminate_timed_out_child(&mut child) {
                Ok(status) => status.code(),
                Err(terminate_error) => {
                    push_execution_issue(
                        &mut execution_issues,
                        format!("executor termination after wait failure failed: {terminate_error}"),
                    );
                    None
                }
            };
            ("failed", exit_code, false)
        }
    };

    if let Err(error) = join_stdin(stdin_writer)
        && !timed_out
    {
        push_execution_issue(
            &mut execution_issues,
            format!("stdin delivery failed: {error}"),
        );
        force_failed = true;
    }

    let stdout_capture = join_capture_receipt(stdout_reader, "stdout", &mut execution_issues);
    let stderr_capture = join_capture_receipt(stderr_reader, "stderr", &mut execution_issues);
    if stdout_capture.incomplete || stderr_capture.incomplete {
        force_failed = true;
    }
    let stdout_log_truncated = stdout_capture.capture.persisted_truncated;
    let stderr_log_truncated = stderr_capture.capture.persisted_truncated;
    let mut output_capture_issues = Vec::new();
    if let Some(error) = stdout_capture.capture.persist_error.as_deref() {
        output_capture_issues.push(format!("stdout raw log persistence failed: {error}"));
    }
    if let Some(error) = stderr_capture.capture.persist_error.as_deref() {
        output_capture_issues.push(format!("stderr raw log persistence failed: {error}"));
    }
    let (raw_stdout, stdout_truncated) = bounded_text(
        stdout_capture.capture.bytes,
        stdout_capture.capture.retained_truncated,
        limits.stdout_record_bytes,
    );
    let (raw_stderr, stderr_truncated) = bounded_text(
        stderr_capture.capture.bytes,
        stderr_capture.capture.retained_truncated,
        limits.stderr_record_bytes,
    );

    // Once the executor has launched, provenance degradation must not erase the
    // execution receipt. Missing ChangeSet evidence stays explicit and empty.
    let changeset = match capture_changeset(cwd, output_dir, &safe_id, stamp, pre_status.as_ref()) {
        Ok(changeset) => changeset,
        Err(error) => {
            push_execution_issue(
                &mut execution_issues,
                format!("changeset capture failed: {error}"),
            );
            force_failed = true;
            Changeset::empty()
        }
    };

    let (stdout, mut secrets_redacted) = crate::security::redact_secrets(&raw_stdout);
    let (stderr, stderr_secrets) = crate::security::redact_secrets(&raw_stderr);
    secrets_redacted.extend(stderr_secrets);
    secrets_redacted.sort();
    secrets_redacted.dedup();

    if force_failed && status_label == "ok" {
        status_label = "failed";
    }

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

fn spawn_stdin_delivery(mut stdin: ChildStdin, prompt: Vec<u8>) -> JoinHandle<io::Result<()>> {
    thread::spawn(move || stdin.write_all(&prompt))
}

fn join_stdin(handle: Option<JoinHandle<io::Result<()>>>) -> RepoDeskResult<()> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .map_err(|_| RepoDeskError::Api("executor stdin delivery thread panicked".to_string()))?
        .map_err(Into::into)
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
) -> RepoDeskResult<BoundedTee> {
    handle
        .join()
        .map_err(|_| RepoDeskError::Api(format!("executor {stream} capture thread panicked")))?
        .map_err(Into::into)
}

struct ReceiptCapture {
    capture: BoundedTee,
    incomplete: bool,
}

fn join_capture_receipt(
    handle: JoinHandle<io::Result<BoundedTee>>,
    stream: &str,
    execution_issues: &mut Vec<String>,
) -> ReceiptCapture {
    match join_capture(handle, stream) {
        Ok(capture) => ReceiptCapture {
            capture,
            incomplete: false,
        },
        Err(error) => {
            push_execution_issue(
                execution_issues,
                format!("{stream} capture failed: {error}"),
            );
            ReceiptCapture {
                capture: BoundedTee {
                    bytes: Vec::new(),
                    retained_truncated: true,
                    persisted_truncated: true,
                    persist_error: None,
                },
                incomplete: true,
            }
        }
    }
}

fn push_execution_issue(issues: &mut Vec<String>, issue: String) {
    issues.push(issue);
    issues.sort();
    issues.dedup();
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_timed_out_child(child: &mut Child) -> RepoDeskResult<ExitStatus> {
    if let Some(kill_binary) = ["/bin/kill", "/usr/bin/kill"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new(kill_binary)
            .args(["-KILL", "--", &process_group])
            .status();
    }
    let _ = child.kill();
    child.wait().map_err(Into::into)
}

#[cfg(not(unix))]
fn terminate_timed_out_child(child: &mut Child) -> RepoDeskResult<ExitStatus> {
    let _ = child.kill();
    child.wait().map_err(Into::into)
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
