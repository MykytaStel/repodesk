//! Direct command execution after RepoDesk's command policy has allowed it.
//!
//! The runner never invokes a shell and keeps stdout/stderr retention bounded,
//! but the child still runs with the host user's OS privileges. Policy approval
//! is therefore not process containment.

use std::process::{Command, ExitStatus, Stdio};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::command_policy::{CommandPolicyPlan, evaluate_command};
use crate::process_io::{BoundedBytes, drain_bounded_bytes};

pub const MAX_POLICY_COMMAND_STDOUT_BYTES: usize = 256 * 1024;
pub const MAX_POLICY_COMMAND_STDERR_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandIsolation {
    /// The command is policy-checked and direct-spawned, but it is not contained
    /// by an OS sandbox, container, namespace, seccomp profile, or equivalent.
    NotOsIsolated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyCheckedCommandPlan {
    pub policy: CommandPolicyPlan,
    pub isolation: CommandIsolation,
}

#[derive(Debug)]
pub struct PolicyCheckedCommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub isolation: CommandIsolation,
}

pub fn prepare_policy_checked_command(command: &str) -> Result<PolicyCheckedCommandPlan, String> {
    let policy = evaluate_command(command);

    if policy.verdict == "block" {
        return Err(format!("Command blocked by policy: {}", policy.reason));
    }
    if policy.required_confirmation {
        return Err("Command requires manual confirmation/override.".to_string());
    }
    if policy.verdict != "allow" {
        return Err(format!(
            "Command policy did not explicitly allow execution (verdict: {}).",
            policy.verdict
        ));
    }

    Ok(PolicyCheckedCommandPlan {
        policy,
        isolation: CommandIsolation::NotOsIsolated,
    })
}

/// Execute a command that the policy explicitly allows.
///
/// This uses argv directly rather than a shell and drains both output pipes
/// concurrently. It does **not** provide OS/process isolation.
pub fn run_policy_checked_command(command: &str) -> Result<PolicyCheckedCommandOutput, String> {
    let plan = prepare_policy_checked_command(command)?;
    let tokens = shlex::split(&plan.policy.command)
        .ok_or_else(|| "Command parse error: unmatched quote.".to_string())?;
    let (program, args) = tokens
        .split_first()
        .ok_or_else(|| "Command cannot be empty.".to_string())?;

    let captured = spawn_bounded(
        program,
        args,
        MAX_POLICY_COMMAND_STDOUT_BYTES,
        MAX_POLICY_COMMAND_STDERR_BYTES,
    )?;

    Ok(PolicyCheckedCommandOutput {
        status: captured.status,
        stdout: captured.stdout.bytes,
        stderr: captured.stderr.bytes,
        stdout_truncated: captured.stdout.truncated,
        stderr_truncated: captured.stderr.truncated,
        isolation: plan.isolation,
    })
}

struct CapturedProcessOutput {
    status: ExitStatus,
    stdout: BoundedBytes,
    stderr: BoundedBytes,
}

fn spawn_bounded(
    program: &str,
    args: &[String],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<CapturedProcessOutput, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Command execution failed: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Command stdout pipe was not available.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Command stderr pipe was not available.".to_string())?;

    let stdout_reader = thread::spawn(move || drain_bounded_bytes(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || drain_bounded_bytes(stderr, stderr_limit));

    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(format!("Command wait failed: {error}"));
        }
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| "Command stdout reader panicked.".to_string())?
        .map_err(|error| format!("Command stdout read failed: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Command stderr reader panicked.".to_string())?
        .map_err(|error| format!("Command stderr read failed: {error}"))?;

    Ok(CapturedProcessOutput {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_plan_is_explicitly_not_os_isolated() {
        let plan = prepare_policy_checked_command("git status").unwrap();
        assert_eq!(plan.policy.verdict, "allow");
        assert_eq!(plan.isolation, CommandIsolation::NotOsIsolated);
    }

    #[test]
    fn blocked_and_unknown_commands_fail_before_spawn() {
        let blocked = prepare_policy_checked_command("rm -rf /").unwrap_err();
        assert!(blocked.contains("blocked by policy"));

        let unknown = prepare_policy_checked_command("definitely-not-a-real-command").unwrap_err();
        assert!(unknown.contains("requires manual confirmation"));
    }

    #[cfg(unix)]
    #[test]
    fn noisy_child_is_fully_drained_with_bounded_retention() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("noisy-command");
        std::fs::write(
            &script,
            "#!/bin/sh\ni=0\nwhile [ $i -lt 5000 ]; do printf 'stdout-line\\n'; printf 'stderr-line\\n' >&2; i=$((i + 1)); done\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let output = spawn_bounded(&script.display().to_string(), &[], 128, 96)
            .expect("verbose process should complete while both pipes are drained");
        assert!(output.status.success());
        assert_eq!(output.stdout.bytes.len(), 128);
        assert_eq!(output.stderr.bytes.len(), 96);
        assert!(output.stdout.truncated);
        assert!(output.stderr.truncated);
    }
}
