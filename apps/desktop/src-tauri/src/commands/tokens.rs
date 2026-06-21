use super::*;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTotals {
    pub entries_count: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_tokens: usize,
    pub today_total_tokens: usize,
    pub remaining_daily_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageItem {
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub estimated_cost_units: Option<f64>,
    pub currency_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenArtifactEstimate {
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub exists: bool,
    pub size_bytes: u64,
    pub estimated_tokens: Option<usize>,
    pub status: String,
    pub recommendation: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCostSummary {
    pub estimated_total_units: f64,
    pub currency_label: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageSnapshot {
    pub generated_at_ms: u128,
    pub totals: TokenTotals,
    pub by_provider: Vec<TokenUsageItem>,
    pub by_model: Vec<TokenUsageItem>,
    pub active_artifacts: Vec<TokenArtifactEstimate>,
    pub cost_estimate: TokenCostSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTokenUsageInput {
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub category: String,
    pub notes: Option<String>,
}

fn token_artifact_estimate(kind: &str) -> TokenArtifactEstimate {
    match artifact_path(kind) {
        Ok((title, path)) => {
            let metadata = fs::metadata(&path).ok();
            let exists = metadata.is_some();
            let size_bytes = metadata
                .as_ref()
                .map(|value| value.len())
                .unwrap_or_default();

            if !exists {
                return TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: None,
                    status: "missing".into(),
                    recommendation: "Generate this artifact before sending context to an agent."
                        .into(),
                    error: None,
                };
            }

            match repodesk_core::tokens::estimate_file(&path) {
                Ok(estimate) => TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: Some(estimate.estimated_tokens),
                    status: estimate.status.as_label().to_string(),
                    recommendation: estimate.status.recommendation().to_string(),
                    error: None,
                },
                Err(error) => TokenArtifactEstimate {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists,
                    size_bytes,
                    estimated_tokens: None,
                    status: "unreadable".into(),
                    recommendation: "Open the artifact directly or rebuild it.".into(),
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => TokenArtifactEstimate {
            kind: kind.to_string(),
            title: kind.to_string(),
            path: None,
            exists: false,
            size_bytes: 0,
            estimated_tokens: None,
            status: "missing_task".into(),
            recommendation: "Create or select an active task first.".into(),
            error: Some(error),
        },
    }
}

pub(crate) fn build_token_usage_snapshot() -> TokenUsageSnapshot {
    let report = repodesk_core::usage::token_ledger::read_token_report().unwrap_or(
        repodesk_core::usage::token_ledger::TokenReport {
            entries_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            today_tokens: 0,
            by_agent: Vec::new(),
            by_model: Vec::new(),
        },
    );
    let cost_config = repodesk_core::usage::cost::load_cost_config().unwrap_or_default();
    let budget_config = repodesk_core::usage::budget::load_budget_config().unwrap_or_default();

    let daily_hard_limit = budget_config.daily_hard_limit;
    let today_total_tokens = report.today_tokens;
    let remaining_daily_tokens = daily_hard_limit.saturating_sub(today_total_tokens);

    let mut estimated_total_units = 0.0;
    let by_provider = report
        .by_agent
        .iter()
        .map(|item| {
            let estimate = repodesk_core::usage::cost::estimate_agent_cost(
                &cost_config,
                &item.agent,
                item.input_tokens,
                item.output_tokens,
            );
            estimated_total_units += estimate.estimated_cost_units;
            TokenUsageItem {
                provider: item.agent.clone(),
                model: None,
                input_tokens: item.input_tokens,
                output_tokens: item.output_tokens,
                total_tokens: item.total_tokens,
                estimated_cost_units: Some(estimate.estimated_cost_units),
                currency_label: Some(estimate.currency_label),
            }
        })
        .collect::<Vec<_>>();

    let by_model = report
        .by_model
        .iter()
        .map(|item| {
            let estimate = repodesk_core::usage::cost::estimate_agent_cost(
                &cost_config,
                &item.agent,
                item.input_tokens,
                item.output_tokens,
            );
            TokenUsageItem {
                provider: item.agent.clone(),
                model: Some(item.model.clone()),
                input_tokens: item.input_tokens,
                output_tokens: item.output_tokens,
                total_tokens: item.total_tokens,
                estimated_cost_units: Some(estimate.estimated_cost_units),
                currency_label: Some(estimate.currency_label),
            }
        })
        .collect::<Vec<_>>();

    TokenUsageSnapshot {
        generated_at_ms: now_ms(),
        totals: TokenTotals {
            entries_count: report.entries_count,
            total_input_tokens: report.total_input_tokens,
            total_output_tokens: report.total_output_tokens,
            total_tokens: report.total_tokens,
            today_total_tokens,
            remaining_daily_tokens,
        },
        by_provider,
        by_model,
        active_artifacts: vec![
            token_artifact_estimate("context"),
            token_artifact_estimate("smart_context"),
            token_artifact_estimate("prompt_codex"),
            token_artifact_estimate("prompt_chatgpt"),
            token_artifact_estimate("prompt_review"),
            token_artifact_estimate("checks_summary"),
        ],
        cost_estimate: TokenCostSummary {
            estimated_total_units,
            currency_label: cost_config.currency_label,
            note: "Planning estimate from local RepoDesk cost config. Real billing depends on provider and model."
                .into(),
        },
    }
}

#[tauri::command]
pub fn token_usage_snapshot() -> TokenUsageSnapshot {
    build_token_usage_snapshot()
}

#[tauri::command]
pub fn log_token_usage(input: LogTokenUsageInput) -> Result<TokenUsageSnapshot, ErrorPayload> {
    validate_short_id("Provider", &input.provider)?;
    if let Some(model) = &input.model {
        validate_model_name("Model", model)?;
    }
    validate_short_id("Category", &input.category)?;
    validate_optional_notes(&input.notes)?;

    if input.input_tokens > 10_000_000 || input.output_tokens > 10_000_000 {
        return Err(ErrorPayload::resource_limit("Token counts are too large"));
    }

    repodesk_core::usage::token_ledger::log_token_event(
        repodesk_core::usage::token_ledger::LogTokenInput {
            agent: input.provider.trim().to_ascii_lowercase(),
            model: input
                .model
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            input_tokens: input.input_tokens,
            output_tokens: input.output_tokens,
            category: input.category.trim().to_string(),
            notes: input.notes,
        },
    )
    .map_err(ErrorPayload::from)?;

    Ok(build_token_usage_snapshot())
}

#[tauri::command]
pub fn estimate_raw_text(text: String) -> repodesk_core::tokens::TokenEstimate {
    repodesk_core::tokens::estimate_text(&text)
}

/// Per-day token/cost trend over the last `days` calendar days (oldest-first,
/// continuous — empty days are zero). Defaults to 14 days, capped at 90.
#[tauri::command]
pub fn token_cost_trend(
    days: Option<usize>,
) -> Result<Vec<repodesk_core::usage::token_ledger::CostTrendPoint>, ErrorPayload> {
    let window = days.unwrap_or(14).clamp(1, 90);
    repodesk_core::usage::token_ledger::cost_trend(window).map_err(ErrorPayload::from)
}

/// The current cost rate card (`costs.toml`), seeded with USD defaults on first
/// run. Surfaced so the Models & Cost surface can show and edit real rates.
#[tauri::command]
pub async fn cost_config_get() -> Result<repodesk_core::usage::cost::CostConfig, ErrorPayload> {
    repodesk_core::usage::cost::load_cost_config().map_err(ErrorPayload::from)
}

/// Persist an edited cost rate card.
#[tauri::command]
pub async fn cost_config_save(
    config: repodesk_core::usage::cost::CostConfig,
) -> Result<repodesk_core::usage::cost::CostConfig, ErrorPayload> {
    repodesk_core::usage::cost::save_cost_config(&config).map_err(ErrorPayload::from)?;
    Ok(config)
}

/// Reset the cost rate card to the built-in USD defaults.
#[tauri::command]
pub async fn cost_config_reset() -> Result<repodesk_core::usage::cost::CostConfig, ErrorPayload> {
    repodesk_core::usage::cost::reset_cost_config().map_err(ErrorPayload::from)
}
