//! Command classification and execution-gating policy.
//!
//! This module answers whether RepoDesk may hand a command to a process runner.
//! It does **not** provide OS/process isolation, filesystem containment, syscall
//! filtering, privilege separation, or a shell sandbox.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandPolicyRule {
    pub name: String,
    pub verdict: String,
    pub pattern: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandPolicyPlan {
    pub command: String,
    pub verdict: String,
    pub reason: String,
    pub required_confirmation: bool,
    pub matched_rules: Vec<CommandPolicyRule>,
}

#[derive(Clone, Copy)]
struct RuleSpec {
    name: &'static str,
    verdict: &'static str,
    pattern: &'static str,
    reason: &'static str,
}

const RULE_SPECS: &[RuleSpec] = &[
    RuleSpec {
        name: "dangerous_delete",
        verdict: "block",
        pattern: "rm -rf /, rm -rf ., find ... -delete",
        reason: "Destructive filesystem command.",
    },
    RuleSpec {
        name: "privileged_shell",
        verdict: "block",
        pattern: "sudo, su, chmod 777",
        reason: "Privilege escalation or broad permission change.",
    },
    RuleSpec {
        name: "remote_shell_pipe",
        verdict: "block",
        pattern: "curl|sh, wget|sh, bash <(curl ...)",
        reason: "Remote code execution without review.",
    },
    RuleSpec {
        name: "secret_access",
        verdict: "block",
        pattern: ".env, *.pem, *.key, credential stores and secret artifacts",
        reason: "Potential secret exposure.",
    },
    RuleSpec {
        name: "publish_or_force_push",
        verdict: "warn",
        pattern: "git push --force, npm publish, cargo publish",
        reason: "Release or history-changing command requires explicit user intent.",
    },
    RuleSpec {
        name: "safe_checks",
        verdict: "allow",
        pattern: "cargo check/fmt/test/clippy, pnpm test/build/typecheck, git status/diff/log",
        reason: "Read-only or verification-focused command.",
    },
];

impl From<RuleSpec> for CommandPolicyRule {
    fn from(rule: RuleSpec) -> Self {
        Self {
            name: rule.name.to_string(),
            verdict: rule.verdict.to_string(),
            pattern: rule.pattern.to_string(),
            reason: rule.reason.to_string(),
        }
    }
}

pub fn command_policy_rules() -> Vec<CommandPolicyRule> {
    RULE_SPECS.iter().copied().map(Into::into).collect()
}

pub fn command_policy_text() -> String {
    let mut output = String::new();
    output.push_str("Command policy:\n\n");
    output.push_str("RepoDesk must classify and judge commands before an agent executes them.\n");
    output.push_str(
        "This policy gate is not an OS/process sandbox and does not contain a child process.\n",
    );
    output.push_str("Patch agents must not receive unrestricted shell access.\n\n");
    output.push_str("Rules:\n");

    for rule in RULE_SPECS {
        output.push_str(&format!(
            "  - {} [{}]: {} — {}\n",
            rule.name, rule.verdict, rule.pattern, rule.reason
        ));
    }

    output
}

pub fn evaluate_command(command: &str) -> CommandPolicyPlan {
    let normalized = command.trim().to_lowercase();
    let mut matched_rules = Vec::new();

    let tokens = match shlex::split(command) {
        Some(tokens) => tokens,
        None => {
            return CommandPolicyPlan {
                command: command.to_string(),
                verdict: "block".to_string(),
                reason: "Command is malformed (unclosed quotes).".to_string(),
                required_confirmation: true,
                matched_rules: vec![CommandPolicyRule {
                    name: "malformed".to_string(),
                    verdict: "block".to_string(),
                    pattern: String::new(),
                    reason: "Parser error".to_string(),
                }],
            };
        }
    };

    if has_dangerous_delete(&tokens) {
        matched_rules.push(find_rule("dangerous_delete"));
    }
    if has_privileged_shell(&tokens) {
        matched_rules.push(find_rule("privileged_shell"));
    }
    if has_remote_shell_pipe(&tokens) {
        matched_rules.push(find_rule("remote_shell_pipe"));
    }
    if has_secret_access(&tokens) {
        matched_rules.push(find_rule("secret_access"));
    }
    if has_publish_or_force_push(&tokens) {
        matched_rules.push(find_rule("publish_or_force_push"));
    }
    if is_safe_check(&tokens, &normalized) {
        matched_rules.push(find_rule("safe_checks"));
    }

    let has_block = matched_rules.iter().any(|rule| rule.verdict == "block");
    let has_warn = matched_rules.iter().any(|rule| rule.verdict == "warn");
    let has_allow = matched_rules.iter().any(|rule| rule.verdict == "allow");

    let (verdict, reason, required_confirmation) = if has_block {
        (
            "block".to_string(),
            "Command matches a blocked command-policy rule.".to_string(),
            true,
        )
    } else if has_warn {
        (
            "warn".to_string(),
            "Command may be valid, but requires explicit user confirmation.".to_string(),
            true,
        )
    } else if has_allow {
        (
            "allow".to_string(),
            "Command matches safe verification/read-only patterns.".to_string(),
            false,
        )
    } else {
        (
            "warn".to_string(),
            "Unknown command. Keep it manual or review before giving it to an agent.".to_string(),
            true,
        )
    };

    CommandPolicyPlan {
        command: command.to_string(),
        verdict,
        reason,
        required_confirmation,
        matched_rules,
    }
}

pub fn format_command_policy_plan(plan: &CommandPolicyPlan) -> String {
    let mut output = String::new();
    output.push_str("Command policy plan:\n\n");
    output.push_str(&format!("command: {}\n", plan.command));
    output.push_str(&format!("verdict: {}\n", plan.verdict));
    output.push_str(&format!(
        "requires confirmation: {}\n",
        plan.required_confirmation
    ));
    output.push_str(&format!("reason: {}\n", plan.reason));
    output.push_str("matched rules:\n");

    if plan.matched_rules.is_empty() {
        output.push_str("  - none\n");
    } else {
        for rule in &plan.matched_rules {
            output.push_str(&format!(
                "  - {} [{}]: {}\n",
                rule.name, rule.verdict, rule.reason
            ));
        }
    }

    output
}

fn find_rule(name: &str) -> CommandPolicyRule {
    RULE_SPECS
        .iter()
        .copied()
        .find(|rule| rule.name == name)
        .map(Into::into)
        .expect("command policy rule must exist")
}

fn has_dangerous_delete(tokens: &[String]) -> bool {
    let mut is_rm = false;
    let mut has_rf = false;
    let mut target_root = false;

    for token in tokens {
        if token == "rm" || token == "find" {
            is_rm = true;
        }
        if token == "-rf" || token == "-r" || token == "-R" || token == "-delete" || token == "-fr"
        {
            has_rf = true;
        }
        if token == "/"
            || token == "."
            || token == "*"
            || token == "/*"
            || token == ".*"
            || token == ".."
        {
            target_root = true;
        }
    }

    is_rm && has_rf && target_root
}

fn has_privileged_shell(tokens: &[String]) -> bool {
    if let Some(first) = tokens.first()
        && (first == "sudo" || first == "su")
    {
        return true;
    }

    if let Some(pos) = tokens.iter().position(|token| token == "chmod")
        && pos + 1 < tokens.len()
        && tokens[pos + 1] == "777"
    {
        return true;
    }

    false
}

fn has_remote_shell_pipe(tokens: &[String]) -> bool {
    let mut has_download = false;
    let mut has_pipe = false;
    let mut has_shell = false;

    for token in tokens {
        if token == "curl" || token == "wget" {
            has_download = true;
        }
        if token == "|" || token == "<" {
            has_pipe = true;
        }
        if token == "sh" || token == "bash" || token == "zsh" {
            has_shell = true;
        }
    }

    has_download && has_pipe && has_shell
}

fn has_secret_access(tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        let lower = token.to_ascii_lowercase();
        lower.starts_with("api_key=")
            || lower.starts_with("token=")
            || lower.starts_with("secret=")
            || lower.starts_with("credentials=")
            || crate::security::is_blocked_path(token).is_some()
    })
}

fn has_publish_or_force_push(tokens: &[String]) -> bool {
    if tokens.len() >= 2 {
        if tokens[0] == "git"
            && tokens[1] == "push"
            && tokens
                .iter()
                .any(|token| token == "--force" || token == "-f")
        {
            return true;
        }
        if (tokens[0] == "npm"
            || tokens[0] == "cargo"
            || tokens[0] == "pnpm"
            || tokens[0] == "yarn")
            && tokens[1] == "publish"
        {
            return true;
        }
    }
    false
}

fn is_safe_check(tokens: &[String], _command: &str) -> bool {
    if tokens.is_empty() {
        return false;
    }

    if tokens[0] == "cargo"
        && tokens.len() >= 2
        && (tokens[1] == "check"
            || tokens[1] == "fmt"
            || tokens[1] == "test"
            || tokens[1] == "clippy")
    {
        return true;
    }

    if tokens[0] == "pnpm" || tokens[0] == "npm" || tokens[0] == "yarn" {
        if tokens.len() >= 2
            && (tokens[1] == "test" || tokens[1] == "build" || tokens[1] == "typecheck")
        {
            return true;
        }
        if tokens.len() >= 3
            && tokens[1] == "run"
            && (tokens[2] == "test" || tokens[2] == "build" || tokens[2] == "typecheck")
        {
            return true;
        }
    }

    if tokens[0] == "git"
        && tokens.len() >= 2
        && (tokens[1] == "status"
            || tokens[1] == "diff"
            || tokens[1] == "log"
            || tokens[1] == "branch")
    {
        return true;
    }

    tokens[0] == "repodesk"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_and_unknown_commands_fail_closed() {
        let malformed = evaluate_command("cargo test '");
        assert_eq!(malformed.verdict, "block");
        assert!(malformed.required_confirmation);

        let unknown = evaluate_command("some-new-tool --do-work");
        assert_eq!(unknown.verdict, "warn");
        assert!(unknown.required_confirmation);
    }

    #[test]
    fn dangerous_delete_flags_bare_glob_star_target() {
        let tokens = |command: &str| {
            command
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert!(has_dangerous_delete(&tokens("rm -rf *")));
        assert!(has_dangerous_delete(&tokens("rm -rf .")));
        assert!(has_dangerous_delete(&tokens("rm -rf /")));
        assert!(!has_dangerous_delete(&tokens("rm -rf build")));
    }

    #[test]
    fn secret_access_allows_normal_files() {
        let safe_tokens = vec![
            "src/token_ledger.rs",
            "src/tokens.ts",
            "src/privateRoute.tsx",
            "src/credentials_form.tsx",
            "my_secret_sauce.rs",
            "tokenized_input",
        ];

        for token in safe_tokens {
            assert!(
                !has_secret_access(&[token.to_string()]),
                "Should not block normal file: {token}"
            );
        }
    }

    #[test]
    fn secret_access_blocks_secrets() {
        let secret_tokens = vec![
            ".env",
            ".env.local",
            "./.env",
            "../.env",
            "config/.env",
            "private.key",
            "id_rsa",
            "secret.pem",
            "credentials.json",
            "token=abc",
            "api_key=abc",
            "secret=abc",
            "credentials=abc",
            "config/id_rsa",
            "config/credentials.json",
            "~/.ssh/config",
        ];

        for token in secret_tokens {
            assert!(
                has_secret_access(&[token.to_string()]),
                "Should block secret file: {token}"
            );
        }
    }

    #[test]
    fn policy_text_does_not_claim_os_isolation() {
        let text = command_policy_text();
        assert!(text.contains("not an OS/process sandbox"));
    }
}
