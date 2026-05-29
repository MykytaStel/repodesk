use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::usage::token_ledger::TokenReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    pub currency_label: String,
    pub rates: Vec<AgentRate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRate {
    pub agent: String,
    pub model: String,
    pub input_cost_per_1k_units: f64,
    pub output_cost_per_1k_units: f64,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub agent: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub estimated_cost_units: f64,
    pub currency_label: String,
    pub note: String,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            currency_label: "cost_units".to_string(),
            rates: vec![
                AgentRate {
                    agent: "ollama".to_string(),
                    model: "local".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Local model. No API billing. Still costs time, RAM, CPU/GPU and electricity.".to_string(),
                },
                AgentRate {
                    agent: "chatgpt".to_string(),
                    model: "user-configured".to_string(),
                    input_cost_per_1k_units: 1.0,
                    output_cost_per_1k_units: 3.0,
                    notes: "Placeholder rate. Replace with your real plan/model economics if needed.".to_string(),
                },
                AgentRate {
                    agent: "codex".to_string(),
                    model: "user-configured".to_string(),
                    input_cost_per_1k_units: 1.2,
                    output_cost_per_1k_units: 4.0,
                    notes: "Placeholder rate for patch work. Update locally when you know real costs.".to_string(),
                },
                AgentRate {
                    agent: "gemini".to_string(),
                    model: "user-configured".to_string(),
                    input_cost_per_1k_units: 0.8,
                    output_cost_per_1k_units: 2.5,
                    notes: "Placeholder rate for second-opinion review.".to_string(),
                },
                AgentRate {
                    agent: "hermes".to_string(),
                    model: "local-experimental".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Local sandbox agent. No API billing; use with strict permissions.".to_string(),
                },
            ],
        }
    }
}

impl crate::utils::ConfigStore for CostConfig {
    const FILE_NAME: &'static str = "costs.toml";
}

pub fn ensure_cost_config() -> RepoDeskResult<CostConfig> {
    use crate::utils::ConfigStore;
    CostConfig::ensure_config()
}

pub fn load_cost_config() -> RepoDeskResult<CostConfig> {
    use crate::utils::ConfigStore;
    CostConfig::load_config()
}

pub fn estimate_agent_cost(
    config: &CostConfig,
    agent: &str,
    input_tokens: usize,
    output_tokens: usize,
) -> CostEstimate {
    let normalized = agent.to_ascii_lowercase();
    let rate = config
        .rates
        .iter()
        .find(|rate| rate.agent.eq_ignore_ascii_case(&normalized));

    let fallback;
    let rate = match rate {
        Some(rate) => rate,
        None => {
            fallback = AgentRate {
                agent: normalized.clone(),
                model: "unknown".to_string(),
                input_cost_per_1k_units: 1.0,
                output_cost_per_1k_units: 3.0,
                notes: "Unknown agent. Using conservative placeholder cost units.".to_string(),
            };
            &fallback
        }
    };

    let input_cost = (input_tokens as f64 / 1000.0) * rate.input_cost_per_1k_units;
    let output_cost = (output_tokens as f64 / 1000.0) * rate.output_cost_per_1k_units;

    CostEstimate {
        agent: rate.agent.clone(),
        model: rate.model.clone(),
        input_tokens,
        output_tokens,
        total_tokens: input_tokens + output_tokens,
        estimated_cost_units: input_cost + output_cost,
        currency_label: config.currency_label.clone(),
        note: rate.notes.clone(),
    }
}

pub fn format_cost_config(config: &CostConfig) -> String {
    let mut output = String::new();

    output.push_str("Cost config:\n\n");
    output.push_str(&format!("Currency label: {}\n\n", config.currency_label));

    for rate in &config.rates {
        output.push_str(&format!("- {} / {}\n", rate.agent, rate.model));
        output.push_str(&format!(
            "  input per 1k:  {:.4}\n",
            rate.input_cost_per_1k_units
        ));
        output.push_str(&format!(
            "  output per 1k: {:.4}\n",
            rate.output_cost_per_1k_units
        ));
        output.push_str(&format!("  notes: {}\n\n", rate.notes));
    }

    output.push_str("These are local planning units, not authoritative vendor prices.\n");
    output.push_str("Edit ~/.repodesk/config/costs.toml when you want real rates.\n");

    output
}

pub fn format_cost_estimate(estimate: &CostEstimate) -> String {
    format!(
        r#"Cost estimate:

Agent: {}
Model: {}
Input tokens: {}
Output tokens: {}
Total tokens: {}
Estimated cost: {:.4} {}

Note: {}
"#,
        estimate.agent,
        estimate.model,
        estimate.input_tokens,
        estimate.output_tokens,
        estimate.total_tokens,
        estimate.estimated_cost_units,
        estimate.currency_label,
        estimate.note
    )
}

pub fn format_cost_report(config: &CostConfig, report: &TokenReport) -> String {
    if report.entries_count == 0 {
        return "No token ledger entries yet.\n".to_string();
    }

    let mut output = String::new();
    let mut total_cost = 0.0;

    output.push_str("Cost report:\n\n");
    output.push_str(&format!("Entries: {}\n", report.entries_count));
    output.push_str(&format!("Total tokens: {}\n\n", report.total_tokens));
    output.push_str("By agent:\n");

    for item in &report.by_agent {
        let estimate =
            estimate_agent_cost(config, &item.agent, item.input_tokens, item.output_tokens);
        total_cost += estimate.estimated_cost_units;
        output.push_str(&format!(
            "  - {}: tokens={}, estimated_cost={:.4} {}\n",
            item.agent, item.total_tokens, estimate.estimated_cost_units, estimate.currency_label
        ));
    }

    output.push_str(&format!(
        "\nEstimated total: {:.4} {}\n",
        total_cost, config.currency_label
    ));
    output.push_str("\nThis is a planning estimate. Real billing depends on the provider/model.\n");

    output
}
