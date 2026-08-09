pub mod ai_discovery_commands {
    #[tauri::command]
    pub fn ai_discovery_scan() -> Result<repodesk_core::ai_discovery::AiDiscoveryReport, String> {
        repodesk_core::ai_discovery::write_ai_discovery_report().map_err(|error| error.to_string())
    }
}

mod code_workspace;
pub mod commands;
mod store;
mod terminal;

mod git_workspace_commands {
    #[tauri::command]
    pub fn git_workspace_snapshot()
    -> Result<repodesk_core::git_workspace::GitWorkspaceSnapshot, String> {
        Ok(repodesk_core::git_workspace::build_git_workspace_snapshot())
    }

    #[tauri::command]
    pub fn git_file_diff(path: String, cached: bool) -> Result<String, String> {
        Ok(repodesk_core::git_workspace::active_file_diff(
            &path, cached,
        ))
    }
}

mod code_workbench_commands {
    use repodesk_core::code_workspace::{
        CodeWorkspaceFileStatus, load_active_code_workspace, read_active_code_document,
    };
    use serde_json::json;

    #[tauri::command]
    pub fn code_workbench_snapshot() -> serde_json::Value {
        match load_active_code_workspace() {
            Ok(snapshot) => {
                let changed_files = snapshot
                    .files
                    .iter()
                    .filter(|file| file.status != CodeWorkspaceFileStatus::Clean)
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>();
                json!({
                    "connected": true,
                    "changed_files": changed_files,
                    "source": snapshot.source,
                    "truncated": snapshot.truncated,
                })
            }
            Err(error) => json!({
                "connected": false,
                "error": error.to_string(),
                "changed_files": [],
            }),
        }
    }

    #[tauri::command]
    pub fn read_code_file(relative_path: String) -> Result<serde_json::Value, String> {
        let document =
            read_active_code_document(&relative_path).map_err(|error| error.to_string())?;
        Ok(json!({
            "path": document.path,
            "bytes": document.bytes,
            "content": document.content,
            "language": document.language,
            "fingerprint": document.fingerprint,
        }))
    }
}

use tauri::Emitter;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::ShortcutState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(terminal::TerminalManager::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["shift+super+k", "shift+ctrl+k"])
                .unwrap_or_else(|err| {
                    eprintln!("Failed to register global shortcuts: {}", err);
                    tauri_plugin_global_shortcut::Builder::new()
                })
                .with_handler(|app: &tauri::AppHandle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && let Some(window) = app.get_webview_window("main")
                    {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("open-command-palette", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            let quit_i = MenuItem::with_id(app, "quit", "Quit RepoDesk", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_i])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(true)
                .icon(app.default_window_icon().cloned().unwrap())
                .on_menu_event(|app: &tauri::AppHandle, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event
                        && let Some(window) = tray.app_handle().get_webview_window("main")
                    {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            terminal::terminal_create,
            terminal::terminal_list,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_kill,
            code_workspace::code_workspace_snapshot,
            code_workspace::code_workspace_read,
            code_workspace::code_workspace_save,
            code_workbench_commands::read_code_file,
            code_workbench_commands::code_workbench_snapshot,
            git_workspace_commands::git_workspace_snapshot,
            git_workspace_commands::git_file_diff,
            ai_discovery_commands::ai_discovery_scan,
            commands::desktop_snapshot,
            commands::product_workflow_state,
            commands::work_phase_state,
            commands::work_engineering_intelligence,
            commands::work_set_execution_mode,
            commands::work_review,
            commands::work_import_manual_changes,
            commands::work_verify,
            commands::read_artifact,
            commands::agent_context_pack,
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
            commands::task_list,
            commands::task_use,
            commands::task_runner_snapshot,
            commands::task_runner_run,
            commands::task_runner_run_all,
            commands::language_intelligence_snapshot,
            commands::repository_intelligence_snapshot,
            commands::token_usage_snapshot,
            commands::log_token_usage,
            commands::estimate_raw_text,
            commands::token_cost_trend,
            commands::cost_config_get,
            commands::cost_config_save,
            commands::cost_config_reset,
            commands::custom_providers_list,
            commands::custom_providers_presets,
            commands::custom_providers_save,
            commands::custom_providers_delete,
            commands::project_ai_scan,
            commands::project_ai_import,
            commands::model_health_snapshot,
            commands::refresh_model_health,
            commands::start_local_server,
            commands::system_model_recommendations,
            commands::routing_decision,
            commands::routing_snapshot,
            commands::get_active_project_config,
            commands::project_list_configs,
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
            commands::work_execution_preview,
            commands::orchestrate_run,
            commands::orchestrate_loop,
            commands::orchestrate_status,
            commands::orchestrate_show,
            commands::orchestrate_review,
            commands::orchestrate_run_diffs,
            commands::orchestrate_check_proof,
            commands::orchestrate_worktrees,
            commands::orchestrate_cleanup_worktree,
            commands::orchestration_runs,
            commands::credential_set,
            commands::credential_delete,
            commands::credential_status,
            commands::audit_recent,
            commands::audit_verify,
            commands::task_timeline,
            commands::coding_agent_executors,
            commands::outcomes_list,
            commands::outcomes_stats,
            commands::outcomes_confirm,
            commands::playbooks_list,
            commands::playbooks_save,
            commands::playbooks_delete,
            commands::playbooks_import,
            commands::paid_agent_gate,
            commands::commit_ready_changes,
            commands::repopilot_findings,
            commands::repopilot_history,
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

        assert_eq!(snapshot.providers.len(), 7);
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
    fn run_action_executes_whitelisted_action() {
        let action = commands::find_action("runtime-route-patch").expect("action is registered");
        let result = tauri::async_runtime::block_on(commands::action_service::run_action(&action));
        assert!(result.ok, "whitelisted action should run: {result:?}");
        assert!(!result.stdout.is_empty());
    }

    #[test]
    fn run_action_rejects_unknown_action() {
        let action = repodesk_core::workflow::DesktopAction {
            id: "rm-rf-root".into(),
            title: "blocked".into(),
            description: "blocked".into(),
            category: "blocked".into(),
            risk: "blocked".into(),
            command_preview: "repodesk rm -rf /".into(),
            args: vec!["rm".into(), "-rf".into(), "/".into()],
        };
        let result = tauri::async_runtime::block_on(commands::action_service::run_action(&action));
        assert!(!result.ok);
        assert!(result.stderr.contains("is not registered or allowed"));
    }
}
