use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub kind: String,
    pub role: String,
    pub default_budget_tokens: usize,
    pub allowed_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub preferred_for: Vec<String>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        default_agents_config()
    }
}

impl crate::utils::ConfigStore for AgentsConfig {
    const FILE_NAME: &'static str = "agents.toml";
}

pub fn ensure_agents_config() -> RepoDeskResult<AgentsConfig> {
    use crate::utils::ConfigStore;
    AgentsConfig::ensure_config()
}

pub fn load_agents_config() -> RepoDeskResult<AgentsConfig> {
    use crate::utils::ConfigStore;
    AgentsConfig::load_config()
}

pub fn recommend_agents(category: &str) -> RepoDeskResult<Vec<AgentConfig>> {
    let config = ensure_agents_config()?;
    let normalized = category.to_ascii_lowercase();

    let mut matches = config
        .agents
        .into_iter()
        .filter(|agent| {
            agent
                .preferred_for
                .iter()
                .any(|item| item.eq_ignore_ascii_case(&normalized))
                || agent.role.to_ascii_lowercase().contains(&normalized)
                || agent.kind.to_ascii_lowercase().contains(&normalized)
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        matches = load_agents_config()?.agents;
    }

    matches.sort_by_key(|a| a.default_budget_tokens);

    Ok(matches)
}

pub fn format_agents(config: &AgentsConfig) -> String {
    let mut output = String::new();

    output.push_str("Agents config:\n\n");

    for agent in &config.agents {
        output.push_str(&format!("- {} ({})\n", agent.name, agent.kind));
        output.push_str(&format!("  role: {}\n", agent.role));
        output.push_str(&format!(
            "  default budget: {} tokens\n",
            agent.default_budget_tokens
        ));
        output.push_str(&format!(
            "  preferred for: {}\n",
            agent.preferred_for.join(", ")
        ));
        output.push_str(&format!(
            "  allowed: {}\n",
            agent.allowed_actions.join(", ")
        ));
        output.push_str(&format!(
            "  forbidden: {}\n\n",
            agent.forbidden_actions.join(", ")
        ));
    }

    output
}

fn default_agents_config() -> AgentsConfig {
    AgentsConfig {
        agents: vec![
            AgentConfig {
                name: "ollama".to_string(),
                kind: "local".to_string(),
                role: "local compression and cheap reasoning".to_string(),
                default_budget_tokens: 30_000,
                allowed_actions: vec![
                    "summarize context".to_string(),
                    "compress logs".to_string(),
                    "draft local notes".to_string(),
                ],
                forbidden_actions: vec![
                    "final architecture decisions".to_string(),
                    "unreviewed production patches".to_string(),
                ],
                preferred_for: vec![
                    "compression".to_string(),
                    "summary".to_string(),
                    "local".to_string(),
                ],
            },
            AgentConfig {
                name: "chatgpt".to_string(),
                kind: "paid".to_string(),
                role: "architecture mentor and reviewer".to_string(),
                default_budget_tokens: 12_000,
                allowed_actions: vec![
                    "architecture review".to_string(),
                    "slice planning".to_string(),
                    "risk analysis".to_string(),
                ],
                forbidden_actions: vec![
                    "full repository ingestion".to_string(),
                    "unbounded refactor request".to_string(),
                ],
                preferred_for: vec![
                    "architecture".to_string(),
                    "review".to_string(),
                    "planning".to_string(),
                ],
            },
            AgentConfig {
                name: "codex".to_string(),
                kind: "paid_patch".to_string(),
                role: "bounded patch executor".to_string(),
                default_budget_tokens: 10_000,
                allowed_actions: vec![
                    "small patch".to_string(),
                    "test update".to_string(),
                    "local refactor".to_string(),
                ],
                forbidden_actions: vec![
                    "broad rewrite".to_string(),
                    "secret access".to_string(),
                    "large unrelated file reads".to_string(),
                ],
                preferred_for: vec![
                    "patch".to_string(),
                    "implementation".to_string(),
                    "tests".to_string(),
                ],
            },
            AgentConfig {
                name: "gemini".to_string(),
                kind: "paid_review".to_string(),
                role: "second opinion reviewer".to_string(),
                default_budget_tokens: 12_000,
                allowed_actions: vec![
                    "independent review".to_string(),
                    "risk scan".to_string(),
                    "alternative plan".to_string(),
                ],
                forbidden_actions: vec![
                    "primary implementation".to_string(),
                    "full repo ingestion by default".to_string(),
                ],
                preferred_for: vec!["review".to_string(), "second-opinion".to_string()],
            },
            AgentConfig {
                name: "hermes".to_string(),
                kind: "local_experimental".to_string(),
                role: "sandbox-only local agent".to_string(),
                default_budget_tokens: 20_000,
                allowed_actions: vec![
                    "read-only experiments".to_string(),
                    "sandbox reasoning".to_string(),
                ],
                forbidden_actions: vec![
                    "production writes".to_string(),
                    "unrestricted shell".to_string(),
                    "secret access".to_string(),
                ],
                preferred_for: vec!["experiment".to_string(), "sandbox".to_string()],
            },
        ],
    }
}
