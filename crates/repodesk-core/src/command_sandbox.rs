use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRule {
    pub name: String,
    pub verdict: String,
    pub pattern: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPlan {
    pub command: String,
    pub verdict: String,
    pub reason: String,
    pub required_confirmation: bool,
    pub matched_rules: Vec<SandboxRule>,
}

pub fn sandbox_rules() -> Vec<SandboxRule> {
    vec![
        SandboxRule {
            name: "dangerous_delete".to_string(),
            verdict: "block".to_string(),
            pattern: "rm -rf /, rm -rf ., find ... -delete".to_string(),
            reason: "Destructive filesystem command.".to_string(),
        },
        SandboxRule {
            name: "privileged_shell".to_string(),
            verdict: "block".to_string(),
            pattern: "sudo, su, chmod 777".to_string(),
            reason: "Privilege escalation or broad permission change.".to_string(),
        },
        SandboxRule {
            name: "remote_shell_pipe".to_string(),
            verdict: "block".to_string(),
            pattern: "curl|sh, wget|sh, bash <(curl ...)".to_string(),
            reason: "Remote code execution without review.".to_string(),
        },
        SandboxRule {
            name: "secret_access".to_string(),
            verdict: "block".to_string(),
            pattern: ".env, *.pem, *.key, credentials, token".to_string(),
            reason: "Potential secret exposure.".to_string(),
        },
        SandboxRule {
            name: "publish_or_force_push".to_string(),
            verdict: "warn".to_string(),
            pattern: "git push --force, npm publish, cargo publish".to_string(),
            reason: "Release or history-changing command requires explicit user intent."
                .to_string(),
        },
        SandboxRule {
            name: "safe_checks".to_string(),
            verdict: "allow".to_string(),
            pattern: "cargo check/fmt/test/clippy, pnpm test/build/typecheck, git status/diff/log"
                .to_string(),
            reason: "Read-only or verification-focused command.".to_string(),
        },
    ]
}

pub fn sandbox_policy() -> String {
    let mut output = String::new();
    output.push_str("Command sandbox policy:\n\n");
    output.push_str("RepoDesk should plan and judge commands before an agent executes them.\n");
    output.push_str("Patch agents must not receive unrestricted shell access.\n\n");
    output.push_str("Rules:\n");

    for rule in sandbox_rules() {
        output.push_str(&format!(
            "  - {} [{}]: {} — {}\n",
            rule.name, rule.verdict, rule.pattern, rule.reason
        ));
    }

    output
}

pub fn plan_command(command: &str) -> SandboxPlan {
    let normalized = command.trim().to_lowercase();
    let mut matched_rules = Vec::new();

    if has_dangerous_delete(&normalized) {
        matched_rules.push(find_rule("dangerous_delete"));
    }

    if contains_any(&normalized, &["sudo ", "su -", "chmod 777"]) {
        matched_rules.push(find_rule("privileged_shell"));
    }

    if has_remote_shell_pipe(&normalized) {
        matched_rules.push(find_rule("remote_shell_pipe"));
    }

    if contains_any(
        &normalized,
        &[
            ".env",
            ".pem",
            ".key",
            "credentials",
            "api_key",
            "secret",
            "token",
        ],
    ) {
        matched_rules.push(find_rule("secret_access"));
    }

    if contains_any(
        &normalized,
        &["git push --force", "npm publish", "cargo publish"],
    ) {
        matched_rules.push(find_rule("publish_or_force_push"));
    }

    if is_safe_check(&normalized) {
        matched_rules.push(find_rule("safe_checks"));
    }

    let has_block = matched_rules.iter().any(|rule| rule.verdict == "block");
    let has_warn = matched_rules.iter().any(|rule| rule.verdict == "warn");
    let has_allow = matched_rules.iter().any(|rule| rule.verdict == "allow");

    let (verdict, reason, required_confirmation) = if has_block {
        (
            "block".to_string(),
            "Command matches a blocked sandbox rule.".to_string(),
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

    SandboxPlan {
        command: command.to_string(),
        verdict,
        reason,
        required_confirmation,
        matched_rules,
    }
}

pub fn format_sandbox_plan(plan: &SandboxPlan) -> String {
    let mut output = String::new();
    output.push_str("Sandbox command plan:\n\n");
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

fn find_rule(name: &str) -> SandboxRule {
    sandbox_rules()
        .into_iter()
        .find(|rule| rule.name == name)
        .expect("sandbox rule must exist")
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn has_dangerous_delete(command: &str) -> bool {
    command.contains("rm -rf /")
        || command.contains("rm -rf .")
        || command.contains("rm -fr /")
        || command.contains("find ") && command.contains(" -delete")
}

fn has_remote_shell_pipe(command: &str) -> bool {
    (command.contains("curl") || command.contains("wget"))
        && (command.contains("| sh")
            || command.contains("| bash")
            || command.contains("bash <")
            || command.contains("sh <"))
}

fn is_safe_check(command: &str) -> bool {
    command == "cargo check"
        || command == "cargo fmt"
        || command == "cargo fmt --check"
        || command == "cargo test"
        || command.starts_with("cargo check ")
        || command.starts_with("cargo fmt ")
        || command.starts_with("cargo test ")
        || command.starts_with("cargo clippy")
        || command == "pnpm test"
        || command == "pnpm build"
        || command == "pnpm typecheck"
        || command.starts_with("pnpm test ")
        || command.starts_with("pnpm build ")
        || command.starts_with("pnpm typecheck ")
        || command == "npm test"
        || command.starts_with("npm run test")
        || command.starts_with("npm run build")
        || command.starts_with("git status")
        || command.starts_with("git diff")
        || command.starts_with("git log")
        || command.starts_with("git branch")
        || command.starts_with("repodesk ")
}
