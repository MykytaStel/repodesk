//! Legacy compatibility names for the command-policy API.
//!
//! Historical callers used “sandbox” terminology. These aliases perform only
//! command classification/gating; they do not imply OS/process isolation.

pub use crate::command_policy::{
    CommandPolicyPlan as SandboxPlan, CommandPolicyRule as SandboxRule,
};
use crate::command_policy::{
    command_policy_rules, command_policy_text, evaluate_command, format_command_policy_plan,
};

pub fn sandbox_rules() -> Vec<SandboxRule> {
    command_policy_rules()
}

pub fn sandbox_policy() -> String {
    command_policy_text()
}

pub fn plan_command(command: &str) -> SandboxPlan {
    evaluate_command(command)
}

pub fn format_sandbox_plan(plan: &SandboxPlan) -> String {
    format_command_policy_plan(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_plan_matches_canonical_policy() {
        assert_eq!(plan_command("git status"), evaluate_command("git status"));
        assert!(sandbox_policy().contains("not an OS/process sandbox"));
    }
}
