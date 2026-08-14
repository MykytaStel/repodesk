use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

const DEFAULT_GIT_CAPTURE_BYTES: usize = 1024 * 1024;
const DEFAULT_TRUNCATION_MARKER: &str = "\n[git output truncated]";

#[derive(Debug)]
pub(crate) struct BoundedGitCapture {
    pub(crate) success: bool,
    pub(crate) text: String,
    pub(crate) truncated: bool,
}

/// Compatibility helper for callers that only need textual Git output.
///
/// Capture is intentionally bounded so a repository-controlled Git command can
/// never force RepoDesk to materialize arbitrary output in memory. Callers that
/// need a smaller domain-specific budget should use `run_git_captured_bounded`.
pub fn run_git_captured(project_path: &Path, args: &[&str]) -> String {
    let capture = run_git_captured_bounded(project_path, args, DEFAULT_GIT_CAPTURE_BYTES);
    truncate_with_marker(
        &capture.text,
        DEFAULT_GIT_CAPTURE_BYTES,
        capture.truncated,
        DEFAULT_TRUNCATION_MARKER,
    )
}

/// Run Git while retaining only `max + 1` bytes from each output pipe. The one
/// byte overflow probe lets callers preserve their existing truncation marker
/// semantics without ever materializing the complete command output.
///
/// Both pipes are drained concurrently to EOF, so verbose Git output cannot
/// deadlock on a full pipe and cannot force RepoDesk to retain all output in RAM.
pub(crate) fn run_git_captured_bounded(
    project_path: &Path,
    args: &[&str],
    max: usize,
) -> BoundedGitCapture {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(project_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return BoundedGitCapture {
                success: false,
                text: format!("failed to run git {}: {}", args.join(" "), error),
                truncated: false,
            };
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return BoundedGitCapture {
            success: false,
            text: "failed to capture git stdout".to_string(),
            truncated: false,
        };
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return BoundedGitCapture {
            success: false,
            text: "failed to capture git stderr".to_string(),
            truncated: false,
        };
    };

    let stdout_reader = thread::spawn(move || drain_bounded(stdout, max));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, max));
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return BoundedGitCapture {
                success: false,
                text: format!("failed to wait for git {}: {}", args.join(" "), error),
                truncated: false,
            };
        }
    };

    let stdout = stdout_reader.join().ok().and_then(Result::ok);
    let stderr = stderr_reader.join().ok().and_then(Result::ok);

    let Some((stdout, stdout_truncated)) = stdout else {
        return BoundedGitCapture {
            success: false,
            text: "failed to read git stdout".to_string(),
            truncated: false,
        };
    };
    let Some((stderr, stderr_truncated)) = stderr else {
        return BoundedGitCapture {
            success: false,
            text: "failed to read git stderr".to_string(),
            truncated: false,
        };
    };

    let success = status.success();
    let (text, truncated) = if success || stderr.trim().is_empty() {
        (stdout, stdout_truncated)
    } else {
        (stderr, stderr_truncated)
    };

    BoundedGitCapture {
        success,
        text,
        truncated,
    }
}

pub(crate) fn truncate_with_marker(
    text: &str,
    max: usize,
    truncated: bool,
    marker: &str,
) -> String {
    if !truncated && text.len() <= max {
        return text.to_string();
    }

    if max == 0 {
        return String::new();
    }

    if marker.len() > max {
        let mut end = max.min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        return text[..end].to_string();
    }

    let content_budget = max - marker.len();
    let mut end = content_budget.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    let mut output = String::with_capacity(max);
    output.push_str(&text[..end]);
    output.push_str(marker);
    output
}

fn drain_bounded(mut reader: impl Read, max: usize) -> io::Result<(String, bool)> {
    let retain_limit = max.saturating_add(1);
    let mut retained = Vec::with_capacity(retain_limit.min(64 * 1024));
    let mut buffer = [0u8; 8 * 1024];
    let mut exceeded_budget = false;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = retain_limit.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        if retained.len() > max || keep < read {
            exceeded_budget = true;
        }
    }

    Ok((
        String::from_utf8_lossy(&retained).into_owned(),
        exceeded_budget,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bounded_drain_keeps_one_overflow_probe_byte() {
        let input = vec![b'x'; 1024 * 1024];
        let (text, truncated) = drain_bounded(Cursor::new(input), 32).unwrap();
        assert_eq!(text, "x".repeat(33));
        assert!(truncated);
    }

    #[test]
    fn exact_budget_is_not_truncated() {
        let (text, truncated) = drain_bounded(Cursor::new(b"1234"), 4).unwrap();
        assert_eq!(text, "1234");
        assert!(!truncated);
    }

    #[test]
    fn zero_budget_keeps_only_the_overflow_probe() {
        let (text, truncated) = drain_bounded(Cursor::new(b"content"), 0).unwrap();
        assert_eq!(text, "c");
        assert!(truncated);
    }

    #[test]
    fn truncation_marker_stays_inside_budget() {
        let output = truncate_with_marker(&"x".repeat(100), 32, true, "\n[truncated]");
        assert!(output.ends_with("[truncated]"));
        assert!(output.len() <= 32);
    }
}
