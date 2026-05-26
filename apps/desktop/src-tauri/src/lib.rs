mod commands {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WorkflowStep {
        pub id: String,
        pub title: String,
        pub description: String,
        pub status: String,
        pub action_id: Option<String>,
        pub artifact_kind: Option<String>,
        pub command_preview: Option<String>,
        pub blocker: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactStatus {
        pub kind: String,
        pub title: String,
        pub path: Option<String>,
        pub exists: bool,
        pub size_bytes: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProductWorkflowState {
        pub generated_at_ms: u128,
        pub overall_status: String,
        pub primary_cta: String,
        pub recommended_action_id: Option<String>,
        pub recommended_action_title: Option<String>,
        pub steps: Vec<WorkflowStep>,
        pub artifacts: Vec<ArtifactStatus>,
        pub project_ok: bool,
        pub task_ok: bool,
        pub context_ok: bool,
        pub smart_context_ok: bool,
        pub prompts_ok: bool,
        pub checks_ok: bool,
        pub safety_ok: bool,
        pub project_info: CommandResult,
        pub task_status: CommandResult,
        pub workflow_hint: CommandResult,
        pub security_verdict: CommandResult,
        pub checks_summary_preview: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactContent {
        pub kind: String,
        pub title: String,
        pub path: String,
        pub exists: bool,
        pub content: String,
        pub size_bytes: u64,
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
                description:
                    "Ask the workflow brain what should happen next based on current project state."
                        .into(),
                category: "Brain".into(),
                risk: "safe".into(),
                command_preview: "repodesk workflow next".into(),
                args: vec!["workflow".into(), "next".into()],
            },
            DesktopAction {
                id: "doctor-workflow".into(),
                title: "Run workflow doctor".into(),
                description: "Check project, task, context, prompts, checks, and guard state."
                    .into(),
                category: "Brain".into(),
                risk: "safe".into(),
                command_preview: "repodesk doctor workflow".into(),
                args: vec!["doctor".into(), "workflow".into()],
            },
            DesktopAction {
                id: "context-build".into(),
                title: "Build context".into(),
                description: "Build the full active task context.md artifact.".into(),
                category: "Context".into(),
                risk: "safe".into(),
                command_preview: "repodesk context build".into(),
                args: vec!["context".into(), "build".into()],
            },
            DesktopAction {
                id: "smart-context-build".into(),
                title: "Build smart context".into(),
                description: "Build a smaller task-focused context pack to reduce token waste."
                    .into(),
                category: "Context".into(),
                risk: "safe".into(),
                command_preview: "repodesk smart-context build".into(),
                args: vec!["smart-context".into(), "build".into()],
            },
            DesktopAction {
                id: "prompt-all".into(),
                title: "Generate agent prompts".into(),
                description: "Generate Codex, ChatGPT, and review prompts for the active task."
                    .into(),
                category: "Agents".into(),
                risk: "safe".into(),
                command_preview: "repodesk prompt all".into(),
                args: vec!["prompt".into(), "all".into()],
            },
            DesktopAction {
                id: "safety-scan-context".into(),
                title: "Scan context for secrets".into(),
                description:
                    "Scan active context for token/secret/password/private key risk signals.".into(),
                category: "Security".into(),
                risk: "safe".into(),
                command_preview: "repodesk safety scan-context".into(),
                args: vec!["safety".into(), "scan-context".into()],
            },
            DesktopAction {
                id: "security-audit".into(),
                title: "Run security audit".into(),
                description: "Show RepoDesk security policy and risk warnings.".into(),
                category: "Security".into(),
                risk: "safe".into(),
                command_preview: "repodesk security audit".into(),
                args: vec!["security".into(), "audit".into()],
            },
            DesktopAction {
                id: "judge-codex".into(),
                title: "Judge Codex readiness".into(),
                description: "Run guard, safety, and budget judgement for Codex.".into(),
                category: "Judge".into(),
                risk: "safe".into(),
                command_preview: "repodesk judge agent --agent codex".into(),
                args: vec![
                    "judge".into(),
                    "agent".into(),
                    "--agent".into(),
                    "codex".into(),
                ],
            },
            DesktopAction {
                id: "judge-chatgpt".into(),
                title: "Judge ChatGPT readiness".into(),
                description: "Run guard, safety, and budget judgement for ChatGPT.".into(),
                category: "Judge".into(),
                risk: "safe".into(),
                command_preview: "repodesk judge agent --agent chatgpt".into(),
                args: vec![
                    "judge".into(),
                    "agent".into(),
                    "--agent".into(),
                    "chatgpt".into(),
                ],
            },
            DesktopAction {
                id: "runtime-route-patch".into(),
                title: "Route patch work".into(),
                description: "Ask runtime router which provider should handle patch work.".into(),
                category: "Runtime".into(),
                risk: "safe".into(),
                command_preview: "repodesk runtime route --need patch".into(),
                args: vec![
                    "runtime".into(),
                    "route".into(),
                    "--need".into(),
                    "patch".into(),
                ],
            },
            DesktopAction {
                id: "runtime-route-compression".into(),
                title: "Route compression work".into(),
                description: "Ask runtime router which provider should compress context.".into(),
                category: "Runtime".into(),
                risk: "safe".into(),
                command_preview: "repodesk runtime route --need compression".into(),
                args: vec![
                    "runtime".into(),
                    "route".into(),
                    "--need".into(),
                    "compression".into(),
                ],
            },
            DesktopAction {
                id: "checks-run".into(),
                title: "Run configured checks".into(),
                description: "Run active project checks and save checks-summary.md.".into(),
                category: "Checks".into(),
                risk: "bounded-write".into(),
                command_preview: "repodesk checks run".into(),
                args: vec!["checks".into(), "run".into()],
            },
            DesktopAction {
                id: "git-audit".into(),
                title: "Audit git backup state".into(),
                description: "Review branch, remotes, and backup plan before pushing.".into(),
                category: "Backup".into(),
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
        let mut command = Command::new("cargo");
        command
            .arg("run")
            .arg("-q")
            .arg("-p")
            .arg("repodesk-cli")
            .arg("--")
            .args(args)
            .current_dir(&root);

        match command.output() {
            Ok(output) => CommandResult {
                ok: output.status.success(),
                command: format!("repodesk {}", args.join(" ")),
                stdout: truncate_text(&String::from_utf8_lossy(&output.stdout), 18_000),
                stderr: truncate_text(&String::from_utf8_lossy(&output.stderr), 18_000),
                exit_code: output.status.code(),
            },
            Err(error) => CommandResult {
                ok: false,
                command: format!("repodesk {}", args.join(" ")),
                stdout: String::new(),
                stderr: error.to_string(),
                exit_code: None,
            },
        }
    }

    fn run_cli_str(args: &[&str]) -> CommandResult {
        let owned = args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        run_cli(&owned)
    }

    fn save_action_history(result: &ActionRunResult) {
        let file = history_file();
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(line) = serde_json::to_string(result) {
            if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(file) {
                let _ = writeln!(handle, "{line}");
            }
        }
    }

    fn active_task_run_dir() -> Option<PathBuf> {
        repodesk_core::tasks::show_active_task()
            .ok()
            .map(|task| task.config.run_dir)
    }

    fn read_file_if_exists(path: &Path, max_chars: usize) -> Option<String> {
        fs::read_to_string(path)
            .ok()
            .map(|content| truncate_text(&content, max_chars))
    }

    fn artifact_path(kind: &str) -> Result<(String, PathBuf), String> {
        let run_dir = active_task_run_dir().ok_or_else(|| "Active task is not set".to_string())?;
        let (title, file_name) = match kind {
            "context" => ("Context", "context.md"),
            "smart_context" => ("Smart Context", "smart-context.md"),
            "prompt_codex" => ("Codex Prompt", "prompt.codex.md"),
            "prompt_chatgpt" => ("ChatGPT Prompt", "prompt.chatgpt.md"),
            "prompt_review" => ("Review Prompt", "prompt.review.md"),
            "checks_summary" => ("Checks Summary", "checks-summary.md"),
            "token_estimate" => ("Token Estimate", "token-estimate.txt"),
            _ => return Err(format!("Unsupported artifact kind: {kind}")),
        };

        Ok((title.to_string(), run_dir.join(file_name)))
    }

    fn artifact_status(kind: &str) -> ArtifactStatus {
        match artifact_path(kind) {
            Ok((title, path)) => {
                let metadata = fs::metadata(&path).ok();
                ArtifactStatus {
                    kind: kind.to_string(),
                    title,
                    path: Some(path.display().to_string()),
                    exists: metadata.is_some(),
                    size_bytes: metadata.map(|value| value.len()).unwrap_or_default(),
                }
            }
            Err(_) => ArtifactStatus {
                kind: kind.to_string(),
                title: kind.to_string(),
                path: None,
                exists: false,
                size_bytes: 0,
            },
        }
    }

    fn has_block_signal(result: &CommandResult) -> bool {
        let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
        text.contains("block") || text.contains("secret") || text.contains("private key")
    }

    fn has_warn_signal(result: &CommandResult) -> bool {
        let text = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
        text.contains("warn") || text.contains("warning") || text.contains("risk")
    }

    pub(crate) fn build_product_workflow_state() -> ProductWorkflowState {
        let generated_at_ms = now_ms();
        let project_info = run_cli_str(&["project", "info"]);
        let task_status = run_cli_str(&["task", "status"]);
        let workflow_hint = run_cli_str(&["workflow", "next"]);
        let security_verdict = run_cli_str(&["judge", "agent", "--agent", "codex"]);

        let context = artifact_status("context");
        let smart_context = artifact_status("smart_context");
        let prompt_codex = artifact_status("prompt_codex");
        let prompt_chatgpt = artifact_status("prompt_chatgpt");
        let prompt_review = artifact_status("prompt_review");
        let checks_summary = artifact_status("checks_summary");
        let token_estimate = artifact_status("token_estimate");

        let project_ok = project_info.ok;
        let task_ok = task_status.ok;
        let context_ok = context.exists;
        let smart_context_ok = smart_context.exists;
        let prompts_ok = prompt_codex.exists && prompt_chatgpt.exists && prompt_review.exists;
        let checks_ok = checks_summary.exists;
        let safety_ok = security_verdict.ok && !has_block_signal(&security_verdict);

        let mut recommended_action_id = None;
        let mut recommended_action_title = None;
        let mut primary_cta = "Review workflow".to_string();
        let mut overall_status = "ready".to_string();

        let mut choose = |action_id: &str, cta: &str, status: &str| {
            if let Some(action) = find_action(action_id) {
                recommended_action_id = Some(action.id);
                recommended_action_title = Some(action.title);
                primary_cta = cta.to_string();
                overall_status = status.to_string();
            }
        };

        if !project_ok {
            primary_cta = "Add or select a project".into();
            overall_status = "setup_required".into();
        } else if !task_ok {
            primary_cta = "Create an active task".into();
            overall_status = "setup_required".into();
        } else if !context_ok {
            choose("context-build", "Build context", "needs_context");
        } else if !smart_context_ok {
            choose(
                "smart-context-build",
                "Build smart context",
                "needs_smart_context",
            );
        } else if !safety_ok {
            choose(
                "safety-scan-context",
                "Scan context safety",
                "needs_safety_review",
            );
        } else if !prompts_ok {
            choose("prompt-all", "Generate prompts", "needs_prompts");
        } else if !checks_ok {
            choose("checks-run", "Run checks", "needs_checks");
        } else if has_warn_signal(&security_verdict) {
            choose("judge-codex", "Review AI readiness", "warning");
        } else {
            choose("workflow-next", "Ask brain for next step", "ready");
        }

        let steps = vec![
            WorkflowStep {
                id: "project".into(),
                title: "Project".into(),
                description:
                    "RepoDesk needs an active project before it can build context or run checks."
                        .into(),
                status: if project_ok { "done" } else { "current" }.into(),
                action_id: None,
                artifact_kind: None,
                command_preview: Some("repodesk project info".into()),
                blocker: if project_ok {
                    None
                } else {
                    Some("No active project".into())
                },
            },
            WorkflowStep {
                id: "task".into(),
                title: "Task".into(),
                description: "Every AI workflow should be scoped to one concrete task.".into(),
                status: if task_ok {
                    "done"
                } else if project_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: None,
                artifact_kind: None,
                command_preview: Some("repodesk task status".into()),
                blocker: if task_ok {
                    None
                } else {
                    Some("No active task".into())
                },
            },
            WorkflowStep {
                id: "context".into(),
                title: "Context".into(),
                description: "Build the base task context from project state and task metadata."
                    .into(),
                status: if context_ok {
                    "done"
                } else if task_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("context-build".into()),
                artifact_kind: Some("context".into()),
                command_preview: Some("repodesk context build".into()),
                blocker: if task_ok {
                    None
                } else {
                    Some("Requires active task".into())
                },
            },
            WorkflowStep {
                id: "smart_context".into(),
                title: "Smart Context".into(),
                description:
                    "Compress the working context so paid agents do not read unnecessary files."
                        .into(),
                status: if smart_context_ok {
                    "done"
                } else if context_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("smart-context-build".into()),
                artifact_kind: Some("smart_context".into()),
                command_preview: Some("repodesk smart-context build".into()),
                blocker: if context_ok {
                    None
                } else {
                    Some("Requires context.md".into())
                },
            },
            WorkflowStep {
                id: "safety".into(),
                title: "Safety".into(),
                description: "Judge the active context before handing it to AI.".into(),
                status: if safety_ok {
                    "done"
                } else if smart_context_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("safety-scan-context".into()),
                artifact_kind: None,
                command_preview: Some("repodesk safety scan-context".into()),
                blocker: if smart_context_ok {
                    None
                } else {
                    Some("Requires smart context".into())
                },
            },
            WorkflowStep {
                id: "prompts".into(),
                title: "Prompts".into(),
                description: "Generate bounded prompts for Codex, ChatGPT, and review.".into(),
                status: if prompts_ok {
                    "done"
                } else if safety_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("prompt-all".into()),
                artifact_kind: Some("prompt_codex".into()),
                command_preview: Some("repodesk prompt all".into()),
                blocker: if safety_ok {
                    None
                } else {
                    Some("Requires safety pass".into())
                },
            },
            WorkflowStep {
                id: "checks".into(),
                title: "Checks".into(),
                description:
                    "Run configured project checks and keep only the useful summary for AI.".into(),
                status: if checks_ok {
                    "done"
                } else if prompts_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("checks-run".into()),
                artifact_kind: Some("checks_summary".into()),
                command_preview: Some("repodesk checks run".into()),
                blocker: if prompts_ok {
                    None
                } else {
                    Some("Generate prompts first".into())
                },
            },
            WorkflowStep {
                id: "review".into(),
                title: "Review".into(),
                description:
                    "Review prompts, checks, and action history before using an external agent."
                        .into(),
                status: if project_ok && task_ok && smart_context_ok && prompts_ok {
                    "current"
                } else {
                    "blocked"
                }
                .into(),
                action_id: Some("judge-codex".into()),
                artifact_kind: Some("prompt_codex".into()),
                command_preview: Some("repodesk judge agent --agent codex".into()),
                blocker: None,
            },
        ];

        let checks_summary_preview = artifact_path("checks_summary")
            .ok()
            .and_then(|(_, path)| read_file_if_exists(&path, 5000));

        ProductWorkflowState {
            generated_at_ms,
            overall_status,
            primary_cta,
            recommended_action_id,
            recommended_action_title,
            steps,
            artifacts: vec![
                context,
                smart_context,
                prompt_codex,
                prompt_chatgpt,
                prompt_review,
                checks_summary,
                token_estimate,
            ],
            project_ok,
            task_ok,
            context_ok,
            smart_context_ok,
            prompts_ok,
            checks_ok,
            safety_ok,
            project_info,
            task_status,
            workflow_hint,
            security_verdict,
            checks_summary_preview,
        }
    }

    #[tauri::command]
    pub fn desktop_snapshot() -> serde_json::Value {
        json!({
            "mode": "desktop-product-workflow-mvp",
            "workspace_root": workspace_root(),
            "generated_at_ms": now_ms(),
            "actions": action_catalog(),
            "workflow_state": build_product_workflow_state(),
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
            "events": run_cli_str(&["events", "last", "--limit", "10"]),
            "knowledge": run_cli_str(&["knowledge", "show", "--kind", "decision"]),
        })
    }

    #[tauri::command]
    pub fn product_workflow_state() -> ProductWorkflowState {
        build_product_workflow_state()
    }

    #[tauri::command]
    pub fn read_artifact(kind: String) -> Result<ArtifactContent, String> {
        validate_short_id("Artifact kind", &kind)?;
        let (title, path) = artifact_path(kind.trim())?;
        let metadata = fs::metadata(&path).ok();
        let exists = metadata.is_some();
        let size_bytes = metadata.map(|value| value.len()).unwrap_or_default();
        let content = if exists {
            fs::read_to_string(&path).map_err(|error| error.to_string())?
        } else {
            String::new()
        };

        Ok(ArtifactContent {
            kind,
            title,
            path: path.display().to_string(),
            exists,
            content: truncate_text(&content, 70_000),
            size_bytes,
        })
    }

    #[tauri::command]
    pub fn desktop_actions() -> Vec<DesktopAction> {
        action_catalog()
    }

    #[tauri::command]
    pub fn explain_action(action_id: String) -> Result<String, String> {
        validate_short_id("Action id", &action_id)?;
        let action =
            find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
        Ok(format!(
            "{}\n\nCategory: {}\nRisk: {}\nCommand: {}\n\n{}\n\nThis action is whitelisted in Rust. The desktop UI cannot run arbitrary shell commands.",
            action.title, action.category, action.risk, action.command_preview, action.description
        ))
    }

    #[tauri::command]
    pub fn run_desktop_action(action_id: String) -> Result<ActionRunResult, String> {
        validate_short_id("Action id", &action_id)?;
        let action =
            find_action(&action_id).ok_or_else(|| format!("Unknown action: {action_id}"))?;
        let started_at_ms = now_ms();
        let result = run_cli(&action.args);
        let finished_at_ms = now_ms();

        let action_result = ActionRunResult {
            id: action.id,
            title: action.title,
            risk: action.risk,
            category: action.category,
            started_at_ms,
            finished_at_ms,
            result,
        };

        save_action_history(&action_result);
        Ok(action_result)
    }

    #[tauri::command]
    pub fn run_next_safe_step() -> Result<ActionRunResult, String> {
        let state = build_product_workflow_state();
        let action_id = state.recommended_action_id.ok_or_else(|| {
            "No runnable primary action. Add/select a project and create an active task first."
                .to_string()
        })?;
        run_desktop_action(action_id)
    }

    #[tauri::command]
    pub fn action_history() -> Vec<ActionRunResult> {
        let file = history_file();
        let Ok(content) = fs::read_to_string(file) else {
            return vec![];
        };

        content
            .lines()
            .rev()
            .take(50)
            .filter_map(|line| serde_json::from_str::<ActionRunResult>(line).ok())
            .collect()
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
        validate_short_id("Project type", &input.project_type)?;

        if let Some(language) = &input.main_language {
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
            let trimmed = language.trim();
            if !trimmed.is_empty() {
                args.push("--main-language".into());
                args.push(trimmed.into());
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
            commands::product_workflow_state,
            commands::read_artifact,
            commands::desktop_actions,
            commands::explain_action,
            commands::run_desktop_action,
            commands::run_next_safe_step,
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
            .any(|action| action.id == "smart-context-build"));
        assert!(actions
            .iter()
            .any(|action| action.id == "safety-scan-context"));
        assert!(actions.iter().any(|action| action.id == "prompt-all"));
        assert!(actions.iter().any(|action| action.id == "checks-run"));
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

    #[test]
    fn workflow_state_is_safe_to_build_without_panicking() {
        let state = commands::build_product_workflow_state();
        assert!(!state.primary_cta.trim().is_empty());
        assert!(!state.steps.is_empty());
    }
}
