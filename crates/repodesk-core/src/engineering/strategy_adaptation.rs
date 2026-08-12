//! Conservative feedback adaptation for Auto AI strategy.
//!
//! The base strategy remains the primary policy. Historical outcomes may only
//! strengthen or veto an optimization after enough settled runs exist. Current
//! safety/quality signals always win over historical token savings.

use super::ai_strategy::{
    AiPlanShape, AiStrategyInputs, AiStrategyMode, AiStrategyProfile, AiStrategyRecommendation,
    derive_ai_strategy,
};
use super::ai_usage_intelligence::{AiUsageReport, AiUsageSignalCode};
use super::strategy_feedback::{StrategyFeedbackReport, StrategyProfileFeedback};

const STRONG_SUCCESS_RATE: f64 = 0.80;
const WEAK_SUCCESS_RATE: f64 = 0.50;

impl AiStrategyProfile {
    pub fn from_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "lean" => Some(Self::Lean),
            "balanced" => Some(Self::Balanced),
            "local_first" | "local-first" => Some(Self::LocalFirst),
            "quality" => Some(Self::Quality),
            _ => None,
        }
    }
}

/// Apply project/Work Item-local settled outcomes to Auto without changing the
/// semantics of explicit modes.
pub fn derive_ai_strategy_with_feedback(
    usage: &AiUsageReport,
    inputs: AiStrategyInputs,
    requested_mode: AiStrategyMode,
    feedback: &StrategyFeedbackReport,
) -> AiStrategyRecommendation {
    let mut recommendation = derive_ai_strategy(usage, inputs, requested_mode);
    if requested_mode != AiStrategyMode::Auto {
        return recommendation;
    }

    // Current instability is a hard policy: historical success does not justify
    // optimizing through fresh execution/review/verification problems.
    if has_current_instability(usage) {
        return recommendation;
    }

    let narrow_scope = inputs.scope_file_bounded && (1..=4).contains(&inputs.scope_path_count);
    let lean = profile(feedback, AiStrategyProfile::Lean);
    let local = profile(feedback, AiStrategyProfile::LocalFirst);

    // A repeatedly weak Lean history vetoes automatic fan-out collapse. Explicit
    // Lean remains available to the human, but Auto falls back to Balanced.
    if recommendation.profile == AiStrategyProfile::Lean && history_is_weak(lean) {
        apply_profile(&mut recommendation, AiStrategyProfile::Balanced, false);
        return recommendation;
    }

    // Once Lean has enough settled evidence and succeeds reliably for this Work
    // Item, narrow file-bounded work can reuse that strategy even if the generic
    // fan-out warning has cooled down after earlier optimizations.
    if recommendation.profile == AiStrategyProfile::Balanced
        && narrow_scope
        && history_is_strong(lean)
    {
        apply_profile(&mut recommendation, AiStrategyProfile::Lean, true);
        return recommendation;
    }

    // Prompt-heavy Auto normally prefers Local-first. A settled weak Local-first
    // history vetoes that optimization until the user explicitly chooses it.
    if recommendation.profile == AiStrategyProfile::LocalFirst && history_is_weak(local) {
        apply_profile(&mut recommendation, AiStrategyProfile::Balanced, false);
    }

    recommendation
}

fn profile(
    report: &StrategyFeedbackReport,
    profile: AiStrategyProfile,
) -> Option<&StrategyProfileFeedback> {
    report.profiles.iter().find(|item| item.profile == profile)
}

fn history_is_strong(value: Option<&StrategyProfileFeedback>) -> bool {
    value.is_some_and(|value| {
        value.adaptation_ready && value.success_rate.is_some_and(|rate| rate >= STRONG_SUCCESS_RATE)
    })
}

fn history_is_weak(value: Option<&StrategyProfileFeedback>) -> bool {
    value.is_some_and(|value| {
        value.adaptation_ready && value.success_rate.is_some_and(|rate| rate < WEAK_SUCCESS_RATE)
    })
}

fn has_current_instability(usage: &AiUsageReport) -> bool {
    usage.signals.iter().any(|signal| {
        matches!(
            signal.code,
            AiUsageSignalCode::ExecutionChurn
                | AiUsageSignalCode::ChangeRejection
                | AiUsageSignalCode::VerificationInstability
        )
    })
}

fn apply_profile(
    recommendation: &mut AiStrategyRecommendation,
    profile: AiStrategyProfile,
    narrow_scope: bool,
) {
    recommendation.profile = profile;
    recommendation.economy_mode = profile.economy_mode().to_string();
    recommendation.plan_shape = match profile {
        AiStrategyProfile::Lean if narrow_scope => AiPlanShape::SingleWriter,
        AiStrategyProfile::Lean => AiPlanShape::WriterWithReview,
        AiStrategyProfile::Balanced
        | AiStrategyProfile::LocalFirst
        | AiStrategyProfile::Quality => AiPlanShape::AnalyzeWriterReview,
    };
    recommendation.max_agent_steps = match recommendation.plan_shape {
        AiPlanShape::SingleWriter => 1,
        AiPlanShape::WriterWithReview => 2,
        AiPlanShape::AnalyzeWriterReview => 3,
    };
    recommendation.independent_ai_review = recommendation.plan_shape != AiPlanShape::SingleWriter;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::strategy_feedback::StrategyProfileFeedback;

    fn profile_feedback(
        profile: AiStrategyProfile,
        settled: usize,
        succeeded: usize,
    ) -> StrategyProfileFeedback {
        StrategyProfileFeedback {
            profile,
            runs: settled,
            settled_runs: settled,
            succeeded_runs: succeeded,
            failed_runs: settled.saturating_sub(succeeded),
            pending_runs: 0,
            success_rate: (settled > 0).then_some(succeeded as f64 / settled as f64),
            total_actual_tokens: 0,
            total_actual_cost_units: 0.0,
            average_actual_tokens: None,
            average_actual_cost_units: None,
            average_token_estimate_error_ratio: None,
            adaptation_ready: settled >= 3,
        }
    }

    fn narrow_inputs() -> AiStrategyInputs {
        AiStrategyInputs {
            scope_path_count: 2,
            scope_file_bounded: true,
            protected_path_count: 0,
            context_prepared: true,
        }
    }

    #[test]
    fn strong_lean_history_can_keep_auto_lean_after_fanout_warning_disappears() {
        let feedback = StrategyFeedbackReport {
            strategy_runs: 5,
            settled_runs: 5,
            pending_runs: 0,
            profiles: vec![profile_feedback(AiStrategyProfile::Lean, 5, 5)],
            recent_runs: Vec::new(),
        };
        let recommendation = derive_ai_strategy_with_feedback(
            &AiUsageReport::default(),
            narrow_inputs(),
            AiStrategyMode::Auto,
            &feedback,
        );
        assert_eq!(recommendation.profile, AiStrategyProfile::Lean);
        assert_eq!(recommendation.plan_shape, AiPlanShape::SingleWriter);
    }

    #[test]
    fn weak_lean_history_vetoes_auto_fanout_collapse() {
        let mut usage = AiUsageReport::default();
        usage.signals.push(crate::engineering::AiUsageSignal {
            code: AiUsageSignalCode::AgentFanout,
            severity: crate::engineering::AiUsageSignalSeverity::Warning,
            title: "fanout".into(),
            detail: "fanout".into(),
            recommendation: "reduce".into(),
        });
        let feedback = StrategyFeedbackReport {
            strategy_runs: 4,
            settled_runs: 4,
            pending_runs: 0,
            profiles: vec![profile_feedback(AiStrategyProfile::Lean, 4, 1)],
            recent_runs: Vec::new(),
        };
        let recommendation = derive_ai_strategy_with_feedback(
            &usage,
            narrow_inputs(),
            AiStrategyMode::Auto,
            &feedback,
        );
        assert_eq!(recommendation.profile, AiStrategyProfile::Balanced);
    }

    #[test]
    fn explicit_mode_is_never_rewritten_by_feedback() {
        let feedback = StrategyFeedbackReport {
            strategy_runs: 4,
            settled_runs: 4,
            pending_runs: 0,
            profiles: vec![profile_feedback(AiStrategyProfile::Lean, 4, 0)],
            recent_runs: Vec::new(),
        };
        let recommendation = derive_ai_strategy_with_feedback(
            &AiUsageReport::default(),
            narrow_inputs(),
            AiStrategyMode::Lean,
            &feedback,
        );
        assert_eq!(recommendation.profile, AiStrategyProfile::Lean);
    }
}
