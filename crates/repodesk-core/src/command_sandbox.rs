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

    let tokens = match shlex::split(command) {
        Some(t) => t,
        None => {
            return SandboxPlan {
                command: command.to_string(),
                verdict: "block".to_string(),
                reason: "Command is malformed (unclosed quotes).".to_string(),
                required_confirmation: true,
                matched_rules: vec![SandboxRule {
                    name: "malformed".to_string(),
                    verdict: "block".to_string(),
                    pattern: "".to_string(),
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
        if token == "/" || token == "." || token == "/*" || token == ".*" || token == ".." {
            target_root = true;
        }
    }

    is_rm && has_rf && target_root
}

fn has_privileged_shell(tokens: &[String]) -> bool {
    if let Some(first) = tokens.first() {
        if first == "sudo" || first == "su" {
            return true;
        }
    }

    // Check for chmod 777
    if let Some(pos) = tokens.iter().position(|t| t == "chmod") {
        if pos + 1 < tokens.len() && tokens[pos + 1] == "777" {
            return true;
        }
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
    for token in tokens {
        let lower = token.to_lowercase();
        
        if lower == ".env" || lower.starts_with(".env.") {
            return true;
        }
        
        if lower.ends_with(".pem") || lower.ends_with(".key") {
            return true;
        }
        
        if lower.starts_with("api_key=") || lower.starts_with("token=") || lower.starts_with("secret=") || lower.starts_with("credentials=") {
            return true;
        }

        if lower == "api_key" || lower == "token" || lower == "secret" || lower == "credentials" {
            return true;
        }
    }
    false
}

fn has_publish_or_force_push(tokens: &[String]) -> bool {
    if tokens.len() >= 2 {
        if tokens[0] == "git"
            && tokens[1] == "push"
            && tokens.iter().any(|t| t == "--force" || t == "-f")
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

    if tokens[0] == "repodesk" {
        return true;
    }

    false
}
