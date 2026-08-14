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
        // Rates are USD **per 1K tokens** for each provider's RepoDesk default
        // model, from public list pricing (≈ mid-2026). They are deliberate
        // estimates, not a billing source of truth — actual spend depends on the
        // exact model and your plan, so override per provider in `costs.toml`.
        Self {
            currency_label: "USD".to_string(),
            rates: vec![
                AgentRate {
                    agent: "ollama".to_string(),
                    model: "local".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Local model. No API billing. Still costs time, RAM, CPU/GPU and electricity.".to_string(),
                },
                AgentRate {
                    agent: "openai_api".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    input_cost_per_1k_units: 0.00015,
                    output_cost_per_1k_units: 0.0006,
                    notes: "USD/1K for gpt-4o-mini list pricing. Override for larger OpenAI models.".to_string(),
                },
                AgentRate {
                    agent: "anthropic_api".to_string(),
                    model: "claude-sonnet-4-6".to_string(),
                    input_cost_per_1k_units: 0.003,
                    output_cost_per_1k_units: 0.015,
                    notes: "USD/1K for Claude Sonnet list pricing. Override for Haiku/Opus tiers.".to_string(),
                },
                AgentRate {
                    agent: "gemini_api".to_string(),
                    model: "gemini-2.5-flash".to_string(),
                    input_cost_per_1k_units: 0.000075,
                    output_cost_per_1k_units: 0.0003,
                    notes: "USD/1K for Gemini Flash list pricing. Override for Pro tiers.".to_string(),
                },
                AgentRate {
                    agent: "codex_cli".to_string(),
                    model: "subscription".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Coding-agent CLI billed by your CLI plan, not per token. Set a nominal rate to register patch-run spend.".to_string(),
                },
                AgentRate {
                    agent: "claude_code_cli".to_string(),
                    model: "subscription".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Coding-agent CLI billed by your CLI plan, not per token. Set a nominal rate to register patch-run spend.".to_string(),
                },
                AgentRate {
                    agent: "chatgpt".to_string(),
                    model: "legacy".to_string(),
                    input_cost_per_1k_units: 0.00015,
                    output_cost_per_1k_units: 0.0006,
                    notes: "Legacy manual ChatGPT route retained for historical ledger entries.".to_string(),
                },
                AgentRate {
                    agent: "codex".to_string(),
                    model: "legacy".to_string(),
                    input_cost_per_1k_units: 0.0,
                    output_cost_per_1k_units: 0.0,
                    notes: "Legacy Codex route retained for historical ledger entries.".to_string(),
                },
                AgentRate {
                    agent: "gemini".to_string(),
                    model: "legacy".to_string(),
                    input_cost_per_1k_units: 0.000075,
                    output_cost_per_1k_units: 0.0003,
                    notes: "Legacy Gemini route retained for historical ledger entries.".to_string(),
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

/// Persist an edited cost config to `costs.toml`.
pub fn save_cost_config(config: &CostConfig) -> RepoDeskResult<()> {
    use crate::utils::ConfigStore;
    config.save_config()
}

/// Reset `costs.toml` to the built-in default rate card and return it.
pub fn reset_cost_config() -> RepoDeskResult<CostConfig> {
    let config = CostConfig::default();
    save_cost_config(&config)?;
    Ok(config)
}

fn conservative_fallback(agent: &str, model: &str) -> AgentRate {
    AgentRate {
        agent: agent.to_ascii_lowercase(),
        model: if model.trim().is_empty() {
            "unknown".to_string()
        } else {
            model.to_string()
        },
        input_cost_per_1k_units: 1.0,
        output_cost_per_1k_units: 3.0,
        notes: "No exact model rate is configured. Using conservative placeholder cost units; add this provider/model to costs.toml for meaningful accounting."
            .to_string(),
    }
}

fn estimate_with_rate(
    config: &CostConfig,
    rate: &AgentRate,
    input_tokens: usize,
    output_tokens: usize,
) -> CostEstimate {
    let input_cost = (input_tokens as f64 / 1000.0) * rate.input_cost_per_1k_units;
    let output_cost = (output_tokens as f64 / 1000.0) * rate.output_cost_per_1k_units;

    CostEstimate {
        agent: rate.agent.clone(),
        model: rate.model.clone(),
        input_tokens,
        output_tokens,
        total_tokens: input_tokens.saturating_add(output_tokens),
        estimated_cost_units: input_cost + output_cost,
        currency_label: config.currency_label.clone(),
        note: rate.notes.clone(),
    }
}

/// Estimate a recorded provider/model pair. Exact model identity is part of the
/// accounting key: a cheaper model must never silently inherit another model's
/// rate merely because both are served by the same provider.
///
/// A user may configure `model = "*"` as an explicit provider-wide fallback.
/// Otherwise an unknown model receives a conservative placeholder estimate and
/// an explanatory note rather than a misleading default-model price.
pub fn estimate_model_cost(
    config: &CostConfig,
    agent: &str,
    model: &str,
    input_tokens: usize,
    output_tokens: usize,
) -> CostEstimate {
    let normalized_agent = agent.trim().to_ascii_lowercase();
    let normalized_model = model.trim();

    let exact = config.rates.iter().find(|rate| {
        rate.agent.eq_ignore_ascii_case(&normalized_agent)
            && rate.model.eq_ignore_ascii_case(normalized_model)
    });
    let wildcard = config.rates.iter().find(|rate| {
        rate.agent.eq_ignore_ascii_case(&normalized_agent) && rate.model.trim() == "*"
    });

    if let Some(rate) = exact.or(wildcard) {
        return estimate_with_rate(config, rate, input_tokens, output_tokens);
    }

    let fallback = conservative_fallback(&normalized_agent, normalized_model);
    estimate_with_rate(config, &fallback, input_tokens, output_tokens)
}

/// Planning helper for callers that know a provider but have not selected a
/// concrete model yet. It intentionally uses the provider's first configured
/// rate. Recorded usage must call [`estimate_model_cost`] instead.
pub fn estimate_agent_cost(
    config: &CostConfig,
    agent: &str,
    input_tokens: usize,
    output_tokens: usize,
) -> CostEstimate {
    let normalized = agent.trim().to_ascii_lowercase();
    if let Some(rate) = config
        .rates
        .iter()
        .find(|rate| rate.agent.eq_ignore_ascii_case(&normalized))
    {
        return estimate_with_rate(config, rate, input_tokens, output_tokens);
    }

    let fallback = conservative_fallback(&normalized, "unknown");
    estimate_with_rate(config, &fallback, input_tokens, output_tokens)
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
    output.push_str("By provider/model:\n");

    for item in &report.by_model {
        let estimate = estimate_model_cost(
            config,
            &item.agent,
            &item.model,
            item.input_tokens,
            item.output_tokens,
        );
        total_cost += estimate.estimated_cost_units;
        output.push_str(&format!(
            "  - {}/{}: tokens={}, estimated_cost={:.4} {}\n",
            item.agent,
            item.model,
            item.total_tokens,
            estimate.estimated_cost_units,
            estimate.currency_label
        ));
    }

    output.push_str(&format!(
        "\nEstimated total: {:.4} {}\n",
        total_cost, config.currency_label
    ));
    output.push_str("\nThis is a planning estimate. Real billing depends on the provider/model.\n");

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_rate(agent: &str, model: &str, input: f64, output: f64) -> AgentRate {
        AgentRate {
            agent: agent.to_string(),
            model: model.to_string(),
            input_cost_per_1k_units: input,
            output_cost_per_1k_units: output,
            notes: format!("fixture {agent}/{model}"),
        }
    }

    #[test]
    fn recorded_model_selects_exact_rate_for_same_provider() {
        let config = CostConfig {
            currency_label: "USD".to_string(),
            rates: vec![
                model_rate("provider", "cheap", 1.0, 2.0),
                model_rate("provider", "expensive", 10.0, 20.0),
            ],
        };

        let cheap = estimate_model_cost(&config, "provider", "cheap", 1_000, 1_000);
        let expensive = estimate_model_cost(&config, "provider", "expensive", 1_000, 1_000);

        assert_eq!(cheap.model, "cheap");
        assert_eq!(cheap.estimated_cost_units, 3.0);
        assert_eq!(expensive.model, "expensive");
        assert_eq!(expensive.estimated_cost_units, 30.0);
    }

    #[test]
    fn unknown_recorded_model_does_not_inherit_default_model_rate() {
        let config = CostConfig {
            currency_label: "USD".to_string(),
            rates: vec![model_rate("provider", "default", 0.1, 0.2)],
        };

        let estimate = estimate_model_cost(&config, "provider", "other", 1_000, 1_000);

        assert_eq!(estimate.model, "other");
        assert_eq!(estimate.estimated_cost_units, 4.0);
        assert!(estimate.note.contains("No exact model rate"));
    }

    #[test]
    fn explicit_wildcard_rate_can_cover_provider_models() {
        let config = CostConfig {
            currency_label: "USD".to_string(),
            rates: vec![model_rate("provider", "*", 2.0, 5.0)],
        };

        let estimate = estimate_model_cost(&config, "provider", "new-model", 1_000, 1_000);

        assert_eq!(estimate.estimated_cost_units, 7.0);
        assert_eq!(estimate.model, "*");
    }
}
