use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{RepoDeskError, RepoDeskResult};

use super::RecoveryEngine;

const RECOVERY_STATE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PersistedRecoveryState {
    version: u32,
    engine: RecoveryEngine,
}

pub struct RecoveryStore;

impl RecoveryStore {
    pub fn load(
        path: &Path,
        project: String,
        history_limit: usize,
    ) -> RepoDeskResult<RecoveryEngine> {
        if !path.exists() {
            return Ok(RecoveryEngine::new(project, history_limit));
        }
        let contents = fs::read_to_string(path)?;
        let persisted: PersistedRecoveryState = serde_json::from_str(&contents)?;
        if persisted.version != RECOVERY_STATE_VERSION {
            return Err(RepoDeskError::Api(format!(
                "Unsupported recovery state version {}",
                persisted.version
            )));
        }
        if persisted.engine.project != project {
            return Ok(RecoveryEngine::new(project, history_limit));
        }
        let mut engine = persisted.engine;
        engine.history_limit = history_limit;
        engine.enforce_loaded_history_limit();
        Ok(engine)
    }

    pub fn save(path: &Path, engine: &RecoveryEngine) -> RepoDeskResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let staging = path.with_extension("json.staging");
        let persisted = PersistedRecoveryState {
            version: RECOVERY_STATE_VERSION,
            engine: engine.clone(),
        };
        let mut file = File::create(&staging)?;
        serde_json::to_writer_pretty(&mut file, &persisted)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&staging, path)?;
        Ok(())
    }
}
