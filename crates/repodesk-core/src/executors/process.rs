use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::errors::RepoDeskResult;

use super::{apply_sanitized_env, first_meaningful_line, validate_token};

const MAX_PROBE_OUTPUT_BYTES: usize = 8_000;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Read at most `max + 1` bytes so truncation can be detected without ever
/// materializing the rest of a potentially huge agent log in memory.
pub(super) fn read_bounded(path: &Path, max: usize) -> RepoDeskResult<(String, bool)> {
    let file = File::open(path)?;
    let (bytes, truncated) = read_prefix(file, max)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[output truncated]");
    }
    Ok((text, truncated))
}

fn read_prefix(reader: impl Read, max: usize) -> io::Result<(Vec<u8>, bool)> {
    let probe_len = max.saturating_add(1);
    let limit = u64::try_from(probe_len).unwrap_or(u64::MAX);
    let mut bytes = Vec::with_capacity(probe_len.min(64 * 1024));
    reader.take(limit).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > max;
    if truncated {
        bytes.truncate(max);
    }
    Ok((bytes, truncated))
}

#[derive(Debug)]
pub(super) struct ProbeCommandOutput {
    pub(super) status_success: bool,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

impl ProbeCommandOutput {
    pub(super) fn combined(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.clone()
        } else if self.stdout.trim().is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// Run a short, non-interactive CLI probe while draining stdout/stderr
/// concurrently. Captured memory is bounded, but the readers continue draining
/// excess bytes so a verbose child cannot deadlock on a full OS pipe.
pub(super) fn run_probe_command(binary: &str, args: &[&str]) -> Option<ProbeCommandOutput> {
    run_probe_command_with_timeout(binary, args, PROBE_TIMEOUT)
}

fn run_probe_command_with_timeout(
    binary: &str,
    args: &[&str],
    timeout: Duration,
) -> Option<ProbeCommandOutput> {
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
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, MAX_PROBE_OUTPUT_BYTES));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_PROBE_OUTPUT_BYTES));

    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return None;
        }
    };

    let stdout = stdout_reader.join().ok()?.ok()?;
    let stderr = stderr_reader.join().ok()?.ok()?;
    let (stdout, _) = crate::security::redact_secrets(&stdout);
    let (stderr, _) = crate::security::redact_secrets(&stderr);

    Some(ProbeCommandOutput {
        status_success: status.success(),
        stdout,
        stderr,
    })
}

fn drain_bounded(reader: impl Read, max: usize) -> io::Result<String> {
    let capture = crate::process_io::drain_bounded_bytes(reader, max)?;
    let mut text = String::from_utf8_lossy(&capture.bytes).into_owned();
    if capture.truncated {
        text.push_str("\n[output truncated]");
    }
    Ok(text)
}

/// Run `<binary> --version` through the same bounded, concurrently-drained probe
/// path used for authentication status checks.
pub(super) fn probe_version(binary: &str) -> Option<String> {
    let output = run_probe_command(binary, &["--version"])?;
    if !output.status_success {
        return None;
    }
    first_meaningful_line(&output.stdout).or_else(|| first_meaningful_line(&output.stderr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn prefix_reader_keeps_small_streams_intact() {
        let (bytes, truncated) = read_prefix(Cursor::new(b"hello"), 8).unwrap();
        assert_eq!(bytes, b"hello");
        assert!(!truncated);
    }

    #[test]
    fn prefix_reader_reads_only_one_byte_past_the_budget() {
        let input = vec![b'x'; 1024 * 1024];
        let (bytes, truncated) = read_prefix(Cursor::new(input), 32).unwrap();
        assert_eq!(bytes, vec![b'x'; 32]);
        assert!(truncated);
    }

    #[test]
    fn drain_bounded_discards_excess_without_retaining_it() {
        let input = vec![b'x'; 1024 * 1024];
        let text = drain_bounded(Cursor::new(input), 32).unwrap();
        assert_eq!(text, format!("{}\n[output truncated]", "x".repeat(32)));
    }

    #[test]
    fn bounded_file_reader_marks_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.log");
        std::fs::write(&path, b"abcdefghij").unwrap();

        let (text, truncated) = read_bounded(&path, 4).unwrap();
        assert_eq!(text, "abcd\n[output truncated]");
        assert!(truncated);
    }

    #[cfg(unix)]
    #[test]
    fn verbose_probe_is_drained_without_pipe_deadlock() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("verbose-probe");
        std::fs::write(
            &script,
            "#!/bin/sh\ni=0\nwhile [ $i -lt 20000 ]; do printf 'xxxxxxxxxxxxxxxx\\n'; i=$((i + 1)); done\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let output = run_probe_command_with_timeout(
            &script.display().to_string(),
            &[],
            Duration::from_secs(2),
        )
        .expect("verbose probe should complete while pipes are drained");
        assert!(output.status_success);
        assert!(output.stdout.len() <= MAX_PROBE_OUTPUT_BYTES + 32);
        assert!(output.stdout.contains("[output truncated]"));
    }
}
