//! Safe, RepoDesk-owned installation of first-wave language servers.
//!
//! The frontend may choose only a compiled-in `recipe_id`. It never supplies a
//! package name, URL, executable, shell string, destination, or arbitrary argv.
//! Every preview is bound to one exact recipe revision and is single-use.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::paths::RepoDeskPaths;
use crate::projects::get_active_project;

const CONFIRMATION_TTL_MINUTES: i64 = 5;
const MAX_INSTALL_OUTPUT_CHARS: usize = 12_000;
const TYPESCRIPT_LANGUAGE_SERVER_VERSION: &str = "5.3.0";
const TYPESCRIPT_VERSION: &str = "7.0.2";
const VSCODE_LANGSERVERS_VERSION: &str = "4.10.0";
const YAML_LANGUAGE_SERVER_VERSION: &str = "1.24.0";
const TAPLO_VERSION: &str = "0.10.0";

static PROCESS_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageToolInstaller {
    Npm,
    Cargo,
    Rustup,
}

impl LanguageToolInstaller {
    fn executable(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Cargo => "cargo",
            Self::Rustup => "rustup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallLayout {
    ManagedDirectory,
    ExternalToolchain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolInstallPreview {
    pub recipe_id: String,
    pub recipe_revision: String,
    pub server_id: String,
    pub server_label: String,
    pub languages: Vec<String>,
    pub installer: LanguageToolInstaller,
    pub package: String,
    pub version: String,
    pub destination: String,
    pub install_command: LanguageToolCommand,
    pub probe_command: LanguageToolCommand,
    pub network_required: bool,
    pub writes_outside_repository: Vec<String>,
    pub prerequisite_available: bool,
    pub prerequisite_hint: Option<String>,
    pub confirmation_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageToolInstallState {
    Installing,
    Ready,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolInstallStatus {
    pub recipe_id: String,
    pub state: LanguageToolInstallState,
    pub progress: u8,
    pub message: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolInstallResult {
    pub status: LanguageToolInstallStatus,
    pub executable: Option<String>,
    pub output: String,
}

#[derive(Clone)]
struct InstallRecipe {
    id: &'static str,
    server_id: &'static str,
    server_label: &'static str,
    languages: &'static [&'static str],
    installer: LanguageToolInstaller,
    package: &'static str,
    version: &'static str,
    companion_package: Option<(&'static str, &'static str)>,
    executable: &'static str,
    probe_args: &'static [&'static str],
    layout: InstallLayout,
}

const RECIPES: &[InstallRecipe] = &[
    InstallRecipe {
        id: "rust-analyzer",
        server_id: "rust-analyzer",
        server_label: "rust-analyzer",
        languages: &["rust"],
        installer: LanguageToolInstaller::Rustup,
        package: "rust-analyzer",
        version: "active-toolchain",
        companion_package: None,
        executable: "rust-analyzer",
        probe_args: &["--version"],
        layout: InstallLayout::ExternalToolchain,
    },
    InstallRecipe {
        id: "typescript-language-server",
        server_id: "typescript-language-server",
        server_label: "TypeScript Language Server",
        languages: &["typescript", "javascript"],
        installer: LanguageToolInstaller::Npm,
        package: "typescript-language-server",
        version: TYPESCRIPT_LANGUAGE_SERVER_VERSION,
        companion_package: Some(("typescript", TYPESCRIPT_VERSION)),
        executable: "typescript-language-server",
        probe_args: &["--version"],
        layout: InstallLayout::ManagedDirectory,
    },
    InstallRecipe {
        id: "json-language-server",
        server_id: "json-language-server",
        server_label: "JSON Language Server",
        languages: &["json"],
        installer: LanguageToolInstaller::Npm,
        package: "vscode-langservers-extracted",
        version: VSCODE_LANGSERVERS_VERSION,
        companion_package: None,
        executable: "vscode-json-language-server",
        probe_args: &["--version"],
        layout: InstallLayout::ManagedDirectory,
    },
    InstallRecipe {
        id: "yaml-language-server",
        server_id: "yaml-language-server",
        server_label: "YAML Language Server",
        languages: &["yaml"],
        installer: LanguageToolInstaller::Npm,
        package: "yaml-language-server",
        version: YAML_LANGUAGE_SERVER_VERSION,
        companion_package: None,
        executable: "yaml-language-server",
        probe_args: &["--version"],
        layout: InstallLayout::ManagedDirectory,
    },
    InstallRecipe {
        id: "taplo",
        server_id: "taplo",
        server_label: "Taplo",
        languages: &["toml"],
        installer: LanguageToolInstaller::Cargo,
        package: "taplo-cli",
        version: TAPLO_VERSION,
        companion_package: None,
        executable: "taplo",
        probe_args: &["--version"],
        layout: InstallLayout::ManagedDirectory,
    },
];

#[derive(Debug, Clone)]
struct PendingInstall {
    preview: LanguageToolInstallPreview,
    project: String,
    project_root: PathBuf,
    staging_dir: Option<PathBuf>,
    destination_dir: Option<PathBuf>,
    expected_executable: PathBuf,
    work_dir: PathBuf,
    layout: InstallLayout,
}

#[derive(Debug)]
pub struct CommandOutcome {
    pub success: bool,
    pub cancelled: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait LanguageToolCommandRunner: Send + Sync {
    fn run(
        &self,
        command: &LanguageToolCommand,
        cwd: &Path,
        cancel: &AtomicBool,
    ) -> RepoDeskResult<CommandOutcome>;
}

#[derive(Default)]
pub struct SystemLanguageToolCommandRunner;

impl LanguageToolCommandRunner for SystemLanguageToolCommandRunner {
    fn run(
        &self,
        command: &LanguageToolCommand,
        cwd: &Path,
        cancel: &AtomicBool,
    ) -> RepoDeskResult<CommandOutcome> {
        fs::create_dir_all(cwd)?;
        let sequence = PROCESS_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stem = format!(".language-tool-{}-{sequence}", std::process::id());
        let stdout_path = cwd.join(format!("{stem}.stdout"));
        let stderr_path = cwd.join(format!("{stem}.stderr"));
        let stdout_file = File::create(&stdout_path)?;
        let stderr_file = File::create(&stderr_path)?;

        let mut child = Command::new(&command.program)
            .args(&command.args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|error| {
                RepoDeskError::Api(format!(
                    "Failed to start language-tool installer '{}': {error}",
                    command.program
                ))
            })?;

        let (success, cancelled) = loop {
            if cancel.load(Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                break (false, true);
            }
            if let Some(status) = child.try_wait()? {
                break (status.success(), false);
            }
            thread::sleep(StdDuration::from_millis(100));
        };

        let stdout = read_bounded_text(&stdout_path);
        let stderr = read_bounded_text(&stderr_path);
        let _ = fs::remove_file(stdout_path);
        let _ = fs::remove_file(stderr_path);

        Ok(CommandOutcome {
            success,
            cancelled,
            stdout,
            stderr,
        })
    }
}

pub struct LanguageToolInstallService {
    pending: Mutex<HashMap<String, PendingInstall>>,
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
    statuses: Mutex<HashMap<String, LanguageToolInstallStatus>>,
    sequence: AtomicU64,
    runner: Arc<dyn LanguageToolCommandRunner>,
}

impl Default for LanguageToolInstallService {
    fn default() -> Self {
        Self::new(Arc::new(SystemLanguageToolCommandRunner))
    }
}

impl LanguageToolInstallService {
    pub fn new(runner: Arc<dyn LanguageToolCommandRunner>) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            running: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
            sequence: AtomicU64::new(0),
            runner,
        }
    }

    pub fn preview(&self, recipe_id: &str) -> RepoDeskResult<LanguageToolInstallPreview> {
        let project = get_active_project()?;
        let paths = RepoDeskPaths::resolve()?;
        self.preview_for_project(recipe_id, &project.name, &project.path, &paths.home, Utc::now())
    }

    pub fn preview_for_project(
        &self,
        recipe_id: &str,
        project: &str,
        project_root: &Path,
        repodesk_home: &Path,
        now: DateTime<Utc>,
    ) -> RepoDeskResult<LanguageToolInstallPreview> {
        let recipe = recipe(recipe_id)?;
        if project.trim().is_empty() {
            return Err(RepoDeskError::Api(
                "Language-tool installation requires an active project".into(),
            ));
        }

        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let revision = recipe_revision(recipe);
        let tools_root = repodesk_home.join("tools").join("language-servers");
        let destination_dir = (recipe.layout == InstallLayout::ManagedDirectory)
            .then(|| tools_root.join(recipe.id));
        let staging_dir = (recipe.layout == InstallLayout::ManagedDirectory)
            .then(|| tools_root.join(".staging").join(format!("{}-{sequence}", recipe.id)));
        let expected_executable = match recipe.layout {
            InstallLayout::ManagedDirectory => executable_in_install_root(
                staging_dir.as_deref().expect("managed recipe has staging"),
                recipe,
            ),
            InstallLayout::ExternalToolchain => PathBuf::from(recipe.executable),
        };
        let install_command = install_command(recipe, staging_dir.as_deref(), repodesk_home)?;
        let probe_command = LanguageToolCommand {
            program: expected_executable.to_string_lossy().into_owned(),
            args: recipe.probe_args.iter().map(|arg| (*arg).to_string()).collect(),
        };
        let expires_at = now + Duration::minutes(CONFIRMATION_TTL_MINUTES);
        let token = confirmation_token(
            project,
            recipe.id,
            &revision,
            destination_dir.as_deref(),
            &install_command,
            now,
            sequence,
        );
        let prerequisite_available = executable_on_path(recipe.installer.executable());

        let preview = LanguageToolInstallPreview {
            recipe_id: recipe.id.into(),
            recipe_revision: revision,
            server_id: recipe.server_id.into(),
            server_label: recipe.server_label.into(),
            languages: recipe.languages.iter().map(|value| (*value).into()).collect(),
            installer: recipe.installer,
            package: recipe.package.into(),
            version: recipe.version.into(),
            destination: destination_dir
                .as_deref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| "active rustup toolchain".into()),
            install_command,
            probe_command,
            network_required: true,
            writes_outside_repository: vec![match destination_dir.as_deref() {
                Some(path) => path.to_string_lossy().into_owned(),
                None => "rustup active toolchain component registry".into(),
            }],
            prerequisite_available,
            prerequisite_hint: (!prerequisite_available).then(|| {
                format!(
                    "Install '{}' and make it available on PATH before continuing.",
                    recipe.installer.executable()
                )
            }),
            confirmation_token: token.clone(),
            expires_at,
        };

        let pending = PendingInstall {
            preview: preview.clone(),
            project: project.to_string(),
            project_root: project_root.to_path_buf(),
            staging_dir,
            destination_dir,
            expected_executable,
            work_dir: tools_root,
            layout: recipe.layout,
        };
        self.pending
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool preview registry is unavailable".into()))?
            .insert(token, pending);

        Ok(preview)
    }

    pub fn install(&self, confirmation_token: &str) -> RepoDeskResult<LanguageToolInstallResult> {
        self.install_at(confirmation_token, Utc::now())
    }

    pub fn install_at(
        &self,
        confirmation_token: &str,
        now: DateTime<Utc>,
    ) -> RepoDeskResult<LanguageToolInstallResult> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool preview registry is unavailable".into()))?
            .remove(confirmation_token)
            .ok_or_else(|| {
                RepoDeskError::Api(
                    "Language-tool confirmation token is invalid or already used".into(),
                )
            })?;

        if now > pending.preview.expires_at {
            return Err(RepoDeskError::Api(
                "Language-tool confirmation token expired; preview the install again".into(),
            ));
        }
        let current_recipe = recipe(&pending.preview.recipe_id)?;
        if recipe_revision(current_recipe) != pending.preview.recipe_revision {
            return Err(RepoDeskError::Api(
                "Language-tool recipe changed; preview the install again".into(),
            ));
        }
        if !pending.preview.prerequisite_available {
            return Err(RepoDeskError::Api(
                pending
                    .preview
                    .prerequisite_hint
                    .clone()
                    .unwrap_or_else(|| "Required installer is unavailable".into()),
            ));
        }
        if self
            .running
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool runtime registry is unavailable".into()))?
            .contains_key(&pending.preview.recipe_id)
        {
            return Err(RepoDeskError::Api(format!(
                "{} is already being installed",
                pending.preview.server_label
            )));
        }

        let cancel = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool runtime registry is unavailable".into()))?
            .insert(pending.preview.recipe_id.clone(), cancel.clone());

        self.set_status(
            &pending.preview.recipe_id,
            LanguageToolInstallState::Installing,
            10,
            "Preparing isolated installation",
            now,
            None,
            None,
        )?;

        let result = self.execute_pending(&pending, &cancel, now);
        if let Ok(mut running) = self.running.lock() {
            running.remove(&pending.preview.recipe_id);
        }
        result
    }

    pub fn cancel(&self, recipe_id: &str) -> RepoDeskResult<bool> {
        let running = self
            .running
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool runtime registry is unavailable".into()))?;
        let Some(flag) = running.get(recipe_id) else {
            return Ok(false);
        };
        flag.store(true, Ordering::Release);
        Ok(true)
    }

    pub fn status(&self, recipe_id: &str) -> RepoDeskResult<Option<LanguageToolInstallStatus>> {
        Ok(self
            .statuses
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool status registry is unavailable".into()))?
            .get(recipe_id)
            .cloned())
    }

    fn execute_pending(
        &self,
        pending: &PendingInstall,
        cancel: &AtomicBool,
        started_at: DateTime<Utc>,
    ) -> RepoDeskResult<LanguageToolInstallResult> {
        if let Some(staging) = &pending.staging_dir {
            if staging.exists() {
                fs::remove_dir_all(staging)?;
            }
            if let Some(parent) = staging.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        let work_dir = pending.work_dir.clone();
        fs::create_dir_all(&work_dir)?;

        self.set_status(
            &pending.preview.recipe_id,
            LanguageToolInstallState::Installing,
            30,
            "Running allowlisted installer",
            started_at,
            None,
            None,
        )?;
        let install_outcome = self
            .runner
            .run(&pending.preview.install_command, &work_dir, cancel)?;
        let mut output = merged_output(&install_outcome);

        if install_outcome.cancelled || cancel.load(Ordering::Acquire) {
            self.cleanup_staging(pending);
            let status = self.set_status(
                &pending.preview.recipe_id,
                LanguageToolInstallState::Cancelled,
                100,
                "Installation cancelled",
                started_at,
                Some(Utc::now()),
                None,
            )?;
            return Ok(LanguageToolInstallResult {
                status,
                executable: None,
                output,
            });
        }
        if !install_outcome.success {
            self.cleanup_staging(pending);
            let detail = install_failure_detail(&output);
            let status = self.set_status(
                &pending.preview.recipe_id,
                LanguageToolInstallState::Error,
                100,
                "Installer failed",
                started_at,
                Some(Utc::now()),
                Some(detail),
            )?;
            return Ok(LanguageToolInstallResult {
                status,
                executable: None,
                output,
            });
        }

        if pending.layout == InstallLayout::ManagedDirectory && !pending.expected_executable.is_file()
        {
            self.cleanup_staging(pending);
            let detail = "Installer completed but the expected language-server executable was not created"
                .to_string();
            let status = self.set_status(
                &pending.preview.recipe_id,
                LanguageToolInstallState::Error,
                100,
                "Version probe could not start",
                started_at,
                Some(Utc::now()),
                Some(detail),
            )?;
            return Ok(LanguageToolInstallResult {
                status,
                executable: None,
                output,
            });
        }

        self.set_status(
            &pending.preview.recipe_id,
            LanguageToolInstallState::Installing,
            70,
            "Verifying installed language server",
            started_at,
            None,
            None,
        )?;
        let probe_outcome = self
            .runner
            .run(&pending.preview.probe_command, &work_dir, cancel)?;
        let probe_output = merged_output(&probe_outcome);
        if !probe_output.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&probe_output);
        }

        if probe_outcome.cancelled || cancel.load(Ordering::Acquire) {
            self.cleanup_staging(pending);
            let status = self.set_status(
                &pending.preview.recipe_id,
                LanguageToolInstallState::Cancelled,
                100,
                "Installation cancelled",
                started_at,
                Some(Utc::now()),
                None,
            )?;
            return Ok(LanguageToolInstallResult {
                status,
                executable: None,
                output,
            });
        }
        if !probe_outcome.success {
            self.cleanup_staging(pending);
            let status = self.set_status(
                &pending.preview.recipe_id,
                LanguageToolInstallState::Error,
                100,
                "Version probe failed",
                started_at,
                Some(Utc::now()),
                Some("Installed language server failed its version probe".into()),
            )?;
            return Ok(LanguageToolInstallResult {
                status,
                executable: None,
                output,
            });
        }

        let executable = match pending.layout {
            InstallLayout::ManagedDirectory => {
                self.set_status(
                    &pending.preview.recipe_id,
                    LanguageToolInstallState::Installing,
                    90,
                    "Promoting verified installation",
                    started_at,
                    None,
                    None,
                )?;
                let staging = pending.staging_dir.as_deref().ok_or_else(|| {
                    RepoDeskError::Api("Managed install has no staging directory".into())
                })?;
                let destination = pending.destination_dir.as_deref().ok_or_else(|| {
                    RepoDeskError::Api("Managed install has no destination".into())
                })?;
                promote_directory(staging, destination)?;
                Some(
                    executable_in_install_root(destination, recipe(&pending.preview.recipe_id)?)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
            InstallLayout::ExternalToolchain => Some(pending.preview.probe_command.program.clone()),
        };

        let status = self.set_status(
            &pending.preview.recipe_id,
            LanguageToolInstallState::Ready,
            100,
            "Language server installed and verified",
            started_at,
            Some(Utc::now()),
            None,
        )?;
        Ok(LanguageToolInstallResult {
            status,
            executable,
            output,
        })
    }

    fn cleanup_staging(&self, pending: &PendingInstall) {
        if let Some(staging) = &pending.staging_dir {
            let _ = fs::remove_dir_all(staging);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn set_status(
        &self,
        recipe_id: &str,
        state: LanguageToolInstallState,
        progress: u8,
        message: &str,
        started_at: DateTime<Utc>,
        finished_at: Option<DateTime<Utc>>,
        error: Option<String>,
    ) -> RepoDeskResult<LanguageToolInstallStatus> {
        let status = LanguageToolInstallStatus {
            recipe_id: recipe_id.to_string(),
            state,
            progress,
            message: message.to_string(),
            started_at,
            finished_at,
            error,
        };
        self.statuses
            .lock()
            .map_err(|_| RepoDeskError::Api("Language-tool status registry is unavailable".into()))?
            .insert(recipe_id.to_string(), status.clone());
        Ok(status)
    }
}

pub fn managed_executable_path(recipe_id: &str) -> Option<PathBuf> {
    let recipe = recipe(recipe_id).ok()?;
    if recipe.layout != InstallLayout::ManagedDirectory {
        return None;
    }
    let paths = RepoDeskPaths::resolve().ok()?;
    let root = paths
        .home
        .join("tools")
        .join("language-servers")
        .join(recipe.id);
    let executable = executable_in_install_root(&root, recipe);
    executable.is_file().then_some(executable)
}

fn recipe(recipe_id: &str) -> RepoDeskResult<&'static InstallRecipe> {
    RECIPES
        .iter()
        .find(|recipe| recipe.id == recipe_id)
        .ok_or_else(|| RepoDeskError::Api(format!("Unknown language-tool recipe '{recipe_id}'")))
}

fn install_command(
    recipe: &InstallRecipe,
    staging_dir: Option<&Path>,
    _repodesk_home: &Path,
) -> RepoDeskResult<LanguageToolCommand> {
    let args = match recipe.installer {
        LanguageToolInstaller::Npm => {
            let staging = staging_dir.ok_or_else(|| {
                RepoDeskError::Api("Managed npm recipe has no staging directory".into())
            })?;
            let mut args = vec![
                "install".into(),
                "--prefix".into(),
                staging.to_string_lossy().into_owned(),
                "--no-save".into(),
                "--ignore-scripts".into(),
                "--no-audit".into(),
                "--no-fund".into(),
                "--package-lock=false".into(),
                format!("{}@{}", recipe.package, recipe.version),
            ];
            if let Some((package, version)) = recipe.companion_package {
                args.push(format!("{package}@{version}"));
            }
            args
        }
        LanguageToolInstaller::Cargo => {
            let staging = staging_dir.ok_or_else(|| {
                RepoDeskError::Api("Managed Cargo recipe has no staging directory".into())
            })?;
            vec![
                "install".into(),
                recipe.package.into(),
                "--version".into(),
                recipe.version.into(),
                "--locked".into(),
                "--root".into(),
                staging.to_string_lossy().into_owned(),
            ]
        }
        LanguageToolInstaller::Rustup => vec![
            "component".into(),
            "add".into(),
            "rust-analyzer".into(),
        ],
    };
    Ok(LanguageToolCommand {
        program: recipe.installer.executable().into(),
        args,
    })
}

fn executable_in_install_root(root: &Path, recipe: &InstallRecipe) -> PathBuf {
    match recipe.installer {
        LanguageToolInstaller::Npm => {
            let base = root
                .join("node_modules")
                .join(".bin")
                .join(recipe.executable);
            if cfg!(windows) {
                base.with_extension("cmd")
            } else {
                base
            }
        }
        LanguageToolInstaller::Cargo => {
            let base = root.join("bin").join(recipe.executable);
            if cfg!(windows) {
                base.with_extension("exe")
            } else {
                base
            }
        }
        LanguageToolInstaller::Rustup => PathBuf::from(recipe.executable),
    }
}

fn recipe_revision(recipe: &InstallRecipe) -> String {
    let mut hasher = Sha256::new();
    hasher.update(recipe.id.as_bytes());
    hasher.update([0]);
    hasher.update(recipe.server_id.as_bytes());
    hasher.update([0]);
    hasher.update(recipe.package.as_bytes());
    hasher.update([0]);
    hasher.update(recipe.version.as_bytes());
    hasher.update([0]);
    hasher.update(recipe.executable.as_bytes());
    if let Some((package, version)) = recipe.companion_package {
        hasher.update([0]);
        hasher.update(package.as_bytes());
        hasher.update([0]);
        hasher.update(version.as_bytes());
    }
    format!("r1-{}", &hex::encode(hasher.finalize())[..16])
}

fn confirmation_token(
    project: &str,
    recipe_id: &str,
    revision: &str,
    destination: Option<&Path>,
    command: &LanguageToolCommand,
    issued_at: DateTime<Utc>,
    sequence: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project.as_bytes());
    hasher.update([0]);
    hasher.update(recipe_id.as_bytes());
    hasher.update([0]);
    hasher.update(revision.as_bytes());
    hasher.update([0]);
    if let Some(destination) = destination {
        hasher.update(destination.as_os_str().to_string_lossy().as_bytes());
    }
    hasher.update([0]);
    hasher.update(command.program.as_bytes());
    for arg in &command.args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    hasher.update(
        issued_at
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .to_le_bytes(),
    );
    hasher.update(sequence.to_le_bytes());
    format!("lang_install_{}", hex::encode(hasher.finalize()))
}

fn executable_on_path(executable: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| {
            let base = directory.join(executable);
            if cfg!(windows) {
                base.is_file()
                    || base.with_extension("exe").is_file()
                    || base.with_extension("cmd").is_file()
                    || base.with_extension("bat").is_file()
            } else {
                base.is_file()
            }
        })
    })
}

fn promote_directory(staging: &Path, destination: &Path) -> RepoDeskResult<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| RepoDeskError::Api("Language-tool destination has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| RepoDeskError::Api("Language-tool destination is invalid".into()))?;
    let backup = parent.join(format!(".{file_name}.previous"));
    if backup.exists() {
        fs::remove_dir_all(&backup)?;
    }
    if destination.exists() {
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(staging, destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error.into());
    }
    if backup.exists() {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}

fn read_bounded_text(path: &Path) -> String {
    let Ok(bytes) = fs::read(path) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    redact_and_bound(&text)
}

fn merged_output(outcome: &CommandOutcome) -> String {
    let mut value = String::new();
    if !outcome.stdout.trim().is_empty() {
        value.push_str(outcome.stdout.trim());
    }
    if !outcome.stderr.trim().is_empty() {
        if !value.is_empty() {
            value.push('\n');
        }
        value.push_str(outcome.stderr.trim());
    }
    redact_and_bound(&value)
}

fn redact_and_bound(value: &str) -> String {
    let mut output = String::new();
    for line in value.lines() {
        let lower = line.to_ascii_lowercase();
        let sensitive = [
            "authorization",
            "password",
            "secret",
            "token=",
            "_auth",
            "npm_token",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let next = if sensitive {
            "[redacted installer output]"
        } else {
            line
        };
        if output.chars().count() + next.chars().count() + 1 > MAX_INSTALL_OUTPUT_CHARS {
            output.push_str("\n[RepoDesk truncated installer output]");
            break;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(next);
    }
    output
}

fn install_failure_detail(output: &str) -> String {
    output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Language-tool installer exited unsuccessfully")
        .chars()
        .take(600)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_ids_are_literal_and_unique() {
        let ids = RECIPES.iter().map(|recipe| recipe.id).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "rust-analyzer",
                "typescript-language-server",
                "json-language-server",
                "yaml-language-server",
                "taplo"
            ]
        );
    }

    #[test]
    fn redaction_removes_secret_like_lines_and_bounds_output() {
        let safe = redact_and_bound("ok\nAuthorization: Bearer nope\nNPM_TOKEN=secret\nfinished");
        assert!(safe.contains("ok"));
        assert!(safe.contains("finished"));
        assert!(!safe.contains("Bearer nope"));
        assert!(!safe.contains("NPM_TOKEN"));
    }
}
