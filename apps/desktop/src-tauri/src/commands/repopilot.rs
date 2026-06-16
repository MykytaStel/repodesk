use repodesk_core::repopilot::{
    RepoPilotHistory, RepoPilotReport, load_history, parse_review_json, record_report,
};

/// Run `repopilot review . --format json` in the active project and parse the
/// result into a structured findings report. RepoPilot is read-only; it is run
/// with argument arrays (no shell). A non-zero exit (e.g. `--fail-on`) still
/// produces a report, so we read the output file regardless of exit status.
///
/// A successful review is also appended to the active task's health trend
/// (best-effort: a missing active task or write failure never fails the review).
#[tauri::command]
pub fn repopilot_findings() -> RepoPilotReport {
    let project = match repodesk_core::projects::get_active_project() {
        Ok(project) => project,
        Err(error) => return RepoPilotReport::error(format!("No active project: {error}")),
    };

    let output_path =
        std::env::temp_dir().join(format!("repodesk-repopilot-{}.json", super::now_ms()));

    let output = std::process::Command::new("repopilot")
        .arg("review")
        .arg(".")
        .args(["--format", "json", "--output"])
        .arg(&output_path)
        .current_dir(&project.path)
        .output();

    match output {
        Ok(_) => {
            let report = std::fs::read_to_string(&output_path)
                .map(|raw| parse_review_json(&raw))
                .unwrap_or_else(|error| {
                    RepoPilotReport::error(format!("Could not read RepoPilot output: {error}"))
                });
            let _ = std::fs::remove_file(&output_path);
            // Advisory trend; never let a persistence hiccup sink the review.
            let _ = record_report(&report);
            report
        }
        Err(error) => {
            RepoPilotReport::error(format!("RepoPilot is not available on PATH: {error}"))
        }
    }
}

/// Read the active task's persisted RepoPilot health trend (oldest first).
/// Returns an empty history when there is no active task or nothing recorded.
#[tauri::command]
pub fn repopilot_history() -> RepoPilotHistory {
    load_history().unwrap_or_default()
}
