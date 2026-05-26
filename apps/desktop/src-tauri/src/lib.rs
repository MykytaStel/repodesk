mod commands {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommandResult {
        pub ok: bool,
        pub command: String,
        pub stdout: String,
        pub stderr: String,
        pub exit_code: Option<i32>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DesktopAction {
        pub id: String,
        pub title: String,
        pub description: String,
        pub category: String,
        pub risk: String,
        pub command_preview: String,
        #[serde(skip_serializing)]
        pub args: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ActionRunResult {
        pub id: String,
        pub title: String,
        pub risk: String,
        pub category: String,
        pub started_at_ms: u128,
        pub finished_at_ms: u128,
        pub result: CommandResult,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProjectAddInput {
        pub name: String,
        pub path: String,
        pub project_type: String,
        pub main_language: Option<String>,
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }

    fn workspace_root() -> PathBuf {
        let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        for _ in 0..8 {
            if current.join("Cargo.toml").exists() && current.join("crates/repodesk-cli").exists() {
                return current;
            }

            if !current.pop() {
                break;
            }
        }

        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn home_dir() -> PathBuf {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(workspace_root)
    }

    fn history_file() -> PathBuf {
        home_dir()
            .join(".repodesk")
            .join("desktop")
            .join("action-history.jsonl")
    }

    fn truncate_text(value: &str, max_chars: usize) -> String {
        let char_count = value.chars().count();
        if char_count <= max_chars {
            return value.to_string();
        }

        let mut truncated: String = value.chars().take(max_chars).collect();
        truncated.push_str("\n\n[RepoDesk truncated output to keep the UI responsive]");
        truncated
    }

    pub(crate) fn validate_short_id(label: &str, value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(format!("{label} cannot be empty"));
        }

        if trimmed.len() > 80 {
            return Err(format!("{label} is too long"));
        }

        let safe = trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'));

        if !safe {
            return Err(format!(
                "{label} may only contain letters, numbers, dash, underscore, dot or slash"
            ));
        }

        Ok(())
    }

    pub(crate) fn validate_text(label: &str, value: &str, max_len: usize) -> Result<(), String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(format!("{label} cannot be empty"));
        }

        if trimmed.len() > max_len {
            return Err(format!("{label} is too long"));
        }

        if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
            return Err(format!("{label} contains unsupported characters"));
        }

        Ok(())
    }

    pub(crate) fn validate_path(value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("Path cannot be empty".into());
        }

        if trimmed.len() > 512 {
            return Err("Path is too long".into());
        }

        if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
            return Err("Path contains unsupported characters".into());
        }

        Ok(())
    }

    pub(crate) fn action_catalog() -> Vec<DesktopAction> {
        vec![
            DesktopAction {
                id: "workflow-next".into(),
                title: "Decide next workflow step".into(),
                description: "Ask the workflow brain what should happen next based on current project state.".into(),
                category: "Brain".into(),
                risk: "safe".into(),
                command_preview: "repodesk workflow next".into(),
                args: vec!["workflow".into(), "next".into()],
            },
            DesktopAction {
                id: "doctor-workflow".into(),
                title: "Run workflow doctor".into(),
                description: "Check project, task, context, prompts, checks, and guard state.".into(),
                category: "Brain".into(),
                risk: "safe".into(),
                command_preview: "repodesk doctor workflow".into(),
                args: vec!["doctor".into(), "workflow".into()],
            },
            DesktopAction {
                id: "context-build".into(),
                title: "Build context pack".into(),
                description: "Create the main task context pack for bounded agent work.".into(),
                category: "Context".into(),
                risk: "guarded".into(),
                command_preview: "repodesk context build".into(),
                args: vec!["context".into(), "build".into()],
            },
            DesktopAction {
                id: "smart-context-build".into(),
                title: "Build smart context".into(),
                description: "Create smaller token-aware context from active task, repo map, and changed files.".into(),
                category: "Context".into(),
                risk: "guarded".into(),
                command_preview: "repodesk smart-context build".into(),
                args: vec!["smart-context".into(), "build".into()],
            },
            DesktopAction {
                id: "prompt-all".into(),
                title: "Generate agent prompts".into(),
                description: "Generate Codex, ChatGPT, and review prompts from the current context.".into(),
                category: "Context".into(),
                risk: "guarded".into(),
                command_preview: "repodesk prompt all".into(),
                args: vec!["prompt".into(), "all".into()],
            },
            DesktopAction {
                id: "safety-scan-context".into(),
                title: "Safety scan context".into(),
                description: "Scan context for secret-like patterns before sending it to any AI system.".into(),
                category: "Security".into(),
                risk: "safe".into(),
                command_preview: "repodesk safety scan-context".into(),
                args: vec!["safety".into(), "scan-context".into()],
            },
            DesktopAction {
                id: "security-audit".into(),
                title: "Security audit".into(),
                description: "Show security policy and blocked or guarded behavior.".into(),
                category: "Security".into(),
                risk: "safe".into(),
                command_preview: "repodesk security audit".into(),
                args: vec!["security".into(), "audit".into()],
            },
            DesktopAction {
                id: "judge-codex".into(),
                title: "Judge Codex".into(),
                description: "Ask the judge whether Codex should be allowed for the current task.".into(),
                category: "Agent Judge".into(),
                risk: "guarded".into(),
                command_preview: "repodesk judge agent --agent codex".into(),
                args: vec!["judge".into(), "agent".into(), "--agent".into(), "codex".into()],
            },
            DesktopAction {
                id: "judge-chatgpt".into(),
                title: "Judge ChatGPT".into(),
                description: "Ask the judge whether ChatGPT should be allowed for the current task.".into(),
                category: "Agent Judge".into(),
                risk: "expensive".into(),
                command_preview: "repodesk judge agent --agent chatgpt".into(),
                args: vec!["judge".into(), "agent".into(), "--agent".into(), "chatgpt".into()],
            },
            DesktopAction {
                id: "runtime-route-patch".into(),
                title: "Route patch work".into(),
                description: "Ask runtime router which provider should handle patch-oriented work.".into(),
                category: "Runtime".into(),
                risk: "safe".into(),
                command_preview: "repodesk runtime route --need patch".into(),
                args: vec!["runtime".into(), "route".into(), "--need".into(), "patch".into()],
            },
            DesktopAction {
                id: "runtime-route-compression".into(),
                title: "Route compression work".into(),
                description: "Ask runtime router which provider should compress or summarize context.".into(),
                category: "Runtime".into(),
                risk: "safe".into(),
                command_preview: "repodesk runtime route --need compression".into(),
                args: vec!["runtime".into(), "route".into(), "--need".into(), "compression".into()],
            },
            DesktopAction {
                id: "checks-run".into(),
                title: "Run configured checks".into(),
                description: "Run configured project checks and produce a compact summary for agents.".into(),
                category: "Verification".into(),
                risk: "guarded".into(),
                command_preview: "repodesk checks run".into(),
                args: vec!["checks".into(), "run".into()],
            },
            DesktopAction {
                id: "git-audit".into(),
                title: "Audit git state".into(),
                description: "Show local git status, branch, remotes, and backup readiness.".into(),
                category: "Verification".into(),
                risk: "safe".into(),
                command_preview: "repodesk git audit".into(),
                args: vec!["git".into(), "audit".into()],
            },
        ]
    }

    pub(crate) fn find_action(action_id: &str) -> Option<DesktopAction> {
        action_catalog()
            .into_iter()
            .find(|action| action.id == action_id)
    }

    fn run_cli(args: &[String]) -> CommandResult {
        let root = workspace_root();
        let command_preview = format!("cargo run -q -p repodesk-cli -- {}", args.join(" "));

        let output = Command::new("cargo")
            .arg("run")
            .arg("-q")
            .arg("-p")
            .arg("repodesk-cli")
            .arg("--")
            .args(args)
            .current_dir(&root)
            .output();

        match output {
            Ok(output) => CommandResult {
                ok: output.status.success(),
                command: command_preview,
                stdout: truncate_text(&String::from_utf8_lossy(&output.stdout), 12_000),
                stderr: truncate_text(&String::from_utf8_lossy(&output.stderr), 12_000),
                exit_code: output.status.code(),
            },
            Err(error) => CommandResult {
                ok: false,
                command: command_preview,
                stdout: String::new(),
                stderr: format!("Failed to run CLI command: {error}"),
                exit_code: None,
            },
        }
    }

    fn run_cli_str(args: &[&str]) -> CommandResult {
        let owned: Vec<String> = args.iter().map(|item| item.to_string()).collect();
        run_cli(&owned)
    }

    fn append_history(result: &ActionRunResult) -> Result<(), String> {
        let path = history_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let line = serde_json::to_string(result).map_err(|error| error.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| error.to_string())?;

        writeln!(file, "{line}").map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn desktop_actions() -> Vec<DesktopAction> {
        action_catalog()
    }

    #[tauri::command]
    pub fn explain_action(action_id: String) -> Result<String, String> {
        let action =
            find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
        Ok(format!(
            "{}\n\nRisk: {}\nCategory: {}\nCommand: {}\n\n{}",
            action.title, action.risk, action.category, action.command_preview, action.description
        ))
    }

    #[tauri::command]
    pub fn desktop_snapshot() -> serde_json::Value {
        json!({
            "mode": "desktop-management-cockpit",
            "workspace_root": workspace_root().display().to_string(),
            "generated_at_ms": now_ms(),
            "actions": action_catalog(),
            "dashboard": run_cli_str(&["dashboard", "summary"]),
            "workflow": run_cli_str(&["workflow", "next"]),
            "doctor": run_cli_str(&["doctor", "workflow"]),
            "security": run_cli_str(&["security", "audit"]),
            "runtime": run_cli_str(&["runtime", "providers"]),
            "git": run_cli_str(&["git", "audit"]),
            "project_info": run_cli_str(&["project", "info"]),
            "project_list": run_cli_str(&["project", "list"]),
            "task_status": run_cli_str(&["task", "status"]),
            "task_show": run_cli_str(&["task", "show"]),
            "events": run_cli_str(&["events", "last", "--limit", "5"]),
            "knowledge": run_cli_str(&["knowledge", "show", "--kind", "decision"]),
        })
    }

    #[tauri::command]
    pub fn run_desktop_action(action_id: String) -> Result<ActionRunResult, String> {
        let action =
            find_action(&action_id).ok_or_else(|| format!("Action is not allowed: {action_id}"))?;
        let started_at_ms = now_ms();
        let result = run_cli(&action.args);
        let finished_at_ms = now_ms();

        let run_result = ActionRunResult {
            id: action.id,
            title: action.title,
            risk: action.risk,
            category: action.category,
            started_at_ms,
            finished_at_ms,
            result,
        };

        append_history(&run_result)?;
        Ok(run_result)
    }

    #[tauri::command]
    pub fn action_history() -> Vec<ActionRunResult> {
        let path = history_file();
        let Ok(content) = fs::read_to_string(path) else {
            return Vec::new();
        };

        let mut values: Vec<ActionRunResult> = content
            .lines()
            .filter_map(|line| serde_json::from_str::<ActionRunResult>(line).ok())
            .collect();

        values.reverse();
        values.truncate(50);
        values
    }

    #[tauri::command]
    pub fn project_info() -> CommandResult {
        run_cli_str(&["project", "info"])
    }

    #[tauri::command]
    pub fn project_list() -> CommandResult {
        run_cli_str(&["project", "list"])
    }

    #[tauri::command]
    pub fn project_use(name: String) -> Result<CommandResult, String> {
        validate_short_id("Project name", &name)?;
        Ok(run_cli(&vec![
            "project".into(),
            "use".into(),
            name.trim().into(),
        ]))
    }

    #[tauri::command]
    pub fn project_add(input: ProjectAddInput) -> Result<CommandResult, String> {
        validate_short_id("Project name", &input.name)?;
        validate_path(&input.path)?;
        validate_text("Project type", &input.project_type, 80)?;

        if let Some(language) = input.main_language.as_deref() {
            if !language.trim().is_empty() {
                validate_short_id("Main language", language)?;
            }
        }

        let mut args = vec![
            "project".into(),
            "add".into(),
            input.name.trim().into(),
            input.path.trim().into(),
            "--type".into(),
            input.project_type.trim().into(),
        ];

        if let Some(language) = input.main_language {
            let language = language.trim().to_string();
            if !language.is_empty() {
                args.push("--main-language".into());
                args.push(language);
            }
        }

        Ok(run_cli(&args))
    }

    #[tauri::command]
    pub fn task_new(title: String) -> Result<CommandResult, String> {
        validate_text("Task title", &title, 180)?;
        Ok(run_cli(&vec![
            "task".into(),
            "new".into(),
            title.trim().into(),
        ]))
    }

    #[tauri::command]
    pub fn task_status() -> CommandResult {
        run_cli_str(&["task", "status"])
    }

    #[tauri::command]
    pub fn task_show() -> CommandResult {
        run_cli_str(&["task", "show"])
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::desktop_snapshot,
            commands::desktop_actions,
            commands::explain_action,
            commands::run_desktop_action,
            commands::action_history,
            commands::project_info,
            commands::project_list,
            commands::project_use,
            commands::project_add,
            commands::task_new,
            commands::task_status,
            commands::task_show
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::commands;

    #[test]
    fn action_catalog_contains_core_workflow_actions() {
        let actions = commands::action_catalog();
        assert!(actions.iter().any(|action| action.id == "workflow-next"));
        assert!(actions.iter().any(|action| action.id == "context-build"));
        assert!(actions
            .iter()
            .any(|action| action.id == "safety-scan-context"));
        assert!(actions.iter().any(|action| action.id == "prompt-all"));
    }

    #[test]
    fn unknown_actions_are_not_allowed() {
        assert!(commands::find_action("rm-rf-root").is_none());
        assert!(commands::find_action("curl-pipe-shell").is_none());
        assert!(commands::find_action("unrestricted-shell").is_none());
    }

    #[test]
    fn management_validation_blocks_newlines() {
        assert!(commands::validate_text("Task title", "safe title", 80).is_ok());
        assert!(commands::validate_text("Task title", "bad\nnext", 80).is_err());
        assert!(commands::validate_path("/tmp/project").is_ok());
        assert!(commands::validate_path("/tmp/project\nrm -rf").is_err());
    }

    #[test]
    fn project_name_validation_is_conservative() {
        assert!(commands::validate_short_id("Project", "repodesk").is_ok());
        assert!(commands::validate_short_id("Project", "repo desk").is_err());
        assert!(commands::validate_short_id("Project", "repo;rm").is_err());
    }
}
