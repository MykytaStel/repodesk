use std::fs::{self, OpenOptions};
use std::io::Write;

use chrono::Utc;

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

#[derive(Debug, Clone)]
pub struct AddReceiptInput {
    pub agent: String,
    pub outcome: String,
    pub summary: String,
}

pub fn add_receipt(input: AddReceiptInput) -> RepoDeskResult<String> {
    let task = show_active_task()?;
    let file = task.config.run_dir.join("agent-receipts.md");

    let mut handle = OpenOptions::new().create(true).append(true).open(&file)?;

    writeln!(
        handle,
        "\n## {} — {}\n\nAgent: `{}`\nOutcome: `{}`\n\n{}\n",
        Utc::now().to_rfc3339(),
        task.config.title,
        input.agent,
        input.outcome,
        input.summary.trim()
    )?;

    Ok(file.display().to_string())
}

pub fn read_receipts() -> RepoDeskResult<String> {
    let task = show_active_task()?;
    let file = task.config.run_dir.join("agent-receipts.md");

    if !file.exists() {
        return Ok("No agent receipts recorded for the active task.\n".to_string());
    }

    Ok(fs::read_to_string(file)?)
}
