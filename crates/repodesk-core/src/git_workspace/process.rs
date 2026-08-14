use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

#[derive(Debug)]
pub(crate) struct BoundedGitCapture {
    pub(crate) text: String,
    pub(crate) truncated: bool,
}

pub fn run_git_captured(project_path: &Path, args: &[&str]) -> String {
    match Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.trim().is_empty() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                stderr.to_string()
            }
        }
        Err(error) => format!("failed to run git {}: {}", args.join(" "), error),
    }
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
                text: format!("failed to run git {}: {}", args.join(" "), error),
                truncated: false,
            };
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return BoundedGitCapture {
            text: "failed to capture git stdout".to_string(),
            truncated: false,
        };
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return BoundedGitCapture {
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
                text: format!("failed to wait for git {}: {}", args.join(" "), error),
                truncated: false,
            };
        }
    };

    let stdout = stdout_reader.join().ok().and_then(Result::ok);
    let stderr = stderr_reader.join().ok().and_then(Result::ok);

    let Some((stdout, stdout_truncated)) = stdout else {
        return BoundedGitCapture {
            text: "failed to read git stdout".to_string(),
            truncated: false,
        };
    };
    let Some((stderr, stderr_truncated)) = stderr else {
        return BoundedGitCapture {
            text: "failed to read git stderr".to_string(),
            truncated: false,
        };
    };

    let (text, truncated) = if status.success() || stderr.trim().is_empty() {
        (stdout, stdout_truncated)
    } else {
        (stderr, stderr_truncated)
    };

    BoundedGitCapture { text, truncated }
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
}
