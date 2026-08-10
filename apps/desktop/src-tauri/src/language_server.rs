use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};
use repodesk_core::code_workspace::{MAX_EDITABLE_FILE_BYTES, read_active_code_document};
use repodesk_core::language_intelligence::{
    LanguageDiagnostic, LanguageDiagnosticSeverity, LanguageServerAvailability,
    LanguageServerInitializationProfile, LanguageServerProfileState, LspPosition, LspRange,
    active_language_intelligence_snapshot, preferred_server_for_language,
};
use repodesk_core::projects::get_active_project;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_DIAGNOSTICS_PER_FILE: usize = 500;
const MAX_LOCATIONS: usize = 128;
const MAX_SYMBOLS: usize = 500;
const MAX_HOVER_CHARS: usize = 12_000;
const MAX_SERVER_ERROR_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageServerSessionState {
    Starting,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageServerStatus {
    pub project: String,
    pub server_id: String,
    pub state: LanguageServerSessionState,
    pub pid: u32,
    pub open_documents: usize,
    pub started_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageHover {
    pub markdown: String,
    pub range: Option<LspRange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageLocation {
    pub path: String,
    pub library_handle: Option<String>,
    pub read_only: bool,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

struct DefinitionTarget {
    path: String,
    library_handle: Option<String>,
    read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32,
    pub range: LspRange,
    pub selection_range: LspRange,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageDiagnosticsEvent {
    pub project: String,
    pub server_id: String,
    pub path: String,
    pub diagnostics: Vec<LanguageDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    project: String,
    server_id: String,
}

impl SessionKey {
    fn new(project: impl Into<String>, server_id: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            server_id: server_id.into(),
        }
    }
}

struct SessionRegistry<T> {
    entries: HashMap<SessionKey, T>,
}

impl<T> Default for SessionRegistry<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<T> SessionRegistry<T> {
    fn get(&self, key: &SessionKey) -> Option<&T> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: SessionKey, value: T) -> Option<T> {
        self.entries.insert(key, value)
    }

    fn remove(&mut self, key: &SessionKey) -> Option<T> {
        self.entries.remove(key)
    }

    fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.values()
    }

    fn remove_other_projects(&mut self, project: &str) -> Vec<T> {
        let stale = self
            .entries
            .keys()
            .filter(|key| key.project != project)
            .cloned()
            .collect::<Vec<_>>();
        stale
            .into_iter()
            .filter_map(|key| self.entries.remove(&key))
            .collect()
    }

    fn drain(&mut self) -> Vec<T> {
        self.entries.drain().map(|(_, value)| value).collect()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Default, Clone)]
pub struct LanguageServerManager {
    sessions: Arc<Mutex<SessionRegistry<Arc<SessionInner>>>>,
}

struct SessionInner {
    project: String,
    root: PathBuf,
    root_uri: String,
    server_id: String,
    pid: u32,
    started_at: DateTime<Utc>,
    state: Mutex<SessionState>,
    writer: Mutex<ChildStdin>,
    child: Mutex<Child>,
    stderr_tail: Mutex<String>,
    stopping: AtomicBool,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>,
    next_request_id: AtomicU64,
    documents: Mutex<HashMap<String, u32>>,
    document_sync: Mutex<()>,
}

struct SessionState {
    state: LanguageServerSessionState,
    last_error: Option<String>,
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl LanguageServerManager {
    pub fn status(&self) -> Option<LanguageServerStatus> {
        self.sessions
            .lock()
            .ok()
            .and_then(|registry| registry.values().next().cloned())
            .map(|session| session.status())
    }

    pub fn statuses(&self) -> Vec<LanguageServerStatus> {
        self.sessions
            .lock()
            .map(|registry| registry.values().map(|session| session.status()).collect())
            .unwrap_or_default()
    }

    pub fn restart(
        &self,
        app: &AppHandle,
        server_id: &str,
    ) -> Result<LanguageServerStatus, String> {
        let project = get_active_project().map_err(|error| error.to_string())?;
        let canonical_root = project
            .path
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let snapshot =
            active_language_intelligence_snapshot().map_err(|error| error.to_string())?;
        let server = snapshot
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| format!("Language server '{server_id}' is not registered"))?;
        if server.profile_state != LanguageServerProfileState::Active {
            return Err(format!("{} live support is not enabled", server.label));
        }
        if server.availability != LanguageServerAvailability::Available {
            return Err(format!("{} is not available", server.label));
        }

        let key = SessionKey::new(&project.name, server_id);
        let previous = self
            .sessions
            .lock()
            .map_err(|_| "Language server manager lock is poisoned".to_string())?
            .remove(&key);
        if let Some(previous) = previous {
            previous.shutdown();
        }
        let session = SessionInner::start(
            app.clone(),
            project.name,
            canonical_root,
            &server.id,
            &server.executable,
            &server.arguments,
            server.initialization_profile,
        )?;
        let status = session.status();
        self.sessions
            .lock()
            .map_err(|_| "Language server manager lock is poisoned".to_string())?
            .insert(key, session);
        Ok(status)
    }

    pub fn sync_document(
        &self,
        app: &AppHandle,
        path: &str,
        language: &str,
        text: &str,
    ) -> Result<LanguageServerStatus, String> {
        validate_document_payload(path, language, text)?;
        let session = self.ensure_session(app, language)?;
        session.sync_document(path, language, text)?;
        Ok(session.status())
    }

    pub fn close_document(&self, path: &str) -> Result<(), String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "Language server manager lock is poisoned".to_string())?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            session.close_document(path)?;
        }
        Ok(())
    }

    pub fn hover(
        &self,
        app: &AppHandle,
        path: &str,
        text: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<LanguageHover>, String> {
        let session = self.ensure_synced_document(app, path, text)?;
        let result = session.request(
            "textDocument/hover",
            position_params(&session, path, line, column)?,
            REQUEST_TIMEOUT,
        )?;
        parse_hover(result)
    }

    pub fn definition(
        &self,
        app: &AppHandle,
        path: &str,
        text: &str,
        line: u32,
        column: u32,
    ) -> Result<Vec<LanguageLocation>, String> {
        let session = self.ensure_synced_document(app, path, text)?;
        let result = session.request(
            "textDocument/definition",
            position_params(&session, path, line, column)?,
            REQUEST_TIMEOUT,
        )?;
        Ok(parse_locations(&session, &result, MAX_LOCATIONS))
    }

    pub fn references(
        &self,
        app: &AppHandle,
        path: &str,
        text: &str,
        line: u32,
        column: u32,
    ) -> Result<Vec<LanguageLocation>, String> {
        let session = self.ensure_synced_document(app, path, text)?;
        let mut params = position_params(&session, path, line, column)?;
        params["context"] = json!({ "includeDeclaration": true });
        let result = session.request("textDocument/references", params, REQUEST_TIMEOUT)?;
        Ok(parse_locations(&session, &result, MAX_LOCATIONS))
    }

    pub fn document_symbols(
        &self,
        app: &AppHandle,
        path: &str,
        text: &str,
    ) -> Result<Vec<LanguageSymbol>, String> {
        let session = self.ensure_synced_document(app, path, text)?;
        let result = session.request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": session.document_uri(path)? } }),
            REQUEST_TIMEOUT,
        )?;
        Ok(parse_symbols(&result, MAX_SYMBOLS))
    }

    pub fn stop(&self) {
        let sessions = self
            .sessions
            .lock()
            .map(|mut registry| registry.drain())
            .unwrap_or_default();
        for session in sessions {
            crate::code_library::clear_project(&session.project);
            session.shutdown();
        }
    }

    fn ensure_synced_document(
        &self,
        app: &AppHandle,
        path: &str,
        text: &str,
    ) -> Result<Arc<SessionInner>, String> {
        let language = read_active_code_document(path)
            .map_err(|error| error.to_string())?
            .language;
        validate_document_payload(path, &language, text)?;
        let session = self.ensure_session(app, &language)?;
        session.sync_document(path, &language, text)?;
        Ok(session)
    }

    fn ensure_session(&self, app: &AppHandle, language: &str) -> Result<Arc<SessionInner>, String> {
        let project = get_active_project().map_err(|error| error.to_string())?;
        let canonical_root = project
            .path
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let snapshot =
            active_language_intelligence_snapshot().map_err(|error| error.to_string())?;
        let server = preferred_server_for_language(&snapshot, language)
            .ok_or_else(|| format!("No language server is registered for {language}"))?;
        if server.profile_state != LanguageServerProfileState::Active {
            return Err(format!(
                "{} is discoverable but live support is not enabled",
                server.label
            ));
        }
        if server.availability != LanguageServerAvailability::Available {
            return Err(format!(
                "{} is not available for the active project",
                server.label
            ));
        }

        let key = SessionKey::new(&project.name, &server.id);
        let (stale, existing) = {
            let mut registry = self
                .sessions
                .lock()
                .map_err(|_| "Language server manager lock is poisoned".to_string())?;
            let stale = registry.remove_other_projects(&project.name);
            let existing = registry
                .get(&key)
                .filter(|existing| existing.root == canonical_root && existing.is_ready())
                .cloned();
            (stale, existing)
        };
        for session in stale {
            crate::code_library::clear_project(&session.project);
            session.shutdown();
        }
        if let Some(existing) = existing {
            return Ok(existing);
        }

        let mut registry = self
            .sessions
            .lock()
            .map_err(|_| "Language server manager lock is poisoned".to_string())?;
        if let Some(existing) = registry
            .get(&key)
            .filter(|existing| existing.root == canonical_root && existing.is_ready())
        {
            return Ok(existing.clone());
        }
        let session = SessionInner::start(
            app.clone(),
            project.name,
            canonical_root,
            &server.id,
            &server.executable,
            &server.arguments,
            server.initialization_profile,
        )?;
        registry.insert(key, session.clone());
        Ok(session)
    }
}

impl SessionInner {
    fn start(
        app: AppHandle,
        project: String,
        root: PathBuf,
        server_id: &str,
        executable: &str,
        arguments: &[String],
        initialization_profile: LanguageServerInitializationProfile,
    ) -> Result<Arc<Self>, String> {
        let root_uri = file_uri(&root)?;
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("Failed to start {server_id}: {error}"))?;
        let pid = child.id();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("{server_id} stdin was not captured"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("{server_id} stdout was not captured"))?;
        let stderr = child.stderr.take();

        let session = Arc::new(Self {
            project,
            root,
            root_uri,
            server_id: server_id.to_string(),
            pid,
            started_at: Utc::now(),
            state: Mutex::new(SessionState {
                state: LanguageServerSessionState::Starting,
                last_error: None,
            }),
            writer: Mutex::new(stdin),
            child: Mutex::new(child),
            stderr_tail: Mutex::new(String::new()),
            stopping: AtomicBool::new(false),
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            documents: Mutex::new(HashMap::new()),
            document_sync: Mutex::new(()),
        });

        let (stderr_done_sender, stderr_done_receiver) = mpsc::channel();
        if let Some(stderr) = stderr {
            spawn_stderr_reader(Arc::downgrade(&session), stderr, stderr_done_sender);
        } else {
            drop(stderr_done_sender);
        }
        spawn_stdout_reader(
            Arc::downgrade(&session),
            app.clone(),
            stdout,
            stderr_done_receiver,
        );

        let initialize = session.request(
            "initialize",
            json!({
                "processId": Value::Null,
                "clientInfo": { "name": "RepoDesk", "version": "0.1.0" },
                "rootUri": session.root_uri.clone(),
                "initializationOptions": initialization_options(initialization_profile),
                "capabilities": {
                    "workspace": {
                        "workspaceFolders": true,
                        "configuration": true
                    },
                    "textDocument": {
                        "publishDiagnostics": {
                            "relatedInformation": true,
                            "versionSupport": true,
                            "codeDescriptionSupport": true,
                            "dataSupport": true
                        },
                        "hover": { "contentFormat": ["markdown", "plaintext"] },
                        "definition": { "linkSupport": true },
                        "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
                    }
                },
                "workspaceFolders": [{
                    "uri": session.root_uri.clone(),
                    "name": session.project.clone()
                }]
            }),
            INITIALIZE_TIMEOUT,
        );

        if let Err(error) = initialize {
            session.set_error(error.clone());
            return Err(error);
        }

        session.notify("initialized", json!({}))?;
        session.set_ready();
        let _ = app.emit("language-server-status", session.status());
        Ok(session)
    }

    fn is_ready(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.state == LanguageServerSessionState::Ready)
            .unwrap_or(false)
    }

    fn status(&self) -> LanguageServerStatus {
        let (state, last_error) = self
            .state
            .lock()
            .map(|state| (state.state, state.last_error.clone()))
            .unwrap_or((
                LanguageServerSessionState::Error,
                Some("Language server state lock is poisoned".into()),
            ));
        let open_documents = self.documents.lock().map(|docs| docs.len()).unwrap_or(0);
        LanguageServerStatus {
            project: self.project.clone(),
            server_id: self.server_id.clone(),
            state,
            pid: self.pid,
            open_documents,
            started_at: self.started_at,
            last_error,
        }
    }

    fn set_ready(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.state = LanguageServerSessionState::Ready;
            state.last_error = None;
        }
    }

    fn set_error(&self, error: String) {
        if let Ok(mut state) = self.state.lock() {
            state.state = LanguageServerSessionState::Error;
            state.last_error = Some(truncate_chars(&error, MAX_SERVER_ERROR_CHARS));
        }
        fail_pending(self, error);
    }

    fn record_stderr(&self, line: &str) {
        if let Ok(mut tail) = self.stderr_tail.lock() {
            append_bounded_stderr(&mut tail, line, MAX_SERVER_ERROR_CHARS);
        }
    }

    fn stream_error(&self, protocol_error: Option<&str>) -> Option<String> {
        let status = self
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok().flatten())
            .map(|status| status.to_string());
        let stderr = self
            .stderr_tail
            .lock()
            .map(|tail| tail.clone())
            .unwrap_or_default();
        language_server_stream_error(
            self.stopping.load(Ordering::Acquire),
            &self.server_id,
            protocol_error,
            status.as_deref(),
            &stderr,
        )
    }

    fn sync_document(&self, path: &str, language: &str, text: &str) -> Result<(), String> {
        let _sync = self
            .document_sync
            .lock()
            .map_err(|_| "Language document sync lock is poisoned".to_string())?;
        let uri = self.document_uri(path)?;
        let mut documents = self
            .documents
            .lock()
            .map_err(|_| "Language document registry lock is poisoned".to_string())?;

        if let Some(version) = documents.get_mut(path) {
            *version = version.saturating_add(1);
            self.notify(
                "textDocument/didChange",
                json!({
                    "textDocument": { "uri": uri, "version": *version },
                    "contentChanges": [{ "text": text }]
                }),
            )?;
        } else {
            documents.insert(path.to_string(), 1);
            self.notify(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": language,
                        "version": 1,
                        "text": text
                    }
                }),
            )?;
        }
        Ok(())
    }

    fn close_document(&self, path: &str) -> Result<(), String> {
        let uri = self.document_uri(path)?;
        let mut documents = self
            .documents
            .lock()
            .map_err(|_| "Language document registry lock is poisoned".to_string())?;
        if documents.remove(path).is_none() {
            return Ok(());
        }
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        )
    }

    fn document_uri(&self, path: &str) -> Result<String, String> {
        let document = read_active_code_document(path).map_err(|error| error.to_string())?;
        let full = self.root.join(document.path);
        let canonical = full.canonicalize().map_err(|error| error.to_string())?;
        if !canonical.starts_with(&self.root) {
            return Err("Language server document escaped the active project".into());
        }
        file_uri(&canonical)
    }

    fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        if method != "initialize" && !self.is_ready() {
            return Err("Language server is not ready".into());
        }
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| "Language request registry lock is poisoned".to_string())?
            .insert(id, sender);

        if let Err(error) = self.write_json(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            return Err(error);
        }

        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                let _ = self.notify("$/cancelRequest", json!({ "id": id }));
                Err(format!("Language server request timed out: {method}"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
                "Language server disconnected while handling: {method}"
            )),
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write_json(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }))
    }

    fn write_json(&self, value: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| "Language server writer lock is poisoned".to_string())?;
        writer
            .write_all(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes())
            .and_then(|_| writer.write_all(&bytes))
            .and_then(|_| writer.flush())
            .map_err(|error| format!("Failed writing to {}: {error}", self.server_id))
    }

    fn respond(&self, id: Value, result: Result<Value, Value>) -> Result<(), String> {
        let message = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        self.write_json(&message)
    }

    fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        if self.is_ready() {
            let _ = self.request("shutdown", Value::Null, Duration::from_secs(2));
            let _ = self.notify("exit", Value::Null);
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn initialization_options(profile: LanguageServerInitializationProfile) -> Value {
    match profile {
        LanguageServerInitializationProfile::Default => Value::Null,
        LanguageServerInitializationProfile::Taplo => json!({}),
    }
}

fn validate_document_payload(path: &str, language: &str, text: &str) -> Result<(), String> {
    if text.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(format!(
            "Language document exceeds the {MAX_EDITABLE_FILE_BYTES} byte editor limit"
        ));
    }
    let document = read_active_code_document(path).map_err(|error| error.to_string())?;
    if document.language != language {
        return Err(format!(
            "Requested document language is {}, not {language}",
            document.language
        ));
    }
    Ok(())
}

fn position_params(
    session: &SessionInner,
    path: &str,
    line: u32,
    column: u32,
) -> Result<Value, String> {
    Ok(json!({
        "textDocument": { "uri": session.document_uri(path)? },
        "position": {
            "line": line.saturating_sub(1),
            "character": column.saturating_sub(1)
        }
    }))
}

fn spawn_stdout_reader(
    session: Weak<SessionInner>,
    app: AppHandle,
    stdout: impl Read + Send + 'static,
    stderr_done: mpsc::Receiver<()>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_json_rpc_frame(&mut reader) {
                Ok(Some(message)) => {
                    if let Some(session) = session.upgrade() {
                        handle_server_message(&session, &app, message);
                    } else {
                        break;
                    }
                }
                Ok(None) => {
                    if let Some(session) = session.upgrade() {
                        let _ = stderr_done.recv_timeout(Duration::from_millis(100));
                        if let Some(error) = session.stream_error(None) {
                            session.set_error(error);
                            let _ = app.emit("language-server-status", session.status());
                        }
                    }
                    break;
                }
                Err(error) => {
                    if let Some(session) = session.upgrade() {
                        let _ = stderr_done.recv_timeout(Duration::from_millis(100));
                        if let Some(error) = session.stream_error(Some(&error.to_string())) {
                            session.set_error(error);
                            let _ = app.emit("language-server-status", session.status());
                        }
                    }
                    break;
                }
            }
        }
    });
}

fn spawn_stderr_reader(
    session: Weak<SessionInner>,
    stderr: impl Read + Send + 'static,
    done: mpsc::Sender<()>,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let Some(session) = session.upgrade() else {
                break;
            };
            session.record_stderr(&line);
        }
        let _ = done.send(());
    });
}

fn read_json_rpc_frame(reader: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let length = content_length.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Missing Content-Length header",
        )
    })?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn handle_server_message(session: &Arc<SessionInner>, app: &AppHandle, message: Value) {
    if let Some(id) = message.get("id").and_then(Value::as_u64)
        && message.get("method").is_none()
    {
        let pending = session
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id));
        if let Some(sender) = pending {
            let result = if let Some(error) = message.get("error") {
                Err(error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Language server request failed")
                    .to_string())
            } else {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            };
            let _ = sender.send(result);
        }
        return;
    }

    if let (Some(id), Some(method)) = (
        message.get("id").cloned(),
        message.get("method").and_then(Value::as_str),
    ) {
        let result = handle_server_request(session, method, message.get("params"));
        let _ = session.respond(id, result);
        return;
    }

    if message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
        && let Some(event) = diagnostics_event(session, message.get("params"))
    {
        let _ = app.emit("language-diagnostics", event);
    }
}

fn handle_server_request(
    session: &SessionInner,
    method: &str,
    params: Option<&Value>,
) -> Result<Value, Value> {
    match method {
        "workspace/configuration" => {
            let count = params
                .and_then(|value| value.get("items"))
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            Ok(Value::Array(vec![Value::Null; count]))
        }
        "workspace/workspaceFolders" => Ok(json!([{
            "uri": session.root_uri.clone(),
            "name": session.project.clone()
        }])),
        "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create"
        | "workspace/semanticTokens/refresh"
        | "workspace/inlayHint/refresh"
        | "workspace/codeLens/refresh"
        | "workspace/diagnostic/refresh" => Ok(Value::Null),
        "window/showMessageRequest" => Ok(Value::Null),
        "workspace/applyEdit" => Ok(workspace_edit_rejection()),
        _ => Err(json!({
            "code": -32601,
            "message": format!("RepoDesk does not implement server request: {method}")
        })),
    }
}

fn workspace_edit_rejection() -> Value {
    json!({
        "applied": false,
        "failureReason": "RepoDesk does not allow language servers to mutate the workspace"
    })
}

fn diagnostics_event(
    session: &SessionInner,
    params: Option<&Value>,
) -> Option<LanguageDiagnosticsEvent> {
    let params = params?;
    let uri = params.get("uri")?.as_str()?;
    let path = session.relative_path_from_uri(uri)?;
    let diagnostics = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(MAX_DIAGNOSTICS_PER_FILE)
        .filter_map(|value| parse_diagnostic(&session.server_id, &path, value))
        .collect::<Vec<_>>();
    Some(LanguageDiagnosticsEvent {
        project: session.project.clone(),
        server_id: session.server_id.clone(),
        path,
        diagnostics,
    })
}

fn parse_diagnostic(server_id: &str, path: &str, value: &Value) -> Option<LanguageDiagnostic> {
    let range = parse_range(value.get("range")?)?;
    let severity = match value.get("severity").and_then(Value::as_u64).unwrap_or(3) {
        1 => LanguageDiagnosticSeverity::Error,
        2 => LanguageDiagnosticSeverity::Warning,
        4 => LanguageDiagnosticSeverity::Hint,
        _ => LanguageDiagnosticSeverity::Information,
    };
    let code = value.get("code").and_then(|code| match code {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    });
    Some(LanguageDiagnostic {
        server_id: server_id.to_string(),
        path: path.to_string(),
        range,
        severity,
        message: truncate_chars(value.get("message")?.as_str()?, MAX_SERVER_ERROR_CHARS),
        code,
        source: value
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn parse_range(value: &Value) -> Option<LspRange> {
    Some(LspRange {
        start: parse_position(value.get("start")?)?,
        end: parse_position(value.get("end")?)?,
    })
}

fn parse_position(value: &Value) -> Option<LspPosition> {
    Some(LspPosition {
        line: u32::try_from(value.get("line")?.as_u64()?).ok()?,
        character: u32::try_from(value.get("character")?.as_u64()?).ok()?,
    })
}

fn parse_hover(value: Value) -> Result<Option<LanguageHover>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let markdown = hover_contents(value.get("contents").unwrap_or(&Value::Null));
    if markdown.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(LanguageHover {
        markdown: truncate_chars(&markdown, MAX_HOVER_CHARS),
        range: value.get("range").and_then(parse_range),
    }))
}

fn hover_contents(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default(),
        Value::Array(values) => values
            .iter()
            .map(hover_contents)
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn parse_locations(session: &SessionInner, value: &Value, limit: usize) -> Vec<LanguageLocation> {
    let values = if value.is_array() {
        value.as_array().cloned().unwrap_or_default()
    } else if value.is_null() {
        Vec::new()
    } else {
        vec![value.clone()]
    };

    values
        .into_iter()
        .take(limit)
        .filter_map(|location| {
            if let Some(uri) = location.get("uri").and_then(Value::as_str) {
                let target = session.definition_target_from_uri(uri)?;
                let range = parse_range(location.get("range")?)?;
                return Some(location_from_range(target, range));
            }
            let uri = location.get("targetUri")?.as_str()?;
            let target = session.definition_target_from_uri(uri)?;
            let range = location
                .get("targetSelectionRange")
                .or_else(|| location.get("targetRange"))
                .and_then(parse_range)?;
            Some(location_from_range(target, range))
        })
        .collect()
}

fn location_from_range(target: DefinitionTarget, range: LspRange) -> LanguageLocation {
    LanguageLocation {
        path: target.path,
        library_handle: target.library_handle,
        read_only: target.read_only,
        line: range.start.line.saturating_add(1),
        column: range.start.character.saturating_add(1),
        end_line: range.end.line.saturating_add(1),
        end_column: range.end.character.saturating_add(1),
    }
}

fn parse_symbols(value: &Value, limit: usize) -> Vec<LanguageSymbol> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    for item in items {
        collect_symbol(item, None, &mut symbols, limit);
        if symbols.len() >= limit {
            break;
        }
    }
    symbols
}

fn collect_symbol(
    value: &Value,
    inherited_container: Option<&str>,
    output: &mut Vec<LanguageSymbol>,
    limit: usize,
) {
    if output.len() >= limit {
        return;
    }
    let Some(name) = value.get("name").and_then(Value::as_str) else {
        return;
    };
    let range = value
        .get("range")
        .and_then(parse_range)
        .or_else(|| value.get("location")?.get("range").and_then(parse_range));
    let Some(range) = range else {
        return;
    };
    let selection_range = value
        .get("selectionRange")
        .and_then(parse_range)
        .unwrap_or(range);
    let container_name = value
        .get("containerName")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| inherited_container.map(str::to_string));
    output.push(LanguageSymbol {
        name: name.to_string(),
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .map(str::to_string),
        kind: value
            .get("kind")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
        range,
        selection_range,
        container_name,
    });
    if let Some(children) = value.get("children").and_then(Value::as_array) {
        for child in children {
            collect_symbol(child, Some(name), output, limit);
            if output.len() >= limit {
                break;
            }
        }
    }
}

impl SessionInner {
    fn relative_path_from_uri(&self, uri: &str) -> Option<String> {
        let path = path_from_file_uri(uri)?;
        let canonical = path.canonicalize().ok()?;
        let relative = canonical.strip_prefix(&self.root).ok()?;
        Some(slash_path(relative))
    }

    fn definition_target_from_uri(&self, uri: &str) -> Option<DefinitionTarget> {
        if let Ok(definition) =
            crate::code_library::issue_definition(&self.project, &self.root, &self.server_id, uri)
        {
            return Some(DefinitionTarget {
                path: definition.display_path,
                library_handle: Some(definition.handle),
                read_only: true,
            });
        }
        self.relative_path_from_uri(uri)
            .map(|path| DefinitionTarget {
                path,
                library_handle: None,
                read_only: false,
            })
    }
}

fn file_uri(path: &Path) -> Result<String, String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    let mut display = canonical.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !display.starts_with('/') {
        display.insert(0, '/');
    }
    Ok(format!("file://{}", percent_encode_path(&display)))
}

fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let decoded = percent_decode(raw)?;
    let normalized =
        if cfg!(windows) && decoded.starts_with('/') && decoded.as_bytes().get(2) == Some(&b':') {
            decoded[1..].to_string()
        } else {
            decoded
        };
    Some(PathBuf::from(normalized))
}

fn percent_encode_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            decoded.push((hex_value(high)? << 4) | hex_value(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn fail_pending(session: &SessionInner, error: String) {
    if let Ok(mut pending) = session.pending.lock() {
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(error.clone()));
        }
    }
}

fn append_bounded_stderr(target: &mut String, line: &str, max_chars: usize) {
    if max_chars == 0 {
        target.clear();
        return;
    }
    if !target.is_empty() && !line.is_empty() {
        target.push('\n');
    }
    target.push_str(line);
    if target.chars().count() <= max_chars {
        return;
    }

    let suffix = target
        .chars()
        .rev()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    *target = format!("…{suffix}");
}

fn language_server_exit_error(server_id: &str, exit_status: Option<&str>, stderr: &str) -> String {
    let stderr = stderr.trim();
    if exit_status.is_none() && stderr.is_empty() {
        return format!("{server_id} closed its stdout stream");
    }

    let mut message = format!("{server_id} exited");
    if let Some(status) = exit_status {
        message.push_str(" with ");
        message.push_str(status);
    }
    if !stderr.is_empty() {
        message.push_str(": ");
        message.push_str(stderr);
    }
    if server_id == "rust-analyzer" && stderr.contains("unknown binary 'rust-analyzer'") {
        message.push_str("\nInstall it with: rustup component add rust-analyzer");
    }
    message
}

fn language_server_stream_error(
    stopping: bool,
    server_id: &str,
    protocol_error: Option<&str>,
    exit_status: Option<&str>,
    stderr: &str,
) -> Option<String> {
    if stopping {
        return None;
    }
    if let Some(protocol_error) = protocol_error {
        let mut message = format!("{server_id} protocol read failed: {protocol_error}");
        let stderr = stderr.trim();
        if !stderr.is_empty() {
            message.push_str(": ");
            message.push_str(stderr);
        }
        return Some(message);
    }
    Some(language_server_exit_error(server_id, exit_status, stderr))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut output = value.chars().take(max_chars).collect::<String>();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn session_registry_keeps_servers_isolated_and_evicts_other_projects() {
        let mut registry = SessionRegistry::default();
        registry.insert(SessionKey::new("alpha", "rust-analyzer"), "rust");
        registry.insert(
            SessionKey::new("alpha", "typescript-language-server"),
            "typescript",
        );

        assert_eq!(
            registry.get(&SessionKey::new("alpha", "rust-analyzer")),
            Some(&"rust")
        );
        assert_eq!(registry.len(), 2);

        let stale = registry.remove_other_projects("beta");
        assert_eq!(stale.len(), 2);
        assert!(registry.is_empty());

        registry.insert(SessionKey::new("beta", "taplo"), "toml");
        assert!(registry.remove_other_projects("beta").is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn taplo_initialization_profile_is_explicit() {
        assert_eq!(
            initialization_options(LanguageServerInitializationProfile::Default),
            Value::Null
        );
        assert_eq!(
            initialization_options(LanguageServerInitializationProfile::Taplo),
            json!({})
        );
    }

    #[test]
    fn json_rpc_frame_reader_obeys_content_length() {
        let body = br#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#;
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend_from_slice(body);
        let mut reader = BufReader::new(Cursor::new(frame));
        let parsed = read_json_rpc_frame(&mut reader)
            .expect("frame")
            .expect("message");
        assert_eq!(parsed["id"], 7);
        assert_eq!(parsed["result"]["ok"], true);
    }

    #[test]
    fn percent_encoding_round_trips_spaces_and_unicode() {
        let encoded = percent_encode_path("/tmp/repo desk/привіт.rs");
        assert!(encoded.contains("%20"));
        let decoded = percent_decode(&encoded).expect("decode");
        assert_eq!(decoded, "/tmp/repo desk/привіт.rs");
    }

    #[test]
    fn hover_content_accepts_markup_and_arrays() {
        let value = json!([
            { "language": "rust", "value": "fn demo()" },
            { "kind": "markdown", "value": "**docs**" }
        ]);
        assert_eq!(hover_contents(&value), "fn demo()\n\n**docs**");
    }

    #[test]
    fn lsp_ranges_stay_zero_based_until_ui_navigation() {
        let range = parse_range(&json!({
            "start": { "line": 4, "character": 2 },
            "end": { "line": 4, "character": 8 }
        }))
        .expect("range");
        let location = location_from_range(
            DefinitionTarget {
                path: "src/lib.rs".into(),
                library_handle: None,
                read_only: false,
            },
            range,
        );
        assert_eq!(location.line, 5);
        assert_eq!(location.column, 3);
        assert!(!location.read_only);
    }

    #[test]
    fn workspace_edits_are_never_accepted() {
        let result = workspace_edit_rejection();
        assert_eq!(result["applied"], false);
    }

    #[test]
    fn early_exit_reports_status_stderr_and_rustup_repair() {
        let error = language_server_exit_error(
            "rust-analyzer",
            Some("exit status: 1"),
            "error: unknown binary 'rust-analyzer' in toolchain 'stable'",
        );
        assert!(error.contains("exit status: 1"));
        assert!(error.contains("unknown binary 'rust-analyzer'"));
        assert!(error.contains("rustup component add rust-analyzer"));
    }

    #[test]
    fn stderr_tail_is_bounded() {
        let mut tail = String::new();
        append_bounded_stderr(&mut tail, "0123456789", 8);
        assert_eq!(tail.chars().count(), 8);
        assert!(tail.ends_with("3456789"));
    }

    #[test]
    fn exit_without_evidence_uses_generic_fallback() {
        assert_eq!(
            language_server_exit_error("rust-analyzer", None, ""),
            "rust-analyzer closed its stdout stream",
        );
    }

    #[test]
    fn intentional_shutdown_suppresses_stream_failures() {
        assert_eq!(
            language_server_stream_error(
                true,
                "rust-analyzer",
                Some("failed to fill whole buffer"),
                Some("signal: 9"),
                "server log",
            ),
            None,
        );
    }
}
