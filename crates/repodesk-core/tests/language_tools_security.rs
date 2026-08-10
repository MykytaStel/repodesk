use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};
use repodesk_core::language_tools::{
    CommandOutcome, LanguageToolCommand, LanguageToolCommandRunner, LanguageToolInstallService,
    LanguageToolInstallState,
};
use repodesk_core::{RepoDeskError, RepoDeskResult};
use tempfile::TempDir;

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<LanguageToolCommand>>,
}

impl FakeRunner {
    fn calls(&self) -> Vec<LanguageToolCommand> {
        self.calls.lock().expect("fake runner calls").clone()
    }
}

impl LanguageToolCommandRunner for FakeRunner {
    fn run(
        &self,
        command: &LanguageToolCommand,
        _cwd: &Path,
        cancel: &AtomicBool,
    ) -> RepoDeskResult<CommandOutcome> {
        self.calls
            .lock()
            .expect("fake runner calls")
            .push(command.clone());

        if cancel.load(Ordering::Acquire) {
            return Ok(CommandOutcome {
                success: false,
                cancelled: true,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        if command.program == "cargo" {
            let root = flag_value(&command.args, "--root").expect("cargo --root");
            let bin = Path::new(root).join("bin");
            fs::create_dir_all(&bin)?;
            fs::write(platform_executable(&bin, "taplo"), b"fake taplo")?;
        } else if command.program == "npm" {
            let prefix = flag_value(&command.args, "--prefix").expect("npm --prefix");
            let bin = Path::new(prefix).join("node_modules").join(".bin");
            fs::create_dir_all(&bin)?;
            for executable in [
                "typescript-language-server",
                "vscode-json-language-server",
                "yaml-language-server",
            ] {
                fs::write(platform_executable(&bin, executable), b"fake language server")?;
            }
        }

        Ok(CommandOutcome {
            success: true,
            cancelled: false,
            stdout: if command.program == "cargo" || command.program == "npm" {
                "installed\nNPM_TOKEN=do-not-leak".into()
            } else {
                "1.0.0".into()
            },
            stderr: String::new(),
        })
    }
}

struct BlockingRunner;

impl LanguageToolCommandRunner for BlockingRunner {
    fn run(
        &self,
        _command: &LanguageToolCommand,
        _cwd: &Path,
        cancel: &AtomicBool,
    ) -> RepoDeskResult<CommandOutcome> {
        for _ in 0..400 {
            if cancel.load(Ordering::Acquire) {
                return Ok(CommandOutcome {
                    success: false,
                    cancelled: true,
                    stdout: "cancelled".into(),
                    stderr: String::new(),
                });
            }
            thread::sleep(StdDuration::from_millis(5));
        }
        Ok(CommandOutcome {
            success: false,
            cancelled: false,
            stdout: String::new(),
            stderr: "blocking fake timed out".into(),
        })
    }
}

#[test]
fn preview_builds_pinned_allowlisted_argv_outside_repository() {
    let repo = TempDir::new().expect("repo tempdir");
    let home = TempDir::new().expect("home tempdir");
    let runner = Arc::new(FakeRunner::default());
    let service = LanguageToolInstallService::new(runner);
    let now = Utc::now();

    let preview = service
        .preview_for_project(
            "typescript-language-server",
            "demo",
            repo.path(),
            home.path(),
            now,
        )
        .expect("preview");

    assert_eq!(preview.recipe_id, "typescript-language-server");
    assert_eq!(preview.install_command.program, "npm");
    assert_eq!(preview.install_command.args[0], "install");
    assert_eq!(
        flag_value(&preview.install_command.args, "--prefix")
            .map(Path::new)
            .and_then(|path| path.parent())
            .and_then(Path::parent),
        Some(home.path().join("tools/language-servers").as_path())
    );
    assert!(
        preview
            .install_command
            .args
            .iter()
            .any(|arg| arg == "typescript-language-server@5.3.0")
    );
    assert!(
        preview
            .install_command
            .args
            .iter()
            .any(|arg| arg == "typescript@7.0.2")
    );
    assert!(preview.install_command.args.iter().any(|arg| arg == "--ignore-scripts"));
    assert!(preview.install_command.args.iter().any(|arg| arg == "--no-audit"));
    assert!(preview.confirmation_token.starts_with("lang_install_"));
    assert!(preview.destination.starts_with(home.path().to_string_lossy().as_ref()));
    assert!(!preview.destination.starts_with(repo.path().to_string_lossy().as_ref()));
    assert_eq!(preview.expires_at, now + Duration::minutes(5));
}

#[test]
fn expired_and_reused_confirmation_tokens_are_rejected() {
    let repo = TempDir::new().expect("repo tempdir");
    let home = TempDir::new().expect("home tempdir");
    let service = LanguageToolInstallService::new(Arc::new(FakeRunner::default()));
    let now = Utc::now();
    let preview = service
        .preview_for_project("taplo", "demo", repo.path(), home.path(), now)
        .expect("preview");

    let error = service
        .install_at(&preview.confirmation_token, now + Duration::minutes(6))
        .expect_err("expired token must fail");
    assert!(error.to_string().contains("expired"));

    let second = service
        .install_at(&preview.confirmation_token, now)
        .expect_err("token is single-use");
    assert!(second.to_string().contains("invalid or already used"));
}

#[test]
fn verified_install_promotes_staging_without_repository_writes() {
    let repo = TempDir::new().expect("repo tempdir");
    let home = TempDir::new().expect("home tempdir");
    let sentinel = repo.path().join("sentinel.txt");
    fs::write(&sentinel, "unchanged").expect("sentinel");

    let final_root = home.path().join("tools/language-servers/taplo");
    fs::create_dir_all(&final_root).expect("existing install");
    fs::write(final_root.join("old.txt"), "old").expect("old install");

    let runner = Arc::new(FakeRunner::default());
    let service = LanguageToolInstallService::new(runner.clone());
    let now = Utc::now();
    let preview = service
        .preview_for_project("taplo", "demo", repo.path(), home.path(), now)
        .expect("preview");
    assert!(preview.prerequisite_available, "cargo must be available while cargo tests run");

    let result = service
        .install_at(&preview.confirmation_token, now)
        .expect("install result");

    assert_eq!(result.status.state, LanguageToolInstallState::Ready);
    assert_eq!(result.status.progress, 100);
    assert_eq!(fs::read_to_string(&sentinel).expect("sentinel read"), "unchanged");
    assert!(!final_root.join("old.txt").exists());
    assert!(platform_executable(&final_root.join("bin"), "taplo").is_file());
    assert!(!result.output.contains("do-not-leak"));
    assert!(!result.output.contains("NPM_TOKEN"));

    let calls = runner.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].program, "cargo");
    assert_eq!(calls[0].args[0], "install");
    assert!(calls[0].args.iter().any(|arg| arg == "taplo-cli"));
    assert!(calls[0].args.iter().any(|arg| arg == "0.10.0"));
    assert!(calls[0].args.iter().any(|arg| arg == "--locked"));
    assert!(calls[0].args.iter().any(|arg| arg == "--root"));
    assert_eq!(calls[1].args, vec!["--version"]);
}

#[test]
fn cancellation_does_not_promote_partial_installation() {
    let repo = TempDir::new().expect("repo tempdir");
    let home = TempDir::new().expect("home tempdir");
    let service = Arc::new(LanguageToolInstallService::new(Arc::new(BlockingRunner)));
    let now = Utc::now();
    let preview = service
        .preview_for_project("taplo", "demo", repo.path(), home.path(), now)
        .expect("preview");
    assert!(preview.prerequisite_available, "cargo must be available while cargo tests run");

    let token = preview.confirmation_token.clone();
    let worker_service = service.clone();
    let worker = thread::spawn(move || worker_service.install_at(&token, now));

    let mut started = false;
    for _ in 0..100 {
        if service
            .status("taplo")
            .expect("status")
            .is_some_and(|status| status.state == LanguageToolInstallState::Installing)
        {
            started = true;
            break;
        }
        thread::sleep(StdDuration::from_millis(5));
    }
    assert!(started, "install should enter installing state");
    assert!(service.cancel("taplo").expect("cancel"));

    let result = worker.join().expect("worker join").expect("install response");
    assert_eq!(result.status.state, LanguageToolInstallState::Cancelled);
    assert!(!home.path().join("tools/language-servers/taplo").exists());
}

#[test]
fn unknown_recipe_is_rejected_before_any_execution() {
    let repo = TempDir::new().expect("repo tempdir");
    let home = TempDir::new().expect("home tempdir");
    let runner = Arc::new(FakeRunner::default());
    let service = LanguageToolInstallService::new(runner.clone());

    let error = service
        .preview_for_project("curl-pipe-shell", "demo", repo.path(), home.path(), Utc::now())
        .expect_err("unknown recipe must fail");

    assert!(matches!(error, RepoDeskError::Api(_)));
    assert!(error.to_string().contains("Unknown language-tool recipe"));
    assert!(runner.calls().is_empty());
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn platform_executable(directory: &Path, executable: &str) -> std::path::PathBuf {
    let base = directory.join(executable);
    if cfg!(windows) {
        base.with_extension(if executable == "taplo" { "exe" } else { "cmd" })
    } else {
        base
    }
}
