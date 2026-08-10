use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use chrono::{DateTime, Duration, Utc};
use repodesk_core::language_intelligence::{
    LanguageIntelligenceSnapshot, LanguageServerDescriptor,
};
use repodesk_core::language_tools::{
    LanguageToolInstallPreview, LanguageToolInstallResult, LanguageToolInstallState,
    LanguageToolInstallStatus,
};
use repodesk_core::paths::RepoDeskPaths;
use repodesk_core::recovery::{
    LanguageRecoveryInput, LanguageRuntimeState, ObserveOutcome, RecoveryAttempt, RecoveryEngine,
    RecoveryRecord, RecoveryRepairPreview, RecoveryRisk, RecoverySnapshot, RecoveryStore,
    RepairCompletion, language_observation, sanitize_recovery_text,
};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use super::language_intelligence::{
    recovery_language_server_restart, recovery_language_server_statuses,
};
use super::language_tools::LANGUAGE_TOOL_INSTALLER;

const RECOVERY_EVENT: &str = "recovery-record-changed";
const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageRuntimeStatus {
    pub project: String,
    pub server_id: String,
    pub state: LanguageRuntimeState,
    pub last_error: Option<String>,
}

pub trait RecoveryLanguageServices: Send + Sync {
    fn discovery(&self) -> Result<LanguageIntelligenceSnapshot, String>;
    fn statuses(&self) -> Vec<LanguageRuntimeStatus>;
    fn install_status(&self, recipe_id: &str) -> Result<Option<LanguageToolInstallStatus>, String>;
    fn preview(&self, recipe_id: &str) -> Result<LanguageToolInstallPreview, String>;
    fn install_observed(
        &self,
        token: &str,
        observer: &dyn Fn(&LanguageToolInstallStatus),
    ) -> Result<LanguageToolInstallResult, String>;
    fn cancel(&self, recipe_id: &str) -> Result<bool, String>;
    fn restart(&self, server_id: &str) -> Result<(), String>;
}

struct PendingRecoveryRepair {
    capability_id: String,
    diagnosis_revision: String,
    action_id: String,
    adapter_confirmation_token: String,
    expires_at: DateTime<Utc>,
}

pub struct RecoveryCoordinator {
    services: Arc<dyn RecoveryLanguageServices>,
    state_path: PathBuf,
    engine: Mutex<RecoveryEngine>,
    pending: Mutex<HashMap<String, PendingRecoveryRepair>>,
    generation: AtomicU64,
    sequence: AtomicU64,
    warnings: Mutex<Vec<String>>,
}

impl RecoveryCoordinator {
    pub fn new(
        services: Arc<dyn RecoveryLanguageServices>,
        state_path: PathBuf,
        project: String,
        history_limit: usize,
    ) -> Self {
        let (mut engine, warnings) =
            match RecoveryStore::load(&state_path, project.clone(), history_limit) {
                Ok(engine) => (engine, Vec::new()),
                Err(error) => (
                    RecoveryEngine::new(project, history_limit),
                    vec![sanitize_recovery_text(&format!(
                        "Recovery history is unavailable: {error}"
                    ))],
                ),
            };
        engine.prune_history(Utc::now() - Duration::days(30));
        let generation = engine
            .snapshot()
            .records
            .iter()
            .map(|record| record.generation)
            .max()
            .unwrap_or(0);
        Self {
            services,
            state_path,
            engine: Mutex::new(engine),
            pending: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(generation),
            sequence: AtomicU64::new(0),
            warnings: Mutex::new(warnings),
        }
    }

    pub fn project(&self) -> String {
        self.engine
            .lock()
            .map(|engine| engine.snapshot().project)
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> RecoverySnapshot {
        let mut snapshot = self
            .engine
            .lock()
            .map(|engine| engine.snapshot())
            .unwrap_or_else(|_| RecoverySnapshot {
                project: String::new(),
                records: Vec::new(),
                actionable_count: 0,
                warnings: vec!["Recovery state is temporarily unavailable".into()],
                generated_at: Utc::now(),
            });
        if let Ok(warnings) = self.warnings.lock() {
            snapshot.warnings.extend(warnings.iter().cloned());
        }
        snapshot
    }

    pub fn history(&self) -> Vec<RecoveryAttempt> {
        self.engine
            .lock()
            .map(|engine| engine.history())
            .unwrap_or_default()
    }

    pub fn check(
        &self,
        capability_id: &str,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let (snapshot, descriptor) = self.discovery_for_capability(capability_id)?;
        let runtime = self.runtime_for(&snapshot.project, &descriptor.id);
        let install = descriptor
            .install_recipe_id
            .as_deref()
            .map(|recipe_id| self.services.install_status(recipe_id))
            .transpose()?
            .flatten();
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let observation = language_observation(LanguageRecoveryInput {
            project: &snapshot.project,
            generation,
            descriptor: &descriptor,
            runtime: runtime.state,
            runtime_error: runtime.last_error.as_deref(),
            install: install.as_ref(),
            observed_at: Utc::now(),
        })
        .ok_or_else(|| format!("Capability '{capability_id}' is discovery-only"))?;
        let record = self.observe(observation, emit)?;
        let automatic = record
            .actions
            .iter()
            .find(|action| action.kind == repodesk_core::recovery::RecoveryActionKind::Automatic)
            .map(|action| action.id.clone());
        match automatic {
            Some(action_id) => self.run_automatic_repair(&snapshot, &descriptor, &action_id, emit),
            None => Ok(record),
        }
    }

    pub fn repair_preview(
        &self,
        capability_id: &str,
        action_id: &str,
    ) -> Result<RecoveryRepairPreview, String> {
        let record = self.record(capability_id)?;
        let action = record
            .actions
            .iter()
            .find(|action| action.id == action_id)
            .ok_or_else(|| format!("Action '{action_id}' is not available"))?;
        if action.kind != repodesk_core::recovery::RecoveryActionKind::Confirmable {
            return Err(format!("Action '{action_id}' does not use confirmation"));
        }
        let recipe_id = action
            .recipe_id
            .as_deref()
            .ok_or_else(|| format!("Action '{action_id}' has no allowlisted recipe"))?;
        let adapter = self.services.preview(recipe_id)?;
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let token = recovery_confirmation_token(
            &self.project(),
            capability_id,
            &record.diagnosis_revision,
            action_id,
            &adapter.recipe_revision,
            adapter.expires_at,
            sequence,
        );
        self.pending
            .lock()
            .map_err(|_| "Recovery confirmation registry is unavailable".to_string())?
            .insert(
                token.clone(),
                PendingRecoveryRepair {
                    capability_id: capability_id.into(),
                    diagnosis_revision: record.diagnosis_revision.clone(),
                    action_id: action_id.into(),
                    adapter_confirmation_token: adapter.confirmation_token.clone(),
                    expires_at: adapter.expires_at,
                },
            );
        let mut changes = vec![format!("Install {}@{}", adapter.package, adapter.version)];
        changes.extend(
            adapter
                .writes_outside_repository
                .iter()
                .map(|path| format!("Write RepoDesk-managed files under {path}")),
        );
        Ok(RecoveryRepairPreview {
            capability_id: capability_id.into(),
            diagnosis_revision: record.diagnosis_revision,
            action_id: action_id.into(),
            title: format!("Install {}", adapter.server_label),
            summary:
                "Install an allowlisted managed language tool without changing the repository."
                    .into(),
            risk: if adapter.network_required {
                RecoveryRisk::Moderate
            } else {
                RecoveryRisk::Low
            },
            recipe_id: adapter.recipe_id,
            recipe_revision: adapter.recipe_revision,
            changes,
            network_required: adapter.network_required,
            verification: format!(
                "Run the version probe, then initialize {} through LSP",
                adapter.server_label
            ),
            confirmation_token: token,
            expires_at: adapter.expires_at,
        })
    }

    pub fn confirm(
        &self,
        confirmation_token: &str,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| "Recovery confirmation registry is unavailable".to_string())?
            .remove(confirmation_token)
            .ok_or_else(|| "Recovery confirmation token is invalid or already used".to_string())?;
        if Utc::now() > pending.expires_at {
            return Err("Recovery confirmation token expired; review the repair again".into());
        }
        let current = self.record(&pending.capability_id)?;
        if current.diagnosis_revision != pending.diagnosis_revision {
            return Err("Recovery diagnosis changed; review the repair again".into());
        }
        if !current
            .actions
            .iter()
            .any(|action| action.id == pending.action_id)
        {
            return Err("Recovery action is no longer available".into());
        }

        let repairing = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| "Recovery state is unavailable".to_string())?;
            engine
                .begin_repair(&pending.capability_id, &pending.action_id, Utc::now())
                .map_err(|error| error.to_string())?
        };
        self.persist();
        emit(&repairing);

        let (snapshot, descriptor) = self.discovery_for_capability(&pending.capability_id)?;
        let result =
            match self
                .services
                .install_observed(&pending.adapter_confirmation_token, &|status| {
                    let _ = self.observe_install_progress(&snapshot, &descriptor, status, emit);
                }) {
                Ok(result) => result,
                Err(error) => {
                    return self.finish(
                        &pending.capability_id,
                        RepairCompletion::Failed {
                            finished_at: Utc::now(),
                            summary: error,
                        },
                        emit,
                    );
                }
            };

        if result.status.state != LanguageToolInstallState::Ready {
            let completion = if result.status.state == LanguageToolInstallState::Cancelled {
                RepairCompletion::Cancelled {
                    finished_at: Utc::now(),
                    summary: result.status.message,
                }
            } else {
                RepairCompletion::Failed {
                    finished_at: Utc::now(),
                    summary: result.status.error.unwrap_or(result.status.message),
                }
            };
            return self.finish(&pending.capability_id, completion, emit);
        }

        if let Err(error) = self.services.restart(&descriptor.id) {
            return self.finish(
                &pending.capability_id,
                RepairCompletion::VerificationFailed {
                    finished_at: Utc::now(),
                    summary: error,
                },
                emit,
            );
        }
        let runtime = self.runtime_for(&snapshot.project, &descriptor.id);
        if runtime.state != LanguageRuntimeState::Ready {
            return self.finish(
                &pending.capability_id,
                RepairCompletion::VerificationFailed {
                    finished_at: Utc::now(),
                    summary: runtime
                        .last_error
                        .unwrap_or_else(|| "Language server did not become ready".into()),
                },
                emit,
            );
        }
        let install = Some(&result.status);
        if let Some(observation) = language_observation(LanguageRecoveryInput {
            project: &snapshot.project,
            generation: self.generation.fetch_add(1, Ordering::Relaxed) + 1,
            descriptor: &descriptor,
            runtime: LanguageRuntimeState::Ready,
            runtime_error: None,
            install,
            observed_at: Utc::now(),
        }) {
            let _ = self.observe(observation, emit)?;
        }
        self.finish(
            &pending.capability_id,
            RepairCompletion::Verified {
                finished_at: Utc::now(),
                summary: "Language server completed live protocol initialization".into(),
            },
            emit,
        )
    }

    pub fn cancel(&self, recipe_id: &str) -> Result<bool, String> {
        self.services.cancel(recipe_id)
    }

    fn run_automatic_repair(
        &self,
        snapshot: &LanguageIntelligenceSnapshot,
        descriptor: &LanguageServerDescriptor,
        action_id: &str,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let capability_id = format!("language:{}", descriptor.id);
        let repairing = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| "Recovery state is unavailable".to_string())?;
            engine
                .begin_repair(&capability_id, action_id, Utc::now())
                .map_err(|error| error.to_string())?
        };
        self.persist();
        emit(&repairing);
        if let Err(error) = self.services.restart(&descriptor.id) {
            return self.finish(
                &capability_id,
                RepairCompletion::Failed {
                    finished_at: Utc::now(),
                    summary: error,
                },
                emit,
            );
        }
        let runtime = self.runtime_for(&snapshot.project, &descriptor.id);
        if runtime.state == LanguageRuntimeState::Ready {
            if let Some(observation) = language_observation(LanguageRecoveryInput {
                project: &snapshot.project,
                generation: self.generation.fetch_add(1, Ordering::Relaxed) + 1,
                descriptor,
                runtime: LanguageRuntimeState::Ready,
                runtime_error: None,
                install: None,
                observed_at: Utc::now(),
            }) {
                let _ = self.observe(observation, emit)?;
            }
            self.finish(
                &capability_id,
                RepairCompletion::Verified {
                    finished_at: Utc::now(),
                    summary: "Language server restarted and initialized".into(),
                },
                emit,
            )
        } else {
            self.finish(
                &capability_id,
                RepairCompletion::Failed {
                    finished_at: Utc::now(),
                    summary: runtime
                        .last_error
                        .unwrap_or_else(|| "Language server restart did not become ready".into()),
                },
                emit,
            )
        }
    }

    fn observe_install_progress(
        &self,
        snapshot: &LanguageIntelligenceSnapshot,
        descriptor: &LanguageServerDescriptor,
        status: &LanguageToolInstallStatus,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let observation = language_observation(LanguageRecoveryInput {
            project: &snapshot.project,
            generation: self.generation.fetch_add(1, Ordering::Relaxed) + 1,
            descriptor,
            runtime: LanguageRuntimeState::Starting,
            runtime_error: None,
            install: Some(status),
            observed_at: Utc::now(),
        })
        .ok_or_else(|| "Language recovery profile is not active".to_string())?;
        self.observe(observation, emit)
    }

    fn observe(
        &self,
        observation: repodesk_core::recovery::RecoveryObservation,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let record = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| "Recovery state is unavailable".to_string())?;
            match engine.observe(observation) {
                ObserveOutcome::Applied(record) => *record,
                ObserveOutcome::IgnoredStale => {
                    return Err("Stale recovery observation was ignored".into());
                }
            }
        };
        self.persist();
        emit(&record);
        Ok(record)
    }

    fn finish(
        &self,
        capability_id: &str,
        completion: RepairCompletion,
        emit: &dyn Fn(&RecoveryRecord),
    ) -> Result<RecoveryRecord, String> {
        let record = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| "Recovery state is unavailable".to_string())?;
            engine
                .finish_repair(capability_id, completion)
                .map_err(|error| error.to_string())?
        };
        self.persist();
        emit(&record);
        Ok(record)
    }

    fn record(&self, capability_id: &str) -> Result<RecoveryRecord, String> {
        self.snapshot()
            .records
            .into_iter()
            .find(|record| record.capability_id == capability_id)
            .ok_or_else(|| format!("Recovery capability '{capability_id}' was not checked"))
    }

    fn discovery_for_capability(
        &self,
        capability_id: &str,
    ) -> Result<(LanguageIntelligenceSnapshot, LanguageServerDescriptor), String> {
        let server_id = capability_id
            .strip_prefix("language:")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("Unsupported recovery capability '{capability_id}'"))?;
        let snapshot = self.services.discovery()?;
        if snapshot.project != self.project() {
            return Err("Active project changed; reopen IDE Health".into());
        }
        let descriptor = snapshot
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
            .ok_or_else(|| format!("Language server '{server_id}' is not registered"))?;
        Ok((snapshot, descriptor))
    }

    fn runtime_for(&self, project: &str, server_id: &str) -> LanguageRuntimeStatus {
        self.services
            .statuses()
            .into_iter()
            .find(|status| status.project == project && status.server_id == server_id)
            .unwrap_or(LanguageRuntimeStatus {
                project: project.into(),
                server_id: server_id.into(),
                state: LanguageRuntimeState::NotStarted,
                last_error: None,
            })
    }

    fn persist(&self) {
        let result = self
            .engine
            .lock()
            .map_err(|_| "Recovery state is unavailable".to_string())
            .and_then(|engine| {
                RecoveryStore::save(&self.state_path, &engine).map_err(|error| error.to_string())
            });
        if let Err(error) = result
            && let Ok(mut warnings) = self.warnings.lock()
        {
            let warning =
                sanitize_recovery_text(&format!("Recovery history could not be saved: {error}"));
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
    }
}

fn recovery_confirmation_token(
    project: &str,
    capability_id: &str,
    diagnosis_revision: &str,
    action_id: &str,
    recipe_revision: &str,
    expires_at: DateTime<Utc>,
    sequence: u64,
) -> String {
    let mut hasher = Sha256::new();
    for value in [
        project,
        capability_id,
        diagnosis_revision,
        action_id,
        recipe_revision,
    ] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update(expires_at.timestamp_millis().to_le_bytes());
    hasher.update(sequence.to_le_bytes());
    let digest = hasher.finalize();
    let encoded = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("recovery_confirm_{encoded}")
}

struct TauriLanguageServices {
    app: AppHandle,
}

impl RecoveryLanguageServices for TauriLanguageServices {
    fn discovery(&self) -> Result<LanguageIntelligenceSnapshot, String> {
        repodesk_core::language_intelligence::active_language_intelligence_snapshot()
            .map_err(|error| error.to_string())
    }

    fn statuses(&self) -> Vec<LanguageRuntimeStatus> {
        recovery_language_server_statuses()
    }

    fn install_status(&self, recipe_id: &str) -> Result<Option<LanguageToolInstallStatus>, String> {
        LANGUAGE_TOOL_INSTALLER
            .status(recipe_id)
            .map_err(|error| error.to_string())
    }

    fn preview(&self, recipe_id: &str) -> Result<LanguageToolInstallPreview, String> {
        LANGUAGE_TOOL_INSTALLER
            .preview(recipe_id)
            .map_err(|error| error.to_string())
    }

    fn install_observed(
        &self,
        token: &str,
        observer: &dyn Fn(&LanguageToolInstallStatus),
    ) -> Result<LanguageToolInstallResult, String> {
        LANGUAGE_TOOL_INSTALLER
            .install_with_observer(token, observer)
            .map_err(|error| error.to_string())
    }

    fn cancel(&self, recipe_id: &str) -> Result<bool, String> {
        LANGUAGE_TOOL_INSTALLER
            .cancel(recipe_id)
            .map_err(|error| error.to_string())
    }

    fn restart(&self, server_id: &str) -> Result<(), String> {
        recovery_language_server_restart(&self.app, server_id)
    }
}

static ACTIVE_RECOVERY: LazyLock<Mutex<Option<Arc<RecoveryCoordinator>>>> =
    LazyLock::new(|| Mutex::new(None));

fn active_coordinator(app: &AppHandle) -> Result<Arc<RecoveryCoordinator>, String> {
    let services = Arc::new(TauriLanguageServices { app: app.clone() });
    let discovery = services.discovery()?;
    let mut active = ACTIVE_RECOVERY
        .lock()
        .map_err(|_| "Recovery coordinator registry is unavailable".to_string())?;
    if let Some(coordinator) = active.as_ref()
        && coordinator.project() == discovery.project
    {
        return Ok(coordinator.clone());
    }
    let paths = RepoDeskPaths::resolve().map_err(|error| error.to_string())?;
    let project_file = safe_project_file(&discovery.project);
    let state_path = paths
        .home
        .join("recovery")
        .join(format!("{project_file}.json"));
    let coordinator = Arc::new(RecoveryCoordinator::new(
        services,
        state_path,
        discovery.project,
        HISTORY_LIMIT,
    ));
    *active = Some(coordinator.clone());
    Ok(coordinator)
}

fn safe_project_file(project: &str) -> String {
    project
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn emit_record(app: &AppHandle, record: &RecoveryRecord) {
    let _ = app.emit(RECOVERY_EVENT, record.clone());
}

#[tauri::command]
pub fn recovery_snapshot(app: AppHandle) -> Result<RecoverySnapshot, String> {
    Ok(active_coordinator(&app)?.snapshot())
}

#[tauri::command]
pub fn recovery_check(app: AppHandle, capability_id: String) -> Result<RecoveryRecord, String> {
    active_coordinator(&app)?.check(&capability_id, &|record| emit_record(&app, record))
}

#[tauri::command]
pub fn recovery_repair_preview(
    app: AppHandle,
    capability_id: String,
    action_id: String,
) -> Result<RecoveryRepairPreview, String> {
    active_coordinator(&app)?.repair_preview(&capability_id, &action_id)
}

#[tauri::command]
pub async fn recovery_repair_confirm(
    app: AppHandle,
    confirmation_token: String,
) -> Result<RecoveryRecord, String> {
    let coordinator = active_coordinator(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        coordinator.confirm(&confirmation_token, &|record| emit_record(&app, record))
    })
    .await
    .map_err(|error| format!("Recovery worker failed: {error}"))?
}

#[tauri::command]
pub fn recovery_repair_cancel(app: AppHandle, recipe_id: String) -> Result<bool, String> {
    active_coordinator(&app)?.cancel(&recipe_id)
}

#[tauri::command]
pub fn recovery_history(app: AppHandle) -> Result<Vec<RecoveryAttempt>, String> {
    Ok(active_coordinator(&app)?.history())
}
