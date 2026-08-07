use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 100;
const MIN_ROWS: u16 = 2;
const MIN_COLS: u16 = 10;
const MAX_ROWS: u16 = 300;
const MAX_COLS: u16 = 500;
const MAX_SESSIONS: usize = 4;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const READ_BUFFER_BYTES: usize = 8 * 1024;

#[derive(Clone, Default)]
pub struct TerminalManager {
    inner: Arc<TerminalManagerInner>,
}

#[derive(Default)]
struct TerminalManagerInner {
    next_id: AtomicU64,
    sessions: Mutex<HashMap<String, Arc<TerminalSession>>>,
}

struct TerminalSession {
    info: TerminalSessionInfo,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    output_sequence: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalSessionInfo {
    pub session_id: String,
    pub cwd: String,
    pub pid: Option<u32>,
    pub shell: String,
}

#[derive(Debug, Clone, Serialize)]
struct TerminalOutputPayload {
    session_id: String,
    sequence: u64,
    /// Base64 keeps the PTY byte stream lossless. The frontend passes decoded
    /// bytes directly to xterm.js, so UTF-8 split across read chunks is safe.
    data: String,
}

#[derive(Debug, Clone, Serialize)]
struct TerminalExitPayload {
    session_id: String,
    exit_code: Option<u32>,
    signal: Option<String>,
    error: Option<String>,
}

impl TerminalManager {
    fn create(
        &self,
        app: AppHandle,
        rows: Option<u16>,
        cols: Option<u16>,
    ) -> Result<TerminalSessionInfo, String> {
        let cwd = active_project_cwd()?;
        let rows = clamp_rows(rows.unwrap_or(DEFAULT_ROWS));
        let cols = clamp_cols(cols.unwrap_or(DEFAULT_COLS));

        {
            let sessions = self
                .inner
                .sessions
                .lock()
                .map_err(|_| "Terminal session registry is unavailable".to_string())?;
            if sessions.len() >= MAX_SESSIONS {
                return Err(format!(
                    "RepoDesk limits interactive terminals to {MAX_SESSIONS} live sessions"
                ));
            }
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Failed to open terminal PTY: {error}"))?;

        let mut command = CommandBuilder::new_default_prog();
        command.cwd(&cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "RepoDesk");

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Failed to start system shell: {error}"))?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Failed to attach terminal output: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Failed to attach terminal input: {error}"))?;

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let session_id = format!("terminal-{id}");
        let info = TerminalSessionInfo {
            session_id: session_id.clone(),
            cwd: cwd.to_string_lossy().to_string(),
            pid,
            shell: "system default".to_string(),
        };
        let session = Arc::new(TerminalSession {
            info: info.clone(),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            output_sequence: AtomicU64::new(0),
        });

        self.inner
            .sessions
            .lock()
            .map_err(|_| "Terminal session registry is unavailable".to_string())?
            .insert(session_id.clone(), Arc::clone(&session));

        let output_app = app.clone();
        let output_session = Arc::clone(&session);
        let output_session_id = session_id.clone();
        thread::Builder::new()
            .name(format!("repodesk-pty-read-{id}"))
            .spawn(move || {
                let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            let sequence = output_session
                                .output_sequence
                                .fetch_add(1, Ordering::Relaxed)
                                + 1;
                            let payload = TerminalOutputPayload {
                                session_id: output_session_id.clone(),
                                sequence,
                                data: BASE64_STANDARD.encode(&buffer[..read]),
                            };
                            if output_app.emit("terminal-output", payload).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            eprintln!("Terminal session {output_session_id} read error: {error}");
                            break;
                        }
                    }
                }
            })
            .map_err(|error| format!("Failed to start terminal reader: {error}"))?;

        let wait_app = app;
        let wait_inner = Arc::clone(&self.inner);
        let wait_session_id = session_id.clone();
        thread::Builder::new()
            .name(format!("repodesk-pty-wait-{id}"))
            .spawn(move || {
                let payload = match child.wait() {
                    Ok(status) => TerminalExitPayload {
                        session_id: wait_session_id.clone(),
                        exit_code: Some(status.exit_code()),
                        signal: status.signal().map(ToOwned::to_owned),
                        error: None,
                    },
                    Err(error) => TerminalExitPayload {
                        session_id: wait_session_id.clone(),
                        exit_code: None,
                        signal: None,
                        error: Some(error.to_string()),
                    },
                };

                if let Ok(mut sessions) = wait_inner.sessions.lock() {
                    sessions.remove(&wait_session_id);
                }
                let _ = wait_app.emit("terminal-exit", payload);
            })
            .map_err(|error| format!("Failed to start terminal waiter: {error}"))?;

        Ok(info)
    }

    fn session(&self, session_id: &str) -> Result<Arc<TerminalSession>, String> {
        self.inner
            .sessions
            .lock()
            .map_err(|_| "Terminal session registry is unavailable".to_string())?
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Terminal session not found: {session_id}"))
    }

    fn list(&self) -> Result<Vec<TerminalSessionInfo>, String> {
        let sessions = self
            .inner
            .sessions
            .lock()
            .map_err(|_| "Terminal session registry is unavailable".to_string())?;
        let mut values = sessions
            .values()
            .map(|session| session.info.clone())
            .collect::<Vec<_>>();
        values.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        Ok(values)
    }

    fn remove(&self, session_id: &str) {
        if let Ok(mut sessions) = self.inner.sessions.lock() {
            sessions.remove(session_id);
        }
    }
}

#[tauri::command]
pub fn terminal_create(
    app: AppHandle,
    manager: State<'_, TerminalManager>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<TerminalSessionInfo, String> {
    manager.create(app, rows, cols)
}

#[tauri::command]
pub fn terminal_list(
    manager: State<'_, TerminalManager>,
) -> Result<Vec<TerminalSessionInfo>, String> {
    manager.list()
}

#[tauri::command]
pub fn terminal_write(
    manager: State<'_, TerminalManager>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "Terminal input chunk is too large (max {MAX_INPUT_BYTES} bytes)"
        ));
    }

    let session = manager.session(&session_id)?;
    let mut writer = session
        .writer
        .lock()
        .map_err(|_| "Terminal input stream is unavailable".to_string())?;
    writer
        .write_all(data.as_bytes())
        .and_then(|_| writer.flush())
        .map_err(|error| format!("Failed to write terminal input: {error}"))
}

#[tauri::command]
pub fn terminal_resize(
    manager: State<'_, TerminalManager>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let session = manager.session(&session_id)?;
    let master = session
        .master
        .lock()
        .map_err(|_| "Terminal PTY is unavailable".to_string())?;
    master
        .resize(PtySize {
            rows: clamp_rows(rows),
            cols: clamp_cols(cols),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to resize terminal: {error}"))
}

#[tauri::command]
pub fn terminal_kill(
    manager: State<'_, TerminalManager>,
    session_id: String,
) -> Result<(), String> {
    let session = manager.session(&session_id)?;
    let result = {
        let mut killer = session
            .killer
            .lock()
            .map_err(|_| "Terminal process handle is unavailable".to_string())?;
        killer
            .kill()
            .map_err(|error| format!("Failed to stop terminal process: {error}"))
    };
    // Drop from the registry immediately so `terminal_list` reflects the kill
    // right away, instead of waiting on the wait-thread's own removal once
    // the child process actually exits.
    manager.remove(&session_id);
    result
}

fn active_project_cwd() -> Result<PathBuf, String> {
    let project = repodesk_core::projects::get_active_project()
        .map_err(|_| "Connect or select a project before opening a terminal".to_string())?;
    let cwd = project
        .path
        .canonicalize()
        .map_err(|error| format!("Active project path is unavailable: {error}"))?;
    if !cwd.is_dir() {
        return Err("Active project path is not a directory".to_string());
    }
    Ok(cwd)
}

fn clamp_rows(rows: u16) -> u16 {
    rows.clamp(MIN_ROWS, MAX_ROWS)
}

fn clamp_cols(cols: u16) -> u16 {
    cols.clamp(MIN_COLS, MAX_COLS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_dimensions_are_bounded() {
        assert_eq!(clamp_rows(0), MIN_ROWS);
        assert_eq!(clamp_rows(u16::MAX), MAX_ROWS);
        assert_eq!(clamp_cols(0), MIN_COLS);
        assert_eq!(clamp_cols(u16::MAX), MAX_COLS);
    }

    #[test]
    fn terminal_manager_starts_empty() {
        let manager = TerminalManager::default();
        assert!(manager.list().unwrap().is_empty());
    }
}
