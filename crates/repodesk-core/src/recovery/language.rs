use chrono::{DateTime, Utc};

use crate::language_intelligence::{
    LanguageServerAvailability, LanguageServerDescriptor, LanguageServerProfileState,
};
use crate::language_tools::{LanguageToolInstallState, LanguageToolInstallStatus};

use super::types::{
    RecoveryAction, RecoveryActionKind, RecoveryEvidence, RecoveryFailureCode, RecoveryObservation,
    RecoverySeverity, RecoveryState,
};

const MAX_LANGUAGE_EVIDENCE_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageRuntimeState {
    NotStarted,
    Starting,
    Ready,
    Error,
}

pub struct LanguageRecoveryInput<'a> {
    pub project: &'a str,
    pub generation: u64,
    pub descriptor: &'a LanguageServerDescriptor,
    pub runtime: LanguageRuntimeState,
    pub runtime_error: Option<&'a str>,
    pub install: Option<&'a LanguageToolInstallStatus>,
    pub observed_at: DateTime<Utc>,
}

pub fn language_observation(input: LanguageRecoveryInput<'_>) -> Option<RecoveryObservation> {
    if input.descriptor.profile_state == LanguageServerProfileState::DiscoveryOnly {
        return None;
    }

    let capability_id = format!("language:{}", input.descriptor.id);
    let unaffected = vec![
        "Editing".into(),
        "Syntax highlighting".into(),
        "Scrolling".into(),
        "Selection".into(),
        "Save".into(),
    ];
    let affected = advertised_capabilities(input.descriptor);
    let project_evidence = RecoveryEvidence {
        label: "Project".into(),
        value: bounded(input.project),
    };

    if input.runtime == LanguageRuntimeState::Ready {
        return Some(RecoveryObservation {
            capability_id,
            module_id: "language_intelligence".into(),
            generation: input.generation,
            observed_at: input.observed_at,
            state: RecoveryState::Healthy,
            severity: RecoverySeverity::Info,
            code: None,
            title: format!("{} is ready", input.descriptor.label),
            explanation: "Language intelligence completed protocol initialization.".into(),
            affected,
            unaffected,
            evidence: vec![project_evidence],
            actions: vec![],
        });
    }

    if let Some(install) = input.install {
        match install.state {
            LanguageToolInstallState::Installing | LanguageToolInstallState::Ready => {
                return Some(RecoveryObservation {
                    capability_id,
                    module_id: "language_intelligence".into(),
                    generation: input.generation,
                    observed_at: input.observed_at,
                    state: RecoveryState::Repairing,
                    severity: RecoverySeverity::Info,
                    code: None,
                    title: format!("Repairing {}", input.descriptor.label),
                    explanation: if install.state == LanguageToolInstallState::Ready {
                        "The managed tool passed its version probe. RepoDesk is verifying live protocol initialization."
                    } else {
                        "RepoDesk is installing the approved managed language tool."
                    }
                    .into(),
                    affected,
                    unaffected,
                    evidence: vec![
                        project_evidence,
                        RecoveryEvidence {
                            label: "Progress".into(),
                            value: format!("{}% — {}", install.progress, bounded(&install.message)),
                        },
                    ],
                    actions: vec![],
                });
            }
            LanguageToolInstallState::Cancelled | LanguageToolInstallState::Error => {}
        }
    }

    if input.descriptor.availability == LanguageServerAvailability::Missing {
        let recipe_id = input.descriptor.install_recipe_id.clone();
        let actions = recipe_id
            .map(|recipe_id| RecoveryAction {
                id: "install-managed-language-server".into(),
                label: "Review repair".into(),
                kind: RecoveryActionKind::Confirmable,
                recipe_id: Some(recipe_id),
            })
            .into_iter()
            .collect();
        let mut evidence = vec![project_evidence];
        if let Some(install) = input.install {
            if install.state == LanguageToolInstallState::Error {
                evidence.push(RecoveryEvidence {
                    label: "Install error".into(),
                    value: bounded(install.error.as_deref().unwrap_or(&install.message)),
                });
            } else if install.state == LanguageToolInstallState::Cancelled {
                evidence.push(RecoveryEvidence {
                    label: "Install status".into(),
                    value: bounded(&install.message),
                });
            }
        }
        return Some(RecoveryObservation {
            capability_id,
            module_id: "language_intelligence".into(),
            generation: input.generation,
            observed_at: input.observed_at,
            state: if input.descriptor.install_recipe_id.is_some() {
                RecoveryState::NeedsApproval
            } else {
                RecoveryState::Blocked
            },
            severity: RecoverySeverity::Warning,
            code: Some(RecoveryFailureCode::MissingExecutable),
            title: format!("{} is unavailable", input.descriptor.label),
            explanation: "The configured language-server executable was not found. Editing and save remain available."
                .into(),
            affected,
            unaffected,
            evidence,
            actions,
        });
    }

    if input.runtime == LanguageRuntimeState::Error {
        let mut evidence = vec![project_evidence];
        if let Some(error) = input.runtime_error {
            evidence.push(RecoveryEvidence {
                label: "Last error".into(),
                value: bounded(error),
            });
        }
        return Some(RecoveryObservation {
            capability_id,
            module_id: "language_intelligence".into(),
            generation: input.generation,
            observed_at: input.observed_at,
            state: RecoveryState::Degraded,
            severity: RecoverySeverity::Warning,
            code: Some(RecoveryFailureCode::InitializationFailed),
            title: format!("{} could not start", input.descriptor.label),
            explanation:
                "Language intelligence failed to initialize. Editing and save remain available."
                    .into(),
            affected,
            unaffected,
            evidence,
            actions: vec![RecoveryAction {
                id: "restart-language-session".into(),
                label: "Restart language session".into(),
                kind: RecoveryActionKind::Automatic,
                recipe_id: None,
            }],
        });
    }

    Some(RecoveryObservation {
        capability_id,
        module_id: "language_intelligence".into(),
        generation: input.generation,
        observed_at: input.observed_at,
        state: RecoveryState::Unknown,
        severity: RecoverySeverity::Info,
        code: None,
        title: format!("{} has not been checked", input.descriptor.label),
        explanation: match input.runtime {
            LanguageRuntimeState::Starting => "Language intelligence is starting.",
            LanguageRuntimeState::NotStarted => {
                "Language intelligence will be checked when this capability is used."
            }
            LanguageRuntimeState::Ready | LanguageRuntimeState::Error => unreachable!(),
        }
        .into(),
        affected,
        unaffected,
        evidence: vec![project_evidence],
        actions: vec![],
    })
}

fn advertised_capabilities(descriptor: &LanguageServerDescriptor) -> Vec<String> {
    let capabilities = &descriptor.capabilities;
    [
        (capabilities.diagnostics, "Diagnostics"),
        (capabilities.hover, "Hover"),
        (capabilities.definition, "Definitions"),
        (capabilities.references, "References"),
        (capabilities.completion, "Completion"),
        (capabilities.rename, "Rename"),
        (capabilities.formatting, "Formatting"),
        (capabilities.document_symbols, "Symbols"),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, label)| label.into())
    .collect()
}

fn bounded(value: &str) -> String {
    let mut characters = value.chars();
    let bounded = characters
        .by_ref()
        .take(MAX_LANGUAGE_EVIDENCE_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}
