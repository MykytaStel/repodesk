use repodesk_core::task_runner::{
    TaskRunBatch, TaskRunResult, TaskRunnerSnapshot, active_task_runner_snapshot, run_active_task,
    run_all_active_tasks,
};

#[tauri::command]
pub fn task_runner_snapshot() -> Result<TaskRunnerSnapshot, String> {
    active_task_runner_snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn task_runner_run(task_id: String) -> Result<TaskRunResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_active_task(&task_id))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn task_runner_run_all() -> Result<TaskRunBatch, String> {
    tauri::async_runtime::spawn_blocking(run_all_active_tasks)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}
