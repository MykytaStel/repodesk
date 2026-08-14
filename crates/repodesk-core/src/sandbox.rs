//! Legacy compatibility facade for policy-checked command execution.
//!
//! Despite the historical module/function names, commands are not OS-isolated.
//! New code should use [`crate::command_execution`] and [`crate::command_policy`].

pub use crate::command_sandbox::{
    SandboxPlan, SandboxRule, format_sandbox_plan, plan_command, sandbox_policy, sandbox_rules,
};

pub fn run_sandboxed_command(command: &str) -> Result<std::process::Output, String> {
    let output = crate::command_execution::run_policy_checked_command(command)?;
    Ok(std::process::Output {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_sandboxed_command_blocks_dangerous() {
        let result = run_sandboxed_command("rm -rf /");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blocked"));
    }

    #[test]
    fn run_sandboxed_command_warns_unknown() {
        let result = run_sandboxed_command("unknown_command_abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires manual confirmation"));
    }
}
