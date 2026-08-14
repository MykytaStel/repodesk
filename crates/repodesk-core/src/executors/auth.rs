use std::env;
use std::path::PathBuf;

use serde::Deserialize;

use super::process::run_probe_command;
use super::{ExecutorAuthStatus, first_meaningful_line};

pub(super) struct AuthProbeResult {
    pub(super) authenticated: Option<bool>,
    pub(super) status: ExecutorAuthStatus,
    pub(super) source: Option<String>,
    pub(super) detail: Option<String>,
    pub(super) notes: Vec<String>,
}

impl AuthProbeResult {
    fn authenticated(source: &str, detail: Option<String>, note: String) -> Self {
        Self {
            authenticated: Some(true),
            status: ExecutorAuthStatus::Authenticated,
            source: Some(source.to_string()),
            detail,
            notes: vec![note],
        }
    }

    fn unauthenticated(source: &str, detail: Option<String>, note: String) -> Self {
        Self {
            authenticated: Some(false),
            status: ExecutorAuthStatus::Unauthenticated,
            source: Some(source.to_string()),
            detail,
            notes: vec![note],
        }
    }

    fn unknown(note: String) -> Self {
        Self {
            authenticated: None,
            status: ExecutorAuthStatus::Unknown,
            source: None,
            detail: None,
            notes: vec![note],
        }
    }

    fn is_unknown(&self) -> bool {
        self.status == ExecutorAuthStatus::Unknown
    }
}

/// Known local authentication artifacts for a coding-agent CLI, relative to the
/// user's home directory. Existence is checked, never contents.
fn auth_artifacts(executor_id: &str) -> &'static [&'static str] {
    match executor_id {
        "codex_cli" => &[".codex/auth.json"],
        "claude_code_cli" => &[
            ".claude/.credentials.json",
            ".config/claude/.credentials.json",
        ],
        _ => &[],
    }
}

/// Prefer documented, side-effect-free CLI auth status commands. Fall back to a
/// local artifact existence check only when the status command is unavailable or
/// inconclusive. Credential files are never opened or parsed.
pub(super) fn detect_authentication(
    executor_id: &str,
    binary: &str,
    home: Option<PathBuf>,
) -> AuthProbeResult {
    let cli_probe = match executor_id {
        "codex_cli" => probe_codex_auth(binary),
        "claude_code_cli" => probe_claude_auth(binary),
        _ => AuthProbeResult::unknown(format!(
            "No auth probe is registered for executor {executor_id}."
        )),
    };
    if !cli_probe.is_unknown() {
        return cli_probe;
    }

    let mut artifact_probe = detect_auth_artifact(executor_id, home);
    if artifact_probe.is_unknown() {
        artifact_probe.notes.extend(cli_probe.notes);
    }
    artifact_probe
}

fn detect_auth_artifact(executor_id: &str, home: Option<PathBuf>) -> AuthProbeResult {
    let Some(home) = home else {
        return AuthProbeResult::unknown(
            "No documented CLI auth status or known local auth artifact found.".to_string(),
        );
    };
    let found = auth_artifacts(executor_id)
        .iter()
        .any(|relative| home.join(relative).exists());
    if found {
        AuthProbeResult::authenticated(
            "local auth artifact",
            Some("known local auth artifact exists; contents not read".to_string()),
            "Local auth artifact found (existence only; contents not read).".to_string(),
        )
    } else {
        AuthProbeResult::unknown(
            "No documented CLI auth status or known local auth artifact found.".to_string(),
        )
    }
}

fn probe_codex_auth(binary: &str) -> AuthProbeResult {
    const SOURCE: &str = "codex login status";
    match run_probe_command(binary, &["login", "status"]) {
        Some(output) => parse_codex_auth_output(SOURCE, output.status_success, &output.combined()),
        None => {
            AuthProbeResult::unknown("Codex auth status probe could not be started.".to_string())
        }
    }
}

fn parse_codex_auth_output(source: &str, status_success: bool, output: &str) -> AuthProbeResult {
    let detail = safe_status_detail(output);
    let lower = output.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not authenticated")
        || lower.contains("logged out")
    {
        return AuthProbeResult::unauthenticated(
            source,
            detail,
            "Codex auth status reports not logged in.".to_string(),
        );
    }
    if status_success && lower.contains("logged in") {
        return AuthProbeResult::authenticated(
            source,
            detail,
            "Codex auth status reports logged in.".to_string(),
        );
    }
    AuthProbeResult::unknown("Codex auth status output was inconclusive.".to_string())
}

fn probe_claude_auth(binary: &str) -> AuthProbeResult {
    const SOURCE: &str = "claude auth status --json";
    match run_probe_command(binary, &["auth", "status", "--json"]) {
        Some(output) => {
            let text = if output.stdout.trim_start().starts_with('{') {
                output.stdout.clone()
            } else {
                output.combined()
            };
            parse_claude_auth_output(SOURCE, output.status_success, &text)
        }
        None => {
            AuthProbeResult::unknown("Claude auth status probe could not be started.".to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClaudeAuthStatusOutput {
    #[serde(rename = "loggedIn")]
    logged_in: Option<bool>,
    #[serde(rename = "authMethod")]
    auth_method: Option<String>,
    #[serde(rename = "apiProvider")]
    api_provider: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

fn parse_claude_auth_output(source: &str, status_success: bool, output: &str) -> AuthProbeResult {
    if let Ok(parsed) = serde_json::from_str::<ClaudeAuthStatusOutput>(output) {
        let detail = claude_auth_detail(&parsed);
        return match parsed.logged_in {
            Some(true) => AuthProbeResult::authenticated(
                source,
                detail,
                "Claude auth status reports logged in.".to_string(),
            ),
            Some(false) => AuthProbeResult::unauthenticated(
                source,
                detail,
                "Claude auth status reports not logged in.".to_string(),
            ),
            None => AuthProbeResult::unknown(
                "Claude auth status JSON did not include loggedIn.".to_string(),
            ),
        };
    }

    let lower = output.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not authenticated")
        || lower.contains("logged out")
    {
        return AuthProbeResult::unauthenticated(
            source,
            safe_status_detail(output),
            "Claude auth status reports not logged in.".to_string(),
        );
    }
    if status_success && (lower.contains("logged in") || lower.contains("authenticated")) {
        return AuthProbeResult::authenticated(
            source,
            safe_status_detail(output),
            "Claude auth status reports logged in.".to_string(),
        );
    }
    AuthProbeResult::unknown("Claude auth status output was inconclusive.".to_string())
}

fn claude_auth_detail(parsed: &ClaudeAuthStatusOutput) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(method) = non_secret_label(parsed.auth_method.as_deref()) {
        parts.push(format!("method {method}"));
    }
    if let Some(provider) = non_secret_label(parsed.api_provider.as_deref()) {
        parts.push(format!("provider {provider}"));
    }
    if let Some(subscription) = non_secret_label(parsed.subscription_type.as_deref()) {
        parts.push(format!("subscription {subscription}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

fn non_secret_label(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let safe = !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'));
    safe.then(|| value.to_string())
}

fn safe_status_detail(output: &str) -> Option<String> {
    first_meaningful_line(output).and_then(|line| {
        let (redacted, _) = crate::security::redact_secrets(&line);
        if looks_like_personal_identifier(&redacted) {
            None
        } else {
            Some(redacted.chars().take(160).collect())
        }
    })
}

fn looks_like_personal_identifier(value: &str) -> bool {
    value.contains('@')
        || value.to_ascii_lowercase().contains("email")
        || value.to_ascii_lowercase().contains("orgid")
}

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fake_binary(body: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let script = dir.path().join("fake-agent");
        std::fs::write(&script, body).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (dir, script.display().to_string())
    }

    #[test]
    fn authentication_falls_back_to_artifact_existence() {
        let home = tempfile::TempDir::new().unwrap();
        assert_eq!(
            detect_authentication(
                "codex_cli",
                "missing-codex-binary",
                Some(home.path().to_path_buf())
            )
            .authenticated,
            None
        );

        std::fs::create_dir_all(home.path().join(".codex")).unwrap();
        std::fs::write(home.path().join(".codex/auth.json"), "{}").unwrap();
        let probe = detect_authentication(
            "codex_cli",
            "missing-codex-binary",
            Some(home.path().to_path_buf()),
        );
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(probe.source.as_deref(), Some("local auth artifact"));

        assert_eq!(
            detect_authentication(
                "claude_code_cli",
                "missing-claude-binary",
                Some(home.path().to_path_buf())
            )
            .authenticated,
            None
        );
        assert_eq!(
            detect_authentication("codex_cli", "missing-codex-binary", None).authenticated,
            None
        );
    }

    #[test]
    fn codex_parser_reports_true_and_false() {
        let logged_in =
            parse_codex_auth_output("codex login status", true, "Logged in using ChatGPT");
        assert_eq!(logged_in.authenticated, Some(true));
        assert_eq!(logged_in.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(logged_in.detail.as_deref(), Some("Logged in using ChatGPT"));

        let logged_out = parse_codex_auth_output("codex login status", false, "Not logged in");
        assert_eq!(logged_out.authenticated, Some(false));
        assert_eq!(logged_out.status, ExecutorAuthStatus::Unauthenticated);
    }

    #[test]
    fn claude_parser_redacts_account_identifiers() {
        let raw = r#"{
          "loggedIn": true,
          "authMethod": "claude.ai",
          "apiProvider": "firstParty",
          "email": "person@example.com",
          "orgId": "abc-123",
          "subscriptionType": "pro"
        }"#;
        let probe = parse_claude_auth_output("claude auth status --json", true, raw);
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        let detail = probe.detail.unwrap();
        assert!(detail.contains("method claude.ai"));
        assert!(detail.contains("provider firstParty"));
        assert!(detail.contains("subscription pro"));
        assert!(!detail.contains("person@example.com"));
        assert!(!detail.contains("abc-123"));
    }

    #[cfg(unix)]
    #[test]
    fn authentication_uses_codex_status_command() {
        let (_dir, binary) = fake_binary(
            "#!/bin/sh\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then echo 'Logged in using ChatGPT'; exit 0; fi\nexit 2\n",
        );

        let probe = detect_authentication("codex_cli", &binary, None);
        assert_eq!(probe.authenticated, Some(true));
        assert_eq!(probe.status, ExecutorAuthStatus::Authenticated);
        assert_eq!(probe.source.as_deref(), Some("codex login status"));
    }

    #[cfg(unix)]
    #[test]
    fn authentication_uses_claude_status_command() {
        let (_dir, binary) = fake_binary(
            "#!/bin/sh\nif [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ] && [ \"$3\" = \"--json\" ]; then printf '{\"loggedIn\":false}\\n'; exit 0; fi\nexit 2\n",
        );

        let probe = detect_authentication("claude_code_cli", &binary, None);
        assert_eq!(probe.authenticated, Some(false));
        assert_eq!(probe.status, ExecutorAuthStatus::Unauthenticated);
        assert_eq!(probe.source.as_deref(), Some("claude auth status --json"));
    }
}
