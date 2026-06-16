pub mod ai_discovery_commands {
    #[tauri::command]
    pub fn ai_discovery_scan() -> Result<repodesk_core::ai_discovery::AiDiscoveryReport, String> {
        repodesk_core::ai_discovery::write_ai_discovery_report().map_err(|error| error.to_string())
    }
}

pub mod commands;
mod store;

mod git_workspace_commands {
    #[tauri::command]
    pub fn git_workspace_snapshot()
    -> Result<repodesk_core::git_workspace::GitWorkspaceSnapshot, String> {
        Ok(repodesk_core::git_workspace::build_git_workspace_snapshot())
    }
}

mod code_workbench_commands {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const MAX_PREVIEW_BYTES: u64 = 80_000;
    const MAX_SAFE_PREVIEW_BYTES: u64 = 160_000;
    const MAX_PREVIEW_CHARS: usize = 4_000;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodeFilePreview {
        pub path: String,
        pub status: String,
        pub bytes: u64,
        pub blocked: bool,
        pub reason: Option<String>,
        pub preview: Option<String>,
    }

    fn active_project_path() -> Result<PathBuf, String> {
        repodesk_core::projects::get_active_project()
            .map(|project| project.path)
            .map_err(|error| error.to_string())
    }

    fn run_git(project_path: &Path, args: &[&str]) -> String {
        Command::new("git")
            .args(args)
            .current_dir(project_path)
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if stdout.trim().is_empty() {
                    stderr
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|error| format!("git command failed: {error}"))
    }

    fn parse_status(project_path: &Path) -> Vec<(String, String)> {
        run_git(project_path, &["status", "--porcelain=v1"])
            .lines()
            .filter_map(|line| {
                if line.len() < 4 {
                    return None;
                }
                let status = line.chars().take(2).collect::<String>();
                let mut path = line.chars().skip(3).collect::<String>();
                if let Some((_, after)) = path.split_once(" -> ") {
                    path = after.to_string();
                }
                Some((path.trim().to_string(), status.trim().to_string()))
            })
            .collect()
    }

    fn safe_preview(project_path: &Path, relative_path: &str, status: &str) -> CodeFilePreview {
        if let Some(reason) = repodesk_core::security::is_blocked_path(relative_path) {
            return CodeFilePreview {
                path: relative_path.into(),
                status: status.into(),
                bytes: 0,
                blocked: true,
                reason: Some(reason),
                preview: None,
            };
        }

        let full_path = project_path.join(relative_path);
        let metadata = match fs::metadata(&full_path) {
            Ok(value) => value,
            Err(error) => {
                return CodeFilePreview {
                    path: relative_path.into(),
                    status: status.into(),
                    bytes: 0,
                    blocked: true,
                    reason: Some(error.to_string()),
                    preview: None,
                };
            }
        };

        if metadata.len() > MAX_PREVIEW_BYTES {
            return CodeFilePreview {
                path: relative_path.into(),
                status: status.into(),
                bytes: metadata.len(),
                blocked: true,
                reason: Some("file is too large for UI preview".into()),
                preview: None,
            };
        }

        match fs::read_to_string(&full_path) {
            Ok(content) => {
                let preview: String = content.chars().take(MAX_PREVIEW_CHARS).collect();
                CodeFilePreview {
                    path: relative_path.into(),
                    status: status.into(),
                    bytes: metadata.len(),
                    blocked: false,
                    reason: None,
                    preview: Some(preview),
                }
            }
            Err(error) => CodeFilePreview {
                path: relative_path.into(),
                status: status.into(),
                bytes: metadata.len(),
                blocked: true,
                reason: Some(error.to_string()),
                preview: None,
            },
        }
    }

    #[tauri::command]
    pub fn code_workbench_snapshot() -> serde_json::Value {
        let project_path = match active_project_path() {
            Ok(path) => path,
            Err(error) => {
                return json!({
                    "connected": false,
                    "error": error,
                    "changed_files": [],
                    "previews": [],
                });
            }
        };

        let status_items = parse_status(&project_path);
        let changed_files: Vec<String> =
            status_items.iter().map(|(path, _)| path.clone()).collect();
        let previews: Vec<CodeFilePreview> = status_items
            .iter()
            .take(30)
            .map(|(path, status)| safe_preview(&project_path, path, status))
            .collect();

        json!({
            "connected": true,
            "project_path": project_path.display().to_string(),
            "changed_files": changed_files,
            "previews": previews,
            "diff_stat": run_git(&project_path, &["diff", "--stat"]),
            "cached_diff_stat": run_git(&project_path, &["diff", "--cached", "--stat"]),
            "recommendation": if status_items.is_empty() { "Workspace is clean. Create or select a task, then build context." } else { "Review changed files, build smart context, then run checks before asking an agent." },
        })
    }

    #[tauri::command]
    pub fn read_code_file(relative_path: String) -> Result<serde_json::Value, String> {
        if relative_path.trim().is_empty()
            || relative_path.contains("..")
            || Path::new(&relative_path).is_absolute()
        {
            return Err("Unsafe relative path".into());
        }
        if let Some(reason) = repodesk_core::security::is_blocked_path(&relative_path) {
            return Err(reason);
        }

        let project_path = active_project_path()?;
        let project_root = project_path
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let file_path = project_root.join(&relative_path);
        let canonical_file = file_path
            .canonicalize()
            .map_err(|error| error.to_string())?;

        if !canonical_file.starts_with(&project_root) {
            return Err("Path escapes active project".into());
        }

        let metadata = fs::metadata(&canonical_file).map_err(|error| error.to_string())?;
        if metadata.len() > MAX_SAFE_PREVIEW_BYTES {
            return Err("File is too large for safe UI preview".into());
        }
        let content = fs::read_to_string(&canonical_file).map_err(|error| error.to_string())?;
        Ok(json!({
            "path": relative_path,
            "bytes": metadata.len(),
            "content": content,
        }))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Auto-updater: enabled with a real signing key + GitHub Releases endpoint
        // (see tauri.conf.json plugins.updater). The plugin only verifies/installs
        // signed update bundles; it is not triggered automatically on launch.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            code_workbench_commands::read_code_file,
            code_workbench_commands::code_workbench_snapshot,
            git_workspace_commands::git_workspace_snapshot,
            ai_discovery_commands::ai_discovery_scan,
            commands::desktop_snapshot,
            commands::product_workflow_state,
            commands::read_artifact,
            commands::desktop_actions,
            commands::explain_action,
            commands::run_desktop_action,
            commands::run_next_safe_step,
            commands::action_history,
            commands::db_status,
            commands::provider_settings,
            commands::save_provider_settings,
            commands::project_info,
            commands::project_list,
            commands::project_use,
            commands::project_add,
            commands::task_new,
            commands::task_status,
            commands::task_show,
            commands::token_usage_snapshot,
            commands::log_token_usage,
            commands::estimate_raw_text,
            commands::model_health_snapshot,
            commands::refresh_model_health,
            commands::routing_decision,
            commands::routing_snapshot,
            commands::get_active_project_config,
            commands::save_project_ignore_rules,
            commands::get_project_file_token_estimates,
            commands::get_api_env_diagnostic,
            commands::save_codex_quota_status,
            commands::get_system_agents,
            commands::get_system_capabilities,
            commands::get_system_peripherals,
            commands::get_system_modules,
            commands::get_event_journal,
            commands::log_ui_event,
            commands::memory_add,
            commands::memory_list,
            commands::memory_consolidate,
            commands::memory_search,
            commands::memory_update,
            commands::memory_delete,
            commands::memory_set_pinned,
            commands::memory_set_status,
            commands::memory_brain_preview,
            commands::memory_capture,
            commands::memory_scan,
            commands::memory_proposals_list,
            commands::memory_proposal_accept,
            commands::memory_proposal_reject,
            commands::memory_reconcile_conflict,
            commands::orchestrate_plan,
            commands::orchestrate_run,
            commands::orchestrate_status,
            commands::orchestrate_show,
            commands::paid_agent_gate,
            commands::commit_ready_changes,
            commands::repopilot_findings,
            commands::backup_state,
            commands::restore_state
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
        assert!(
            actions
                .iter()
                .any(|action| action.id == "smart-context-build")
        );
        assert!(
            actions
                .iter()
                .any(|action| action.id == "safety-scan-context")
        );
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

    #[test]
    fn disabled_model_health_does_not_probe_network() {
        let settings = crate::store::ProviderSettings {
            ollama_enabled: false,
            lm_studio_enabled: false,
            openai_api_enabled: false,
            gemini_api_enabled: false,
            ..crate::store::ProviderSettings::default()
        };

        let snapshot = commands::model_health_from_settings(&settings);

        assert_eq!(snapshot.providers.len(), 6);
        assert!(
            snapshot
                .providers
                .iter()
                .all(|provider| provider.reachability == "disabled")
        );
        assert!(
            snapshot
                .warnings
                .iter()
                .any(|warning| warning.contains("No enabled model provider"))
        );
    }

    #[test]
    fn run_cli_dispatches_allowed_command() {
        // `run_cli` installs a process-global stdout override to capture CLI
        // output. Under libtest's parallel harness that capture races with other
        // tests, so the *content* of stdout is not a reliable invariant here
        // (it can pick up another thread's output). The stable contract is that
        // an allowlisted subcommand dispatches successfully.
        let result = commands::run_cli(&["project".into(), "list".into()]);
        assert!(result.ok, "allowed command should dispatch: {result:?}");
    }

    #[test]
    fn run_cli_rejects_unapproved_commands() {
        let result = commands::run_cli(&["rm".into(), "-rf".into(), "/".into()]);
        assert!(!result.ok);
        assert!(
            result
                .stderr
                .contains("Blocked: Subcommand 'rm' is not registered or allowed.")
        );
    }
}
