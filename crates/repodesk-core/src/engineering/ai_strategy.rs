//! Deterministic execution-strategy recommendations for AI-assisted work.
//!
//! Strategy is deliberately evidence-backed and explainable. It never invents
//! a synthetic productivity score and it never relaxes the Work Item scope,
//! human ChangeSet review, verification, or commit gates. The only decisions
//! here are how much *AI orchestration* is justified and whether read-only
//! reasoning should prefer local/cheaper routes.

use serde::{Deserialize, Serialize};

use super::ai_usage_intelligence::{AiUsageReport, AiUsageSignalCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiStrategyMode {
    #[default]
    Auto,
    Lean,
    Balanced,
    LocalFirst,
    Quality,
}

impl AiStrategyMode {
    pub fn from_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "lean" => Some(Self::Lean),
            "balanced" => Some(Self::Balanced),
            "local_first" | "local-first" | "local" => Some(Self::LocalFirst),
            "quality" => Some(Self::Quality),
            _ => None,
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Lean => "lean",
            Self::Balanced => "balanced",
            Self::LocalFirst => "local_first",
            Self::Quality => "quality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStrategyProfile {
    Lean,
    Balanced,
    LocalFirst,
    Quality,
}

impl AiStrategyProfile {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Lean => "lean",
            Self::Balanced => "balanced",
            Self::LocalFirst => "local_first",
            Self::Quality => "quality",
        }
    }

    /// Existing router economy-mode labels. Patch/coding-agent routing keeps its
    /// normal safety constraints; this primarily nudges read-only reasoning.
    pub fn economy_mode(self) -> &'static str {
        match self {
            Self::Lean | Self::LocalFirst => "economy",
            Self::Balanced => "balanced",
            Self::Quality => "quality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPlanShape {
    /// One bounded coding-agent step. The normal human Review phase still gates
    /// the returned ChangeSet before verification.
    SingleWriter,
    /// Coding agent followed by an independent read-only AI review.
    WriterWithReview,
    /// Analyze -> implement -> independent AI review.
    AnalyzeWriterReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStrategyReasonCode {
    ExplicitMode,
    ColdStart,
    NarrowScope,
    WideOrUnknownScope,
    RepeatedContext,
    AgentFanout,
    PromptHeavy,
    ExecutionChurn,
    ChangeRejection,
    VerificationInstability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStrategyReason {
    pub code: AiStrategyReasonCode,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AiStrategyInputs {
    /// Number of explicit allowed paths in the typed Work Item Contract. Zero
    /// means unknown/unconfigured, not an empty write permission set.
    pub scope_path_count: usize,
    pub protected_path_count: usize,
    pub context_prepared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStrategyRecommendation {
    pub requested_mode: AiStrategyMode,
    pub profile: AiStrategyProfile,
    pub plan_shape: AiPlanShape,
    pub economy_mode: String,
    pub reuse_prepared_context: bool,
    pub max_agent_steps: usize,
    pub independent_ai_review: bool,
    pub reasons: Vec<AiStrategyReason>,
}

/// Derive an explainable strategy from already-observed Work Item evidence.
///
/// Priority for `auto` is intentionally conservative:
/// 1. churn/rejection/verification instability -> quality/full pipeline;
/// 2. repeated context or hand-off fan-out on a narrow scope -> lean;
/// 3. strongly input-heavy usage -> local-first;
/// 4. otherwise keep the existing balanced three-step pipeline.
pub fn derive_ai_strategy(
    usage: &AiUsageReport,
    inputs: AiStrategyInputs,
    requested_mode: AiStrategyMode,
) -> AiStrategyRecommendation {
    let unstable = has_signal(usage, AiUsageSignalCode::ExecutionChurn)
        || has_signal(usage, AiUsageSignalCode::ChangeRejection)
        || has_signal(usage, AiUsageSignalCode::VerificationInstability);
    let repeated = has_signal(usage, AiUsageSignalCode::RepeatedContext);
    let fanout = has_signal(usage, AiUsageSignalCode::AgentFanout);
    let prompt_heavy = has_signal(usage, AiUsageSignalCode::PromptHeavy);
    let narrow_scope = (1..=4).contains(&inputs.scope_path_count);

    let profile = match requested_mode {
        AiStrategyMode::Lean => AiStrategyProfile::Lean,
        AiStrategyMode::Balanced => AiStrategyProfile::Balanced,
        AiStrategyMode::LocalFirst => AiStrategyProfile::LocalFirst,
        AiStrategyMode::Quality => AiStrategyProfile::Quality,
        AiStrategyMode::Auto if unstable => AiStrategyProfile::Quality,
        AiStrategyMode::Auto if narrow_scope && (repeated || fanout) => AiStrategyProfile::Lean,
        AiStrategyMode::Auto if prompt_heavy => AiStrategyProfile::LocalFirst,
        AiStrategyMode::Auto => AiStrategyProfile::Balanced,
    };

    let plan_shape = match profile {
        AiStrategyProfile::Lean if narrow_scope && !unstable => AiPlanShape::SingleWriter,
        AiStrategyProfile::Lean => AiPlanShape::WriterWithReview,
        AiStrategyProfile::Balanced
        | AiStrategyProfile::LocalFirst
        | AiStrategyProfile::Quality => AiPlanShape::AnalyzeWriterReview,
    };

    let mut reasons = Vec::new();
    if requested_mode != AiStrategyMode::Auto {
        reasons.push(AiStrategyReason {
            code: AiStrategyReasonCode::ExplicitMode,
            detail: format!(
                "The user explicitly selected the {} execution strategy.",
                requested_mode.as_label()
            ),
        });
    } else if usage.context.builds == 0 && usage.orchestration.managed_executions == 0 {
        reasons.push(AiStrategyReason {
            code: AiStrategyReasonCode::ColdStart,
            detail: "There is not enough historical execution evidence yet, so Auto keeps the balanced pipeline.".into(),
        });
    }

    if narrow_scope {
        reasons.push(AiStrategyReason {
            code: AiStrategyReasonCode::NarrowScope,
            detail: format!(
                "The Work Item contract bounds writes to {} explicit path(s).",
                inputs.scope_path_count
            ),
        });
    } else {
        reasons.push(AiStrategyReason {
            code: AiStrategyReasonCode::WideOrUnknownScope,
            detail: if inputs.scope_path_count == 0 {
                "The typed write scope is not explicit, so RepoDesk keeps extra AI review unless another mode is selected.".into()
            } else {
                format!(
                    "The Work Item allows {} paths, so RepoDesk treats it as a wider change boundary.",
                    inputs.scope_path_count
                )
            },
        });
    }

    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::RepeatedContext,
        AiStrategyReasonCode::RepeatedContext,
    );
    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::AgentFanout,
        AiStrategyReasonCode::AgentFanout,
    );
    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::PromptHeavy,
        AiStrategyReasonCode::PromptHeavy,
    );
    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::ExecutionChurn,
        AiStrategyReasonCode::ExecutionChurn,
    );
    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::ChangeRejection,
        AiStrategyReasonCode::ChangeRejection,
    );
    push_signal_reason(
        &mut reasons,
        usage,
        AiUsageSignalCode::VerificationInstability,
        AiStrategyReasonCode::VerificationInstability,
    );

    AiStrategyRecommendation {
        requested_mode,
        profile,
        plan_shape,
        economy_mode: profile.economy_mode().to_string(),
        reuse_prepared_context: inputs.context_prepared,
        max_agent_steps: match plan_shape {
            AiPlanShape::SingleWriter => 1,
            AiPlanShape::WriterWithReview => 2,
            AiPlanShape::AnalyzeWriterReview => 3,
        },
        independent_ai_review: plan_shape != AiPlanShape::SingleWriter,
        reasons,
    }
}

fn has_signal(usage: &AiUsageReport, code: AiUsageSignalCode) -> bool {
    usage.signals.iter().any(|signal| signal.code == code)
}

fn push_signal_reason(
    reasons: &mut Vec<AiStrategyReason>,
    usage: &AiUsageReport,
    signal_code: AiUsageSignalCode,
    reason_code: AiStrategyReasonCode,
) {
    if let Some(signal) = usage.signals.iter().find(|signal| signal.code == signal_code) {
        reasons.push(AiStrategyReason {
            code: reason_code,
            detail: signal.detail.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::ai_usage_intelligence::{
        AiContextEfficiency, AiOrchestrationEfficiency, AiUsageSignal, AiUsageSignalSeverity,
    };

    fn report_with(code: AiUsageSignalCode) -> AiUsageReport {
        AiUsageReport {
            context: AiContextEfficiency {
                builds: 2,
                ..AiContextEfficiency::default()
            },
            orchestration: AiOrchestrationEfficiency {
                managed_executions: 1,
                ..AiOrchestrationEfficiency::default()
            },
            signals: vec![AiUsageSignal {
                code,
                severity: AiUsageSignalSeverity::Warning,
                title: "signal".into(),
                detail: "evidence detail".into(),
                recommendation: "recommendation".into(),
            }],
            ..AiUsageReport::default()
        }
    }

    #[test]
    fn auto_collapses_fanout_for_narrow_scoped_work() {
        let recommendation = derive_ai_strategy(
            &report_with(AiUsageSignalCode::AgentFanout),
            AiStrategyInputs {
                scope_path_count: 2,
                protected_path_count: 1,
                context_prepared: true,
            },
            AiStrategyMode::Auto,
        );

        assert_eq!(recommendation.profile, AiStrategyProfile::Lean);
        assert_eq!(recommendation.plan_shape, AiPlanShape::SingleWriter);
        assert_eq!(recommendation.max_agent_steps, 1);
        assert!(recommendation.reuse_prepared_context);
    }

    #[test]
    fn instability_wins_over_fanout_and_keeps_full_quality_pipeline() {
        let mut report = report_with(AiUsageSignalCode::AgentFanout);
        report.signals.push(AiUsageSignal {
            code: AiUsageSignalCode::VerificationInstability,
            severity: AiUsageSignalSeverity::Warning,
            title: "unstable".into(),
            detail: "checks keep failing".into(),
            recommendation: "inspect failures".into(),
        });

        let recommendation = derive_ai_strategy(
            &report,
            AiStrategyInputs {
                scope_path_count: 2,
                protected_path_count: 0,
                context_prepared: true,
            },
            AiStrategyMode::Auto,
        );

        assert_eq!(recommendation.profile, AiStrategyProfile::Quality);
        assert_eq!(recommendation.plan_shape, AiPlanShape::AnalyzeWriterReview);
        assert!(recommendation.independent_ai_review);
    }

    #[test]
    fn explicit_lean_keeps_review_when_scope_is_unknown() {
        let recommendation = derive_ai_strategy(
            &AiUsageReport::default(),
            AiStrategyInputs::default(),
            AiStrategyMode::Lean,
        );

        assert_eq!(recommendation.profile, AiStrategyProfile::Lean);
        assert_eq!(recommendation.plan_shape, AiPlanShape::WriterWithReview);
        assert_eq!(recommendation.max_agent_steps, 2);
    }

    #[test]
    fn prompt_heavy_auto_prefers_local_first_when_no_instability_exists() {
        let recommendation = derive_ai_strategy(
            &report_with(AiUsageSignalCode::PromptHeavy),
            AiStrategyInputs {
                scope_path_count: 8,
                protected_path_count: 0,
                context_prepared: true,
            },
            AiStrategyMode::Auto,
        );

        assert_eq!(recommendation.profile, AiStrategyProfile::LocalFirst);
        assert_eq!(recommendation.economy_mode, "economy");
    }
}
