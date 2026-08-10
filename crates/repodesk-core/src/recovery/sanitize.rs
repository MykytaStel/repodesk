use std::sync::LazyLock;

use regex::Regex;

const MAX_RECOVERY_TEXT_CHARS: usize = 2_000;

static SENSITIVE_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)\b(token|authorization|password|secret|api[-_]?key)\b\s*[:=]\s*[^\r\n,;]+")
        .expect("recovery sensitive-value regex must compile")
});

static HOME_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:/Users/[^/\s]+|/home/[^/\s]+|[A-Z]:\\Users\\[^\\\s]+)")
        .expect("recovery home-path regex must compile")
});

pub fn sanitize_recovery_text(value: &str) -> String {
    let without_secrets = SENSITIVE_ASSIGNMENT.replace_all(value, "$1=[redacted]");
    let without_home = HOME_PATH.replace_all(&without_secrets, "~");
    let mut characters = without_home.chars();
    let bounded = characters
        .by_ref()
        .take(MAX_RECOVERY_TEXT_CHARS.saturating_sub(1))
        .collect::<String>();
    if characters.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}
