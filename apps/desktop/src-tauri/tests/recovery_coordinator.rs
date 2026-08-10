use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, TimeZone, Utc};
use repodesk_core::language_intelligence::{
    LanguageIntelligenceSnapshot, LanguageServerAvailability, LanguageServerCapabilities,
    LanguageServerDescriptor, LanguageServerInitializationProfile, LanguageServerProfileState,
};
use repodesk_core::language_tools::{
    LanguageToolCommand, LanguageToolInstallPreview, LanguageToolInstallResult,
    LanguageToolInstallState, LanguageToolInstallStatus, LanguageToolInstaller,
};
use repodesk_core::recovery::{RecoveryActionKind, RecoveryRecord, RecoveryState};
use repodesk_desktop_lib::commands::recovery::{
    LanguageRuntimeStatus, RecoveryCoordinator, RecoveryLanguageServices,
};

const CAPABILITY_ID: &str = "language:typescript-language-server";
const INSTALL_ACTION: &str = "install-managed-language-server";

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, second).unwrap()
}

fn descriptor(availability: LanguageServerAvailability) -> LanguageServerDescriptor {
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
        profile_state: LanguageServerProfileState::Active,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("typescript-language-server".into()),
    }
}

fn runtime(state: repodesk_core::recovery::LanguageRuntimeState) -> LanguageRuntimeStatus {
    LanguageRuntimeStatus {
        project: "RepoDesk".into(),
        server_id: "typescript-language-server".into(),
        state,
        last_error: (state == repodesk_core::recovery::LanguageRuntimeState::Error)
            .then(|| "initialize request failed".into()),
    }
}

fn install_status(
    state: LanguageToolInstallState,
    progress: u8,
    message: &str,
) -> LanguageToolInstallStatus {
    LanguageToolInstallStatus {
        recipe_id: "typescript-language-server".into(),
        state,
        progress,
        message: message.into(),
        started_at: at(1),
        finished_at: (state != LanguageToolInstallState::Installing).then(|| at(2)),
        error: (state == LanguageToolInstallState::Error).then(|| message.into()),
    }
}

struct FakeLanguageServices {
    descriptor: Mutex<LanguageServerDescriptor>,
    runtime: Mutex<LanguageRuntimeStatus>,
    install: Mutex<Option<LanguageToolInstallStatus>>,
    install_transport_error: AtomicBool,
    restart_becomes_ready: AtomicBool,
    restart_calls: AtomicUsize,
}

impl FakeLanguageServices {
    fn new(availability: LanguageServerAvailability) -> Self {
        Self {
            descriptor: Mutex::new(descriptor(availability)),
            runtime: Mutex::new(runtime(
                repodesk_core::recovery::LanguageRuntimeState::NotStarted,
            )),
            install: Mutex::new(None),
            install_transport_error: AtomicBool::new(false),
            restart_becomes_ready: AtomicBool::new(true),
            restart_calls: AtomicUsize::new(0),
        }
    }

    fn set_runtime(&self, state: repodesk_core::recovery::LanguageRuntimeState) {
        *self.runtime.lock().unwrap() = runtime(state);
    }

    fn set_availability(&self, availability: LanguageServerAvailability) {
        self.descriptor.lock().unwrap().availability = availability;
    }
}

impl RecoveryLanguageServices for FakeLanguageServices {
    fn discovery(&self) -> Result<LanguageIntelligenceSnapshot, String> {
        let descriptor = self.descriptor.lock().unwrap().clone();
        Ok(LanguageIntelligenceSnapshot {
            project: "RepoDesk".into(),
            primary_language: Some("typescript".into()),
            available_count: usize::from(
                descriptor.availability == LanguageServerAvailability::Available,
            ),
            servers: vec![descriptor],
            generated_at: at(1),
        })
    }

    fn statuses(&self) -> Vec<LanguageRuntimeStatus> {
        vec![self.runtime.lock().unwrap().clone()]
    }

    fn install_status(
        &self,
        _recipe_id: &str,
    ) -> Result<Option<LanguageToolInstallStatus>, String> {
        Ok(self.install.lock().unwrap().clone())
    }

    fn preview(&self, _recipe_id: &str) -> Result<LanguageToolInstallPreview, String> {
        Ok(LanguageToolInstallPreview {
            recipe_id: "typescript-language-server".into(),
            recipe_revision: "typescript-language-server:5.3.0:typescript:6.0.3".into(),
            server_id: "typescript-language-server".into(),
            server_label: "TypeScript Language Server".into(),
            languages: vec!["typescript".into(), "javascript".into()],
            installer: LanguageToolInstaller::Npm,
            package: "typescript-language-server".into(),
            version: "5.3.0".into(),
            destination: "/tmp/repodesk/tools/language-servers/typescript-language-server".into(),
            install_command: LanguageToolCommand {
                program: "npm".into(),
                args: vec!["install".into(), "typescript-language-server@5.3.0".into()],
            },
            probe_command: LanguageToolCommand {
                program: "/tmp/repodesk/tools/typescript-language-server".into(),
                args: vec!["--version".into()],
            },
            network_required: true,
            writes_outside_repository: vec!["/tmp/repodesk/tools/language-servers".into()],
            prerequisite_available: true,
            prerequisite_hint: None,
            confirmation_token: "adapter_secret_token".into(),
            expires_at: Utc::now() + Duration::minutes(5),
        })
    }

    fn install_observed(
        &self,
        token: &str,
        observer: &dyn Fn(&LanguageToolInstallStatus),
    ) -> Result<LanguageToolInstallResult, String> {
        if self.install_transport_error.load(Ordering::Relaxed) {
            return Err("installer process could not start".into());
        }
        if token != "adapter_secret_token" {
            return Err("wrong adapter token".into());
        }
        let installing = install_status(LanguageToolInstallState::Installing, 10, "Preparing");
        *self.install.lock().unwrap() = Some(installing.clone());
        observer(&installing);
        let ready = install_status(LanguageToolInstallState::Ready, 100, "Version probe passed");
        *self.install.lock().unwrap() = Some(ready.clone());
        observer(&ready);
        Ok(LanguageToolInstallResult {
            status: ready,
            executable: Some("/tmp/repodesk/tools/typescript-language-server".into()),
            output: "installed\nNPM_TOKEN=must-not-cross-boundary".into(),
        })
    }

    fn cancel(&self, _recipe_id: &str) -> Result<bool, String> {
        Ok(true)
    }

    fn restart(&self, _server_id: &str) -> Result<(), String> {
        self.restart_calls.fetch_add(1, Ordering::Relaxed);
        if self.restart_becomes_ready.load(Ordering::Relaxed) {
            self.set_runtime(repodesk_core::recovery::LanguageRuntimeState::Ready);
        }
        Ok(())
    }
}

fn coordinator(services: Arc<FakeLanguageServices>, state_path: PathBuf) -> RecoveryCoordinator {
    RecoveryCoordinator::new(services, state_path, "RepoDesk".into(), 100)
}

#[test]
fn snapshot_is_lazy_and_first_check_persists_missing_language_record() {
    let temp = tempfile::tempdir().unwrap();
    let state_path = temp.path().join("recovery-state.json");
    let services = Arc::new(FakeLanguageServices::new(
        LanguageServerAvailability::Missing,
    ));
    let first = coordinator(services.clone(), state_path.clone());
    assert!(first.snapshot().records.is_empty());

    let emitted = Mutex::new(Vec::<RecoveryRecord>::new());
    let record = first
        .check(CAPABILITY_ID, &|record| {
            emitted.lock().unwrap().push(record.clone())
        })
        .unwrap();
    assert_eq!(record.state, RecoveryState::NeedsApproval);
    assert_eq!(emitted.lock().unwrap().len(), 1);

    drop(first);
    let reloaded = coordinator(services, state_path);
    assert_eq!(
        reloaded.snapshot().records[0].state,
        RecoveryState::NeedsApproval
    );
}

#[test]
fn confirmation_survives_same_diagnosis_but_rejects_changed_diagnosis() {
    let temp = tempfile::tempdir().unwrap();
    let services = Arc::new(FakeLanguageServices::new(
        LanguageServerAvailability::Missing,
    ));
    let coordinator = coordinator(services.clone(), temp.path().join("state.json"));
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let stable_preview = coordinator
        .repair_preview(CAPABILITY_ID, INSTALL_ACTION)
        .unwrap();
    assert_ne!(stable_preview.confirmation_token, "adapter_secret_token");
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    coordinator
        .confirm(&stable_preview.confirmation_token, &|_| {})
        .unwrap();

    services.set_runtime(repodesk_core::recovery::LanguageRuntimeState::NotStarted);
    services.set_availability(LanguageServerAvailability::Missing);
    *services.install.lock().unwrap() = None;
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let stale_preview = coordinator
        .repair_preview(CAPABILITY_ID, INSTALL_ACTION)
        .unwrap();
    services.set_availability(LanguageServerAvailability::Available);
    services.set_runtime(repodesk_core::recovery::LanguageRuntimeState::Error);
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let error = coordinator
        .confirm(&stale_preview.confirmation_token, &|_| {})
        .unwrap_err();
    assert!(error.contains("diagnosis changed"));
}

#[test]
fn installer_output_never_crosses_record_boundary_and_live_ready_verifies_repair() {
    let temp = tempfile::tempdir().unwrap();
    let services = Arc::new(FakeLanguageServices::new(
        LanguageServerAvailability::Missing,
    ));
    let coordinator = coordinator(services, temp.path().join("state.json"));
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let preview = coordinator
        .repair_preview(CAPABILITY_ID, INSTALL_ACTION)
        .unwrap();
    let emitted = Mutex::new(Vec::<RecoveryRecord>::new());
    let record = coordinator
        .confirm(&preview.confirmation_token, &|record| {
            emitted.lock().unwrap().push(record.clone())
        })
        .unwrap();

    assert_eq!(record.state, RecoveryState::Healthy);
    assert!(
        emitted
            .lock()
            .unwrap()
            .iter()
            .any(|record| { record.state == RecoveryState::Repairing })
    );
    let serialized = serde_json::to_string(&emitted.into_inner().unwrap()).unwrap();
    assert!(!serialized.contains("must-not-cross-boundary"));
    assert!(!serialized.contains("adapter_secret_token"));
}

#[test]
fn automatic_restart_runs_twice_per_diagnosis_then_stays_blocked() {
    let temp = tempfile::tempdir().unwrap();
    let services = Arc::new(FakeLanguageServices::new(
        LanguageServerAvailability::Available,
    ));
    services.set_runtime(repodesk_core::recovery::LanguageRuntimeState::Error);
    services
        .restart_becomes_ready
        .store(false, Ordering::Relaxed);
    let coordinator = coordinator(services.clone(), temp.path().join("state.json"));

    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let third = coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();

    assert_eq!(services.restart_calls.load(Ordering::Relaxed), 2);
    assert_eq!(third.state, RecoveryState::Degraded);
    assert_eq!(third.automatic_attempts, 2);
    assert!(
        third
            .actions
            .iter()
            .all(|action| action.kind != RecoveryActionKind::Automatic)
    );
}

#[test]
fn installer_transport_error_finishes_attempt_instead_of_staying_repairing() {
    let temp = tempfile::tempdir().unwrap();
    let services = Arc::new(FakeLanguageServices::new(
        LanguageServerAvailability::Missing,
    ));
    services
        .install_transport_error
        .store(true, Ordering::Relaxed);
    let coordinator = coordinator(services, temp.path().join("state.json"));
    coordinator.check(CAPABILITY_ID, &|_| {}).unwrap();
    let preview = coordinator
        .repair_preview(CAPABILITY_ID, INSTALL_ACTION)
        .unwrap();

    let record = coordinator
        .confirm(&preview.confirmation_token, &|_| {})
        .unwrap();

    assert_eq!(record.state, RecoveryState::Degraded);
    let history = coordinator.history();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].verification_summary.as_deref(),
        Some("installer process could not start")
    );
}
