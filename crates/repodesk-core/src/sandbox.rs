pub use crate::command_sandbox::{
    SandboxPlan, SandboxRule, format_sandbox_plan, plan_command, sandbox_policy, sandbox_rules,
};

pub fn run_sandboxed_command(command: &str) -> Result<std::process::Output, String> {
    let plan = plan_command(command);
    if plan.verdict == "block" {
        return Err(format!("Command blocked by sandbox policy: {}", plan.reason));
    }
    if plan.required_confirmation {
        return Err("Command requires manual confirmation/override.".to_string());
    }

    let tokens = shlex::split(command).ok_or_else(|| "Malformed command".to_string())?;
    if tokens.is_empty() {
        return Err("Empty command".into());
    }

    let mut cmd = std::process::Command::new(&tokens[0]);
    cmd.args(&tokens[1..]);

    let output = cmd.output().map_err(|e| e.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_sandboxed_command_blocks_dangerous() {
        let res = run_sandboxed_command("rm -rf /");
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("blocked"));
    }

    #[test]
    fn test_run_sandboxed_command_warns_unknown() {
        let res = run_sandboxed_command("unknown_command_abc");
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("requires manual confirmation"));
    }
}


