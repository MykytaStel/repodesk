use repodesk_core::repopilot::{RepoPilotReport, parse_review_json};

/// Run `repopilot review . --format json` in the active project and parse the
/// result into a structured findings report. RepoPilot is read-only; it is run
/// with argument arrays (no shell). A non-zero exit (e.g. `--fail-on`) still
/// produces a report, so we read the output file regardless of exit status.
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
            report
        }
        Err(error) => {
            RepoPilotReport::error(format!("RepoPilot is not available on PATH: {error}"))
        }
    }
}
