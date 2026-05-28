use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::errors::RepoDeskResult;
use crate::init;
use crate::projects::get_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone)]
pub struct ChecksRunResult {
    pub log_file: PathBuf,
    pub summary_file: PathBuf,
    pub success: bool,
    pub commands: Vec<CheckCommandResult>,
}

#[derive(Debug, Clone)]
pub struct CheckCommandResult {
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct ChecksLastResult {
    pub log_file: PathBuf,
    pub summary_file: PathBuf,
    pub summary: String,
}

pub fn run_checks() -> RepoDeskResult<ChecksRunResult> {
    init::init_home()?;

    let project = get_active_project()?;
    let task = show_active_task()?;

    let log_file = task.config.run_dir.join("checks.log");
    let summary_file = task.config.run_dir.join("checks-summary.md");

    let mut log = File::create(&log_file)?;

    writeln!(log, "RepoDesk checks run")?;
    writeln!(log, "Timestamp: {}", Utc::now().to_rfc3339())?;
    writeln!(log, "Project: {}", project.name)?;
    writeln!(log, "Path: {}", project.path.display())?;
    writeln!(log, "Task: {}", task.config.id)?;
    writeln!(log)?;

    if project.checks.is_empty() {
        writeln!(log, "No checks configured for this project.")?;

        let result = ChecksRunResult {
            log_file,
            summary_file,
            success: true,
            commands: Vec::new(),
        };

        write_run_summary(&result, "No checks configured.")?;
        return Ok(result);
    }

    let mut results = Vec::new();

    for check in &project.checks {
        let result = run_shell_command(check, &project.path);

        writeln!(log, "==============================")?;
        writeln!(log, "Command: {}", result.command)?;
        writeln!(log, "Status: {}", result.status)?;
        writeln!(log, "Exit code: {:?}", result.exit_code)?;
        writeln!(log, "Duration ms: {}", result.duration_ms)?;
        writeln!(log, "------------------------------")?;
        writeln!(log, "STDOUT:")?;
        writeln!(log, "{}", result.stdout)?;
        writeln!(log, "------------------------------")?;
        writeln!(log, "STDERR:")?;
        writeln!(log, "{}", result.stderr)?;
        writeln!(log)?;

        results.push(result);
    }

    let success = results.iter().all(|result| result.status == "passed");

    let result = ChecksRunResult {
        log_file,
        summary_file,
        success,
        commands: results,
    };

    write_run_summary(&result, "Generated after checks run.")?;

    Ok(result)
}

pub fn last_checks() -> RepoDeskResult<ChecksLastResult> {
    let task = show_active_task()?;
    let log_file = task.config.run_dir.join("checks.log");
    let summary_file = task.config.run_dir.join("checks-summary.md");

    let summary = if summary_file.exists() {
        fs::read_to_string(&summary_file)?
    } else {
        "No checks summary found. Run `repodesk checks run` first.\n".to_string()
    };

    Ok(ChecksLastResult {
        log_file,
        summary_file,
        summary,
    })
}

pub fn summarize_last_checks() -> RepoDeskResult<ChecksLastResult> {
    let task = show_active_task()?;
    let log_file = task.config.run_dir.join("checks.log");
    let summary_file = task.config.run_dir.join("checks-summary.md");

    let log_content = if log_file.exists() {
        fs::read_to_string(&log_file)?
    } else {
        "No checks.log found. Run `repodesk checks run` first.\n".to_string()
    };

    let summary = format!(
        r#"# Checks Summary

Generated: `{}`
Task: `{}`

## Tail

```txt
{}
```
"#,
        Utc::now().to_rfc3339(),
        task.config.id,
        tail_lines(&log_content, 140)
    );

    fs::write(&summary_file, &summary)?;

    Ok(ChecksLastResult {
        log_file,
        summary_file,
        summary,
    })
}

fn run_shell_command(command: &str, cwd: &Path) -> CheckCommandResult {
    const TIMEOUT_SECS: u64 = 120;
    let started = Instant::now();
    let command_owned = command.to_string();
    let cwd_owned = cwd.to_path_buf();

    let mut child = match if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &command_owned])
            .current_dir(&cwd_owned)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } else {
        Command::new("sh")
            .args(["-c", &command_owned])
            .current_dir(&cwd_owned)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    } {
        Ok(child) => child,
        Err(error) => {
            return CheckCommandResult {
                command: command.to_string(),
                status: "failed".to_string(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis(),
                stdout: String::new(),
                stderr: format!("Failed to spawn command: {error}"),
            };
        }
    };

    let mut stdout_pipe = child.stdout.take().unwrap();
    let mut stderr_pipe = child.stderr.take().unwrap();

    let (tx_out, rx_out) = mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        use std::io::Read;
        let _ = stdout_pipe.read_to_string(&mut s);
        let _ = tx_out.send(s);
    });

    let (tx_err, rx_err) = mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        use std::io::Read;
        let _ = stderr_pipe.read_to_string(&mut s);
        let _ = tx_err.send(s);
    });

    let timeout = Duration::from_secs(TIMEOUT_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = rx_out.recv().unwrap_or_default();
                let stderr = rx_err.recv().unwrap_or_default();
                let duration_ms = started.elapsed().as_millis();
                let status_str = if status.success() { "passed" } else { "failed" };
                return CheckCommandResult {
                    command: command.to_string(),
                    status: status_str.to_string(),
                    exit_code: status.code(),
                    duration_ms,
                    stdout,
                    stderr,
                };
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let duration_ms = started.elapsed().as_millis();
                    return CheckCommandResult {
                        command: command.to_string(),
                        status: "timeout".to_string(),
                        exit_code: None,
                        duration_ms,
                        stdout: String::new(),
                        stderr: format!("Command timed out after {}s and was killed", TIMEOUT_SECS),
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let duration_ms = started.elapsed().as_millis();
                return CheckCommandResult {
                    command: command.to_string(),
                    status: "failed".to_string(),
                    exit_code: None,
                    duration_ms,
                    stdout: String::new(),
                    stderr: format!("Failed to wait for command: {error}"),
                };
            }
        }
    }
}

fn write_run_summary(result: &ChecksRunResult, note: &str) -> RepoDeskResult<()> {
    let mut summary = String::new();

    summary.push_str("# Checks Summary\n\n");
    summary.push_str(&format!("Generated: `{}`\n", Utc::now().to_rfc3339()));
    summary.push_str(&format!(
        "Overall status: `{}`\n",
        if result.success { "passed" } else { "failed" }
    ));
    summary.push_str(&format!("Note: {}\n\n", note));

    summary.push_str("## Commands\n\n");

    if result.commands.is_empty() {
        summary.push_str("- No checks configured.\n");
    } else {
        for command in &result.commands {
            summary.push_str(&format!(
                "- `{}` => `{}` in {}ms, exit={:?}\n",
                command.command, command.status, command.duration_ms, command.exit_code
            ));
        }
    }

    let failed = result
        .commands
        .iter()
        .filter(|command| command.status == "failed")
        .collect::<Vec<_>>();

    if !failed.is_empty() {
        summary.push_str("\n## Failed Command Tails\n\n");

        for command in failed {
            let combined = format!("STDOUT:\n{}\n\nSTDERR:\n{}", command.stdout, command.stderr);
            summary.push_str(&format!("### `{}`\n\n", command.command));
            summary.push_str("```txt\n");
            summary.push_str(&tail_lines(&combined, 120));
            summary.push_str("\n```\n\n");
        }
    }

    summary.push_str("## Agent Instructions\n\n");
    summary.push_str("- Do not paste the full checks.log into a paid model by default.\n");
    summary.push_str("- Use this summary first. Ask for the full log only if needed.\n");
    summary.push_str("- Fix the first failing check before expanding scope.\n");

    fs::write(&result.summary_file, summary)?;

    Ok(())
}

fn tail_lines(value: &str, max_lines: usize) -> String {
    let lines = value.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
