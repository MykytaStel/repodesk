use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{EventEntry, legacy_journal_path};
use super::scan::visit_verified_events;

/// Materialize the verified SQLite ledger as the historical JSONL shape.
///
/// Events are verified and written oldest-first one at a time, so export memory
/// is O(one decoded event) rather than O(total ledger). JSONL remains an export
/// artifact only; modifying it never changes canonical state.
pub(super) fn export_event_journal_jsonl() -> RepoDeskResult<PathBuf> {
    let target = legacy_journal_path()?;
    let temp = target.with_file_name(format!(
        ".event-journal.jsonl.tmp-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));

    let result = (|| -> RepoDeskResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;

        visit_verified_events(|event| {
            let entry = EventEntry::from(event);
            writeln!(file, "{}", serde_json::to_string(&entry)?)?;
            Ok(())
        })?;
        file.flush()?;
        file.sync_all()?;

        if target.exists() {
            let metadata = fs::symlink_metadata(&target)?;
            if metadata.file_type().is_symlink() || metadata.is_file() {
                fs::remove_file(&target)?;
            } else {
                return Err(RepoDeskError::Database(format!(
                    "event journal export target is not a file: {}",
                    target.display()
                )));
            }
        }
        fs::rename(&temp, &target)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    Ok(target)
}
