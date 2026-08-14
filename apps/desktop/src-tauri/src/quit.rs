use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

const SAFE_QUIT_EVENT: &str = "repodesk-safe-quit-requested";
const SAFE_QUIT_FALLBACK: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub struct QuitCoordinator {
    next_request_id: AtomicU64,
    pending_request_id: AtomicU64,
    acknowledged_request_id: AtomicU64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeQuitRequest {
    request_id: u64,
}

impl QuitCoordinator {
    fn request(&self) -> u64 {
        let request_id = self
            .next_request_id
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        self.acknowledged_request_id.store(0, Ordering::SeqCst);
        self.pending_request_id.store(request_id, Ordering::SeqCst);
        request_id
    }

    fn acknowledge(&self, request_id: u64) -> bool {
        if request_id == 0 || self.pending_request_id.load(Ordering::SeqCst) != request_id {
            return false;
        }
        self.acknowledged_request_id
            .store(request_id, Ordering::SeqCst);
        true
    }

    fn complete(&self, request_id: u64) -> bool {
        if self.pending_request_id.load(Ordering::SeqCst) != request_id
            || self.acknowledged_request_id.load(Ordering::SeqCst) != request_id
        {
            return false;
        }
        self.pending_request_id.store(0, Ordering::SeqCst);
        self.acknowledged_request_id.store(0, Ordering::SeqCst);
        true
    }

    fn cancel(&self, request_id: u64) -> bool {
        if self.pending_request_id.load(Ordering::SeqCst) != request_id {
            return false;
        }
        self.pending_request_id.store(0, Ordering::SeqCst);
        self.acknowledged_request_id.store(0, Ordering::SeqCst);
        true
    }

    fn should_force_exit(&self, request_id: u64) -> bool {
        self.pending_request_id.load(Ordering::SeqCst) == request_id
            && self.acknowledged_request_id.load(Ordering::SeqCst) != request_id
    }
}

/// Ask the webview to flush recovery-critical state before process exit.
///
/// If the webview is gone or cannot acknowledge the request within a bounded
/// window, RepoDesk still exits instead of leaving a zombie tray process. Once
/// the frontend acknowledges the request, the fallback is disabled: a failed or
/// hung draft flush must keep the app alive rather than trade liveness for data.
pub fn request_safe_quit(app: &AppHandle) {
    let request_id = app.state::<QuitCoordinator>().request();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(SAFE_QUIT_EVENT, SafeQuitRequest { request_id });
    }

    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(SAFE_QUIT_FALLBACK);
        if app.state::<QuitCoordinator>().should_force_exit(request_id) {
            app.exit(0);
        }
    });
}

#[tauri::command]
pub fn safe_quit_ack(
    request_id: u64,
    coordinator: State<'_, QuitCoordinator>,
) -> Result<(), String> {
    coordinator
        .acknowledge(request_id)
        .then_some(())
        .ok_or_else(|| "Safe quit request is stale or no longer pending".to_string())
}

#[tauri::command]
pub fn safe_quit_complete(
    app: AppHandle,
    request_id: u64,
    coordinator: State<'_, QuitCoordinator>,
) -> Result<(), String> {
    if !coordinator.complete(request_id) {
        return Err("Safe quit request was not acknowledged or is stale".to_string());
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn safe_quit_cancel(
    app: AppHandle,
    request_id: u64,
    coordinator: State<'_, QuitCoordinator>,
) -> Result<(), String> {
    if !coordinator.cancel(request_id) {
        return Err("Safe quit request is stale or no longer pending".to_string());
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledged_request_disables_force_exit_until_completed() {
        let coordinator = QuitCoordinator::default();
        let request_id = coordinator.request();
        assert!(coordinator.should_force_exit(request_id));
        assert!(coordinator.acknowledge(request_id));
        assert!(!coordinator.should_force_exit(request_id));
        assert!(coordinator.complete(request_id));
        assert!(!coordinator.should_force_exit(request_id));
    }

    #[test]
    fn stale_ack_cannot_complete_a_newer_request() {
        let coordinator = QuitCoordinator::default();
        let old_request = coordinator.request();
        let current_request = coordinator.request();
        assert_ne!(old_request, current_request);
        assert!(!coordinator.acknowledge(old_request));
        assert!(!coordinator.complete(old_request));
        assert!(coordinator.should_force_exit(current_request));
    }

    #[test]
    fn cancel_clears_pending_exit() {
        let coordinator = QuitCoordinator::default();
        let request_id = coordinator.request();
        assert!(coordinator.acknowledge(request_id));
        assert!(coordinator.cancel(request_id));
        assert!(!coordinator.complete(request_id));
        assert!(!coordinator.should_force_exit(request_id));
    }
}
