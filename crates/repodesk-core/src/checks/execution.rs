use std::io::Read;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

use super::CheckCommandResult;

const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

const ALLOWED_CHECK_BINARIES: [&str; 26] = [
    "cargo",
    "npm",
    "pnpm",
    "yarn",
    "python",
    "python3",
    "pytest",
    "go",
    "make",
    "gradle",
    "mvn",
    "deno",
    "npx",
    "bun",
    "jest",
    "vitest",
    "eslint",
    "prettier",
    "flake8",
    "mypy",
    "black",
    "repopilot",
    "snyk",
    "sonar-scanner",
    "trivy",
    "checkmarx",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedCheckCommand {
    executable: String,
    args: Vec<String>,
}

pub(super) fn parse_allowed_check_command(command: &str) -> Result<ParsedCheckCommand, String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("Command is empty".to_string());
    }

    // Keep the existing conservative policy even though approved checks are no
    // longer handed to a shell. This avoids silently widening the capability
    // surface while the execution boundary is being hardened.
    let dangerous_chars = [
        ';', '&', '|', '<', '>', '$', '`', '\n', '\r', '(', ')', '\\',
    ];
    for &ch in &dangerous_chars {
        if trimmed.contains(ch) {
            return Err(format!("Command contains restricted character '{ch}'"));
        }
    }

    if trimmed.to_ascii_lowercase().contains("://") {
        return Err("Command arguments may not contain a URL".to_string());
    }

    let tokens = shlex::split(trimmed)
        .ok_or_else(|| "Command has malformed or unclosed quoting".to_string())?;
    let (executable, args) = tokens
        .split_first()
        .ok_or_else(|| "Could not parse command executable".to_string())?;

    if !ALLOWED_CHECK_BINARIES.contains(&executable.as_str()) {
        return Err(format!(
            "Executable '{executable}' is not in the allowed list of check tools"
        ));
    }

    Ok(ParsedCheckCommand {
        executable: executable.clone(),
        args: args.to_vec(),
    })
}

pub(super) fn run_parsed_check_with_timeout(
    display_command: &str,
    parsed: &ParsedCheckCommand,
    cwd: &Path,
    timeout_secs: u64,
) -> CheckCommandResult {
    if let Err(error) = ensure_tree_termination_available() {
        return failed_without_spawn(
            display_command,
            format!("Execution boundary unavailable: {error}"),
        );
    }

    let started = Instant::now();
    let mut command = Command::new(&parsed.executable);
    command
        .args(&parsed.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return CheckCommandResult {
                command: display_command.to_string(),
                status: "failed".to_string(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis(),
                stdout: String::new(),
                stderr: format!(
                    "Failed to spawn approved executable '{}' directly: {error}",
                    parsed.executable
                ),
            };
        }
    };

    let stdout_pipe = match child.stdout.take() {
        Some(pipe) => pipe,
        None => {
            let termination_error = terminate_process_tree(&mut child).err();
            return pipe_failure_result(
                display_command,
                started,
                "stdout",
                termination_error.as_deref(),
            );
        }
    };
    let stderr_pipe = match child.stderr.take() {
        Some(pipe) => pipe,
        None => {
            let termination_error = terminate_process_tree(&mut child).err();
            return pipe_failure_result(
                display_command,
                started,
                "stderr",
                termination_error.as_deref(),
            );
        }
    };

    let (tx_out, rx_out) = mpsc::channel();
    std::thread::spawn(move || {
        let mut pipe = stdout_pipe;
        let mut output = String::new();
        let _ = pipe.read_to_string(&mut output);
        let _ = tx_out.send(output);
    });

    let (tx_err, rx_err) = mpsc::channel();
    std::thread::spawn(move || {
        let mut pipe = stderr_pipe;
        let mut output = String::new();
        let _ = pipe.read_to_string(&mut output);
        let _ = tx_err.send(output);
    });

    let timeout = Duration::from_secs(timeout_secs);
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => finish_completed_check(
            display_command,
            started,
            status,
            &mut child,
            &rx_out,
            &rx_err,
        ),
        Ok(None) => {
            let termination_error = terminate_process_tree(&mut child).err();
            let mut stderr = format!(
                "Command timed out after {timeout_secs}s; RepoDesk terminated the check process tree"
            );
            if let Some(error) = termination_error {
                stderr.push_str(&format!(". Process-tree termination reported: {error}"));
            }

            CheckCommandResult {
                command: display_command.to_string(),
                status: "timeout".to_string(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis(),
                stdout: rx_out.try_recv().unwrap_or_default(),
                stderr,
            }
        }
        Err(error) => {
            let termination_error = terminate_process_tree(&mut child).err();
            let mut stderr = format!("Failed to wait for command: {error}");
            if let Some(error) = termination_error {
                stderr.push_str(&format!(". Process-tree termination reported: {error}"));
            }
            CheckCommandResult {
                command: display_command.to_string(),
                status: "failed".to_string(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis(),
                stdout: rx_out.try_recv().unwrap_or_default(),
                stderr,
            }
        }
    }
}

fn finish_completed_check(
    command: &str,
    started: Instant,
    status: ExitStatus,
    child: &mut Child,
    rx_out: &Receiver<String>,
    rx_err: &Receiver<String>,
) -> CheckCommandResult {
    let stdout = match rx_out.recv_timeout(OUTPUT_DRAIN_TIMEOUT) {
        Ok(output) => output,
        Err(error) => {
            return output_drain_failure_result(
                command, started, status, child, rx_out, rx_err, "stdout", error,
            );
        }
    };
    let stderr = match rx_err.recv_timeout(OUTPUT_DRAIN_TIMEOUT) {
        Ok(output) => output,
        Err(error) => {
            return output_drain_failure_result(
                command, started, status, child, rx_out, rx_err, "stderr", error,
            );
        }
    };

    CheckCommandResult {
        command: command.to_string(),
        status: if status.success() { "passed" } else { "failed" }.to_string(),
        exit_code: status.code(),
        duration_ms: started.elapsed().as_millis(),
        stdout,
        stderr,
    }
}

#[allow(clippy::too_many_arguments)]
fn output_drain_failure_result(
    command: &str,
    started: Instant,
    status: ExitStatus,
    child: &mut Child,
    rx_out: &Receiver<String>,
    rx_err: &Receiver<String>,
    pipe: &str,
    drain_error: RecvTimeoutError,
) -> CheckCommandResult {
    let termination_error = terminate_process_tree(child).err();
    let stdout = rx_out
        .recv_timeout(OUTPUT_DRAIN_TIMEOUT)
        .unwrap_or_default();
    let captured_stderr = rx_err
        .recv_timeout(OUTPUT_DRAIN_TIMEOUT)
        .unwrap_or_default();
    let reason = match drain_error {
        RecvTimeoutError::Timeout => format!(
            "Check leader exited but the {pipe} pipe stayed open; a descendant process likely outlived the approved check leader"
        ),
        RecvTimeoutError::Disconnected => {
            format!("Check leader exited but the {pipe} capture worker disconnected unexpectedly")
        }
    };
    let mut stderr = if captured_stderr.is_empty() {
        reason
    } else {
        format!("{captured_stderr}\n{reason}")
    };
    if let Some(error) = termination_error {
        stderr.push_str(&format!(". Process-tree termination reported: {error}"));
    } else {
        stderr.push_str(". RepoDesk terminated the remaining check process tree");
    }

    CheckCommandResult {
        command: command.to_string(),
        status: "failed".to_string(),
        exit_code: status.code(),
        duration_ms: started.elapsed().as_millis(),
        stdout,
        stderr,
    }
}

fn failed_without_spawn(command: &str, stderr: String) -> CheckCommandResult {
    CheckCommandResult {
        command: command.to_string(),
        status: "failed".to_string(),
        exit_code: None,
        duration_ms: 0,
        stdout: String::new(),
        stderr,
    }
}

fn pipe_failure_result(
    command: &str,
    started: Instant,
    pipe: &str,
    termination_error: Option<&str>,
) -> CheckCommandResult {
    let mut stderr = format!("Failed to take child process {pipe} pipe");
    if let Some(error) = termination_error {
        stderr.push_str(&format!(". Process-tree termination reported: {error}"));
    }
    CheckCommandResult {
        command: command.to_string(),
        status: "failed".to_string(),
        exit_code: None,
        duration_ms: started.elapsed().as_millis(),
        stdout: String::new(),
        stderr,
    }
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn ensure_tree_termination_available() -> Result<(), String> {
    unix_kill_binary()
        .map(|_| ())
        .ok_or_else(|| "neither /bin/kill nor /usr/bin/kill is available".to_string())
}

#[cfg(windows)]
fn ensure_tree_termination_available() -> Result<(), String> {
    let path = windows_taskkill_path();
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("{} is unavailable", path.display()))
    }
}

#[cfg(not(any(unix, windows)))]
fn ensure_tree_termination_available() -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn unix_kill_binary() -> Option<&'static str> {
    ["/bin/kill", "/usr/bin/kill"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
}

#[cfg(windows)]
fn windows_taskkill_path() -> PathBuf {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(system_root)
        .join("System32")
        .join("taskkill.exe")
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child) -> Result<(), String> {
    let kill_binary = unix_kill_binary()
        .ok_or_else(|| "process-group kill executable disappeared after preflight".to_string())?;
    let process_group = format!("-{}", child.id());
    let group_status = Command::new(kill_binary)
        .args(["-KILL", "--", &process_group])
        .status()
        .map_err(|error| format!("failed to invoke process-group kill: {error}"))?;

    let _ = child.kill();
    let wait_result = child.wait();
    if !group_status.success() {
        return Err(format!(
            "process-group kill exited with {group_status}; direct child kill was attempted"
        ));
    }
    wait_result
        .map(|_| ())
        .map_err(|error| format!("failed to reap check process after group kill: {error}"))
}

#[cfg(windows)]
fn terminate_process_tree(child: &mut Child) -> Result<(), String> {
    let taskkill = windows_taskkill_path();
    let pid = child.id().to_string();
    let tree_status = Command::new(&taskkill)
        .args(["/PID", &pid, "/T", "/F"])
        .status()
        .map_err(|error| format!("failed to invoke {}: {error}", taskkill.display()))?;

    let _ = child.kill();
    let wait_result = child.wait();
    if !tree_status.success() {
        return Err(format!(
            "taskkill process-tree termination exited with {tree_status}; direct child kill was attempted"
        ));
    }
    wait_result
        .map(|_| ())
        .map_err(|error| format!("failed to reap check process after taskkill: {error}"))
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(child: &mut Child) -> Result<(), String> {
    child
        .kill()
        .map_err(|error| format!("failed to kill check process: {error}"))?;
    child
        .wait()
        .map(|_| ())
        .map_err(|error| format!("failed to reap check process: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn quoted_arguments_are_parsed_into_one_argv_element() {
        let parsed = parse_allowed_check_command("cargo test \"name with spaces\"").unwrap();
        assert_eq!(parsed.executable, "cargo");
        assert_eq!(parsed.args, vec!["test", "name with spaces"]);
    }

    #[test]
    fn malformed_quoting_is_rejected_before_spawn() {
        let error = parse_allowed_check_command("cargo test \"unfinished").unwrap_err();
        assert!(error.contains("quoting"));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_descendant_process_group() {
        if Command::new("python3").arg("--version").output().is_err() {
            return;
        }

        let dir = tempdir().unwrap();
        let marker = dir.path().join("descendant-survived.txt");
        let child_code = format!(
            "import pathlib,time; time.sleep(2); pathlib.Path({:?}).write_text('survived')",
            marker.to_string_lossy()
        );
        let parent_code = format!(
            "import subprocess,sys,time; subprocess.Popen([sys.executable,'-c',{:?}]); time.sleep(10)",
            child_code
        );
        let parsed = ParsedCheckCommand {
            executable: "python3".to_string(),
            args: vec!["-c".to_string(), parent_code],
        };

        let result =
            run_parsed_check_with_timeout("python descendant fixture", &parsed, dir.path(), 1);
        assert_eq!(result.status, "timeout", "stderr: {}", result.stderr);
        std::thread::sleep(Duration::from_secs(3));
        assert!(
            !marker.exists(),
            "descendant outlived the timed-out check process group"
        );
    }

    #[cfg(unix)]
    #[test]
    fn completed_leader_cannot_leave_descendant_holding_stdio_open() {
        if Command::new("python3").arg("--version").output().is_err() {
            return;
        }

        let dir = tempdir().unwrap();
        let marker = dir.path().join("background-descendant-survived.txt");
        let child_code = format!(
            "import pathlib,time; time.sleep(4); pathlib.Path({:?}).write_text('survived')",
            marker.to_string_lossy()
        );
        let parent_code = format!(
            "import subprocess,sys; subprocess.Popen([sys.executable,'-c',{:?}])",
            child_code
        );
        let parsed = ParsedCheckCommand {
            executable: "python3".to_string(),
            args: vec!["-c".to_string(), parent_code],
        };

        let result = run_parsed_check_with_timeout(
            "python background descendant fixture",
            &parsed,
            dir.path(),
            10,
        );
        assert_eq!(result.status, "failed", "stderr: {}", result.stderr);
        assert!(result.stderr.contains("pipe stayed open"));
        std::thread::sleep(Duration::from_secs(3));
        assert!(
            !marker.exists(),
            "descendant outlived a completed check leader after pipe-drain cleanup"
        );
    }
}
