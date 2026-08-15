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
fn executable_script(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let script = dir.join("agent");
    std::fs::write(&script, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[cfg(unix)]
fn command_for_script(script: &std::path::Path) -> CodingAgentCommandSpec {
    CodingAgentCommandSpec {
        executor_id: "codex_cli".to_string(),
        label: "Test Agent".to_string(),
        program: script.display().to_string(),
        args: Vec::new(),
        stdin_required: true,
        cwd_required: true,
        writes_allowed: true,
        command_preview: "agent [stdin: bounded prompt]".to_string(),
    }
}

#[cfg(unix)]
#[test]
fn run_command_captures_stdout_and_stderr() {
    let dir = tempfile::TempDir::new().unwrap();
    let script = executable_script(dir.path(), "#!/bin/sh\ncat\necho stderr-line >&2\n");
    let command = command_for_script(&script);

    let result =
        run_coding_agent_command(&command, "hello prompt", dir.path(), dir.path(), 5).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.stdout, "hello prompt");
    assert!(result.stderr.contains("stderr-line"));
    assert!(!result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert!(!result.stdout_log_truncated);
    assert!(!result.stderr_log_truncated);
    assert!(result.output_capture_issues.is_empty());
    assert!(std::path::Path::new(&result.stdout_path).exists());
    assert!(std::path::Path::new(&result.stderr_path).exists());
}

#[cfg(unix)]
#[test]
fn verbose_run_is_drained_with_hard_memory_and_disk_caps() {
    let dir = tempfile::TempDir::new().unwrap();
    let script = executable_script(
        dir.path(),
        "#!/bin/sh\ni=0\nwhile [ $i -lt 400 ]; do printf 'abcdefghijklmnop'; printf 'qrstuvwxyz012345' >&2; i=$((i + 1)); done\n",
    );
    let command = command_for_script(&script);
    let limits = runtime::OutputLimits {
        stdout_record_bytes: 64,
        stderr_record_bytes: 48,
        stdout_log_bytes: 128,
        stderr_log_bytes: 96,
    };

    let result = runtime::run_with_limits(&command, "prompt", dir.path(), dir.path(), 5, limits)
        .expect("verbose executor should finish while both pipes are continuously drained");

    assert_eq!(result.status, "ok");
    assert!(result.stdout_truncated);
    assert!(result.stderr_truncated);
    assert!(result.stdout_log_truncated);
    assert!(result.stderr_log_truncated);
    assert!(result.output_capture_issues.is_empty());
    assert!(result.stdout.len() <= limits.stdout_record_bytes);
    assert!(result.stderr.len() <= limits.stderr_record_bytes);
    assert_eq!(
        std::fs::metadata(&result.stdout_path).unwrap().len(),
        limits.stdout_log_bytes as u64
    );
    assert_eq!(
        std::fs::metadata(&result.stderr_path).unwrap().len(),
        limits.stderr_log_bytes as u64
    );
}

#[cfg(unix)]
#[test]
fn probe_version_reads_first_stdout_line() {
    let dir = tempfile::TempDir::new().unwrap();
    let script = executable_script(
        dir.path(),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'fake-agent 1.2.3'; fi\n",
    );

    let version = probe_version(&script.display().to_string());
    assert_eq!(version.as_deref(), Some("fake-agent 1.2.3"));
}

#[cfg(unix)]
#[test]
fn probe_version_returns_none_on_nonzero_exit() {
    let dir = tempfile::TempDir::new().unwrap();
    let script = executable_script(dir.path(), "#!/bin/sh\nexit 1\n");

    assert_eq!(probe_version(&script.display().to_string()), None);
}

#[test]
fn probed_availability_for_missing_binary_skips_probe() {
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
    let script = executable_script(dir.path(), "#!/bin/sh\nsleep 5\n");
    let command = command_for_script(&script);

    let result = run_coding_agent_command(&command, "prompt", dir.path(), dir.path(), 1)
        .expect("timeout result");
    assert_eq!(result.status, "timed_out");
    assert!(result.timed_out);
}

#[cfg(unix)]
#[test]
fn runtime_timeout_includes_blocked_stdin_delivery() {
    use std::time::{Duration, Instant};

    let dir = tempfile::TempDir::new().unwrap();
    let script = executable_script(dir.path(), "#!/bin/sh\nsleep 3\n");
    let command = command_for_script(&script);
    let prompt = "x".repeat(4 * 1024 * 1024);
    let started = Instant::now();

    let result = run_coding_agent_command(&command, &prompt, dir.path(), dir.path(), 1)
        .expect("blocked stdin delivery must resolve through the executor timeout");

    assert_eq!(result.status, "timed_out");
    assert!(result.timed_out);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "executor timeout must include stdin delivery; elapsed={:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn run_command_captures_git_changeset() {
    let repo = tempfile::TempDir::new().unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    };
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);

    let out = tempfile::TempDir::new().unwrap();
    let script = executable_script(
        out.path(),
        "#!/bin/sh\ncat > /dev/null\necho changed >> seed.txt\necho new > added.txt\n",
    );
    let command = command_for_script(&script);

    let result = run_coding_agent_command(&command, "prompt", repo.path(), out.path(), 5).unwrap();
    assert_eq!(result.status, "ok");
    let paths: Vec<&str> = result
        .changed_files
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert!(paths.contains(&"seed.txt"), "tracked change: {paths:?}");
    assert!(paths.contains(&"added.txt"), "new file: {paths:?}");
    assert!(result.diff.contains("seed.txt"));
    assert!(!result.diff_truncated);
    let diff_path = result.diff_path.expect("diff receipt written");
    assert!(std::path::Path::new(&diff_path).exists());
}

#[cfg(unix)]
#[test]
fn post_launch_provenance_failure_still_returns_execution_receipt() {
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }

    let repo = tempfile::TempDir::new().unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);

    let out = tempfile::TempDir::new().unwrap();
    let script = executable_script(
        out.path(),
        "#!/bin/sh\ncat > /dev/null\necho changed >> seed.txt\nrm -rf .git\necho agent-finished\n",
    );
    let command = command_for_script(&script);

    let result = run_coding_agent_command(&command, "prompt", repo.path(), out.path(), 5)
        .expect("once spawn succeeds, provenance failure must not erase the execution receipt");

    assert_eq!(result.status, "failed");
    assert!(result.stdout.contains("agent-finished"));
    assert!(result.changed_files.is_empty());
}
