use chrono::{DateTime, TimeZone, Utc};
use repodesk_core::language_intelligence::{
    LanguageServerAvailability, LanguageServerCapabilities, LanguageServerDescriptor,
    LanguageServerInitializationProfile, LanguageServerProfileState,
};
use repodesk_core::language_tools::{LanguageToolInstallState, LanguageToolInstallStatus};
use repodesk_core::recovery::{
    LanguageRecoveryInput, LanguageRuntimeState, RecoveryActionKind, RecoveryFailureCode,
    RecoveryState, language_observation,
};

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, second).unwrap()
}

fn typescript_descriptor(
    availability: LanguageServerAvailability,
    profile_state: LanguageServerProfileState,
) -> LanguageServerDescriptor {
    LanguageServerDescriptor {
        id: "typescript-language-server".into(),
        label: "TypeScript Language Server".into(),
        executable: "typescript-language-server".into(),
        arguments: vec!["--stdio".into()],
        languages: vec!["typescript".into(), "javascript".into()],
        availability,
        source: None,
        capabilities: LanguageServerCapabilities {
            diagnostics: true,
            hover: true,
            definition: true,
            references: true,
            completion: true,
            rename: true,
            formatting: true,
            document_symbols: true,
        },
        profile_state,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("typescript-language-server".into()),
    }
}

#[test]
fn missing_active_server_requires_approved_managed_install() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Missing,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 1,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::NotStarted,
        runtime_error: None,
        install: None,
        observed_at: at(1),
    })
    .unwrap();

    assert_eq!(observation.state, RecoveryState::NeedsApproval);
    assert_eq!(
        observation.code,
        Some(RecoveryFailureCode::MissingExecutable)
    );
    assert_eq!(observation.actions[0].kind, RecoveryActionKind::Confirmable);
    assert_eq!(
        observation.actions[0].recipe_id.as_deref(),
        Some("typescript-language-server")
    );
    assert!(observation.unaffected.contains(&"Editing".to_string()));
}

#[test]
fn initialized_server_is_healthy() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 2,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Ready,
        runtime_error: None,
        install: None,
        observed_at: at(2),
    })
    .unwrap();

    assert_eq!(observation.state, RecoveryState::Healthy);
    assert_eq!(observation.code, None);
    assert!(observation.actions.is_empty());
}

#[test]
fn initialization_error_is_degraded_and_restart_is_automatic() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 3,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Error,
        runtime_error: Some("initialize request timed out"),
        install: None,
        observed_at: at(3),
    })
    .unwrap();

    assert_eq!(observation.state, RecoveryState::Degraded);
    assert_eq!(
        observation.code,
        Some(RecoveryFailureCode::InitializationFailed)
    );
    assert_eq!(observation.actions[0].kind, RecoveryActionKind::Automatic);
    assert_eq!(observation.actions[0].id, "restart-language-session");
    assert!(
        observation
            .evidence
            .iter()
            .any(|item| item.value.contains("initialize request timed out"))
    );
}

#[test]
fn discovery_only_profile_does_not_create_actionable_record() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Missing,
        LanguageServerProfileState::DiscoveryOnly,
    );
    assert!(
        language_observation(LanguageRecoveryInput {
            project: "RepoDesk",
            generation: 4,
            descriptor: &descriptor,
            runtime: LanguageRuntimeState::NotStarted,
            runtime_error: None,
            install: None,
            observed_at: at(4),
        })
        .is_none()
    );
}

#[test]
fn installer_success_stays_repairing_until_runtime_verification() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let install = LanguageToolInstallStatus {
        recipe_id: "typescript-language-server".into(),
        state: LanguageToolInstallState::Ready,
        progress: 100,
        message: "Language server installed and version-probed".into(),
        started_at: at(4),
        finished_at: Some(at(5)),
        error: None,
    };
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 5,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Starting,
        runtime_error: None,
        install: Some(&install),
        observed_at: at(5),
    })
    .unwrap();

    assert_eq!(observation.state, RecoveryState::Repairing);
    assert_eq!(observation.code, None);
    assert!(observation.actions.is_empty());
}

#[test]
fn failed_install_preserves_bounded_error_and_requires_new_approval() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Missing,
        LanguageServerProfileState::Active,
    );
    let install = LanguageToolInstallStatus {
        recipe_id: "typescript-language-server".into(),
        state: LanguageToolInstallState::Error,
        progress: 100,
        message: "Installer failed".into(),
        started_at: at(5),
        finished_at: Some(at(6)),
        error: Some(format!("npm registry unavailable {}", "x".repeat(2_100))),
    };
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 6,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::NotStarted,
        runtime_error: None,
        install: Some(&install),
        observed_at: at(6),
    })
    .unwrap();

    assert_eq!(observation.state, RecoveryState::NeedsApproval);
    assert_eq!(observation.actions[0].kind, RecoveryActionKind::Confirmable);
    let install_error = observation
        .evidence
        .iter()
        .find(|item| item.label == "Install error")
        .expect("install error evidence");
    assert!(install_error.value.starts_with("npm registry unavailable"));
    assert!(install_error.value.ends_with('…'));
    assert!(install_error.value.chars().count() <= 2_001);
}
