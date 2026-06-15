use serde::Serialize;

/// Safety gate the UI must pass before a paid/cloud-agent prompt is revealed for
/// the user to copy out of RepoDesk. RepoDesk never sends prompts automatically;
/// this gate re-runs the judge pipeline (guard + safety scan + budget) at the
/// human hand-off so secrets/unsafe context surface before anything leaves.
#[derive(Debug, Clone, Serialize)]
pub struct PaidAgentGate {
    pub agent: String,
    pub is_paid: bool,
    pub is_patch: bool,
    /// `ALLOW` / `WARN` / `BLOCK` from the judge pipeline.
    pub decision: String,
    pub reasons: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Map a prompt artifact kind to the agent it targets, if any.
pub fn agent_for_prompt_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "prompt_codex" => Some("codex"),
        "prompt_chatgpt" => Some("chatgpt"),
        "prompt_review" => Some("gemini"),
        _ => None,
    }
}

#[tauri::command]
pub fn paid_agent_gate(agent: String) -> PaidAgentGate {
    let normalized = agent.trim().to_ascii_lowercase();

    let policy = repodesk_core::security::load_security_policy().ok();
    let is_paid = policy
        .as_ref()
        .map(|policy| policy.paid_agents.iter().any(|item| item == &normalized))
        .unwrap_or(false);
    let is_patch = policy
        .as_ref()
        .map(|policy| policy.patch_agents.iter().any(|item| item == &normalized))
        .unwrap_or(false);

    match repodesk_core::judge::judge_agent(&normalized) {
        Ok(report) => PaidAgentGate {
            agent: normalized,
            is_paid,
            is_patch,
            decision: report.decision.as_label().to_string(),
            reasons: report.reasons,
            recommendations: report.recommendations,
        },
        Err(error) => PaidAgentGate {
            agent: normalized,
            is_paid,
            is_patch,
            decision: "BLOCK".to_string(),
            reasons: vec![format!("Judgement unavailable: {error}")],
            recommendations: vec![
                "Build context and run checks before external AI use.".to_string(),
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_paid_prompt_kinds_to_agents() {
        assert_eq!(agent_for_prompt_kind("prompt_codex"), Some("codex"));
        assert_eq!(agent_for_prompt_kind("prompt_chatgpt"), Some("chatgpt"));
        assert_eq!(agent_for_prompt_kind("prompt_review"), Some("gemini"));
    }

    #[test]
    fn non_prompt_kinds_have_no_agent() {
        assert_eq!(agent_for_prompt_kind("context"), None);
        assert_eq!(agent_for_prompt_kind("checks_summary"), None);
        assert_eq!(agent_for_prompt_kind("token_estimate"), None);
    }
}
