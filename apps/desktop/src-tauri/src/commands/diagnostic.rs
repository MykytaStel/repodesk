use super::{
    ApiEnvDiagnostic, CommandResult, action_catalog, build_product_workflow_state, now_ms,
    workspace_root,
};
use serde_json::json;

#[tauri::command]
pub async fn desktop_snapshot() -> serde_json::Value {
    let dashboard = match repodesk_core::dashboard::dashboard_summary().await {
        Ok(stdout) => CommandResult {
            ok: true,
            command: "repodesk dashboard summary".into(),
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk dashboard summary".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let workflow = match repodesk_core::workflow::workflow_next() {
        Ok(stdout) => CommandResult {
            ok: true,
            command: "repodesk workflow next".into(),
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk workflow next".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let doctor = match repodesk_core::workflow_doctor::diagnose_workflow() {
        Ok(report) => CommandResult {
            ok: true,
            command: "repodesk doctor workflow".into(),
            stdout: repodesk_core::workflow_doctor::format_workflow_doctor_report(&report),
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk doctor workflow".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let security = match repodesk_core::security::audit_security_policy() {
        Ok(report) => CommandResult {
            ok: true,
            command: "repodesk security audit".into(),
            stdout: repodesk_core::security::format_security_audit(&report),
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk security audit".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let runtime = {
        let providers = repodesk_core::runtime::runtime_providers();
        CommandResult {
            ok: true,
            command: "repodesk runtime providers".into(),
            stdout: repodesk_core::runtime::format_runtime_providers(&providers),
            stderr: String::new(),
            exit_code: Some(0),
        }
    };

    let git = match repodesk_core::git_audit::git_audit() {
        Ok(stdout) => CommandResult {
            ok: true,
            command: "repodesk git audit".into(),
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk git audit".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let events = match repodesk_core::persistence::event_journal::read_events(10) {
        Ok(evs) => CommandResult {
            ok: true,
            command: "repodesk events last --limit 10".into(),
            stdout: repodesk_core::persistence::event_journal::format_events(&evs),
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CommandResult {
            ok: false,
            command: "repodesk events last --limit 10".into(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: Some(1),
        },
    };

    let project = repodesk_core::projects::get_active_project()
        .ok()
        .map(|config| {
            json!({
                "name": config.name,
                "path": config.path,
                "project_type": config.project_type,
                "main_language": config.main_language,
            })
        });
    let task = repodesk_core::tasks::show_active_task().ok().map(|info| {
        json!({
            "id": info.config.id,
            "project_name": info.config.project_name,
            "title": info.config.title,
            "status": info.config.status,
            "run_dir": info.config.run_dir,
        })
    });

    json!({
        "mode": "desktop-product-workflow-mvp",
        "workspace_root": workspace_root(),
        "generated_at_ms": now_ms(),
        "project": project,
        "task": task,
        "actions": action_catalog(),
        "workflow_state": build_product_workflow_state(),
        "dashboard": dashboard,
        "workflow": workflow,
        "doctor": doctor,
        "security": security,
        "runtime": runtime,
        "git": git,
        "project_info": super::project_info(),
        "project_list": super::project_list(),
        "task_status": super::task_status(),
        "task_show": super::task_show(),
        "events": events,
    })
}

#[tauri::command]
pub fn get_api_env_diagnostic() -> ApiEnvDiagnostic {
    let env_set = |name: &str| {
        std::env::var(name)
            .map(|val| !val.trim().is_empty())
            .unwrap_or(false)
    };
    // A key counts as configured if it's stored in-app OR present in the env.
    let saved = crate::store::read_provider_settings().unwrap_or_default();
    let stored = |key: &str| !key.trim().is_empty();
    ApiEnvDiagnostic {
        openai_api_key_set: stored(&saved.openai_api_key) || env_set("OPENAI_API_KEY"),
        gemini_api_key_set: stored(&saved.gemini_api_key) || env_set("GEMINI_API_KEY"),
        anthropic_api_key_set: stored(&saved.anthropic_api_key) || env_set("ANTHROPIC_API_KEY"),
    }
}
