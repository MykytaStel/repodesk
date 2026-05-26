use std::fs;

use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;

#[derive(Debug, Clone)]
pub struct InitResult {
    pub home: String,
    pub created_dirs: Vec<String>,
}

pub fn init_home() -> RepoDeskResult<InitResult> {
    let paths = RepoDeskPaths::resolve()?;

    let dirs = [
        paths.home.clone(),
        paths.config_dir.clone(),
        paths.projects_dir.clone(),
        paths.runs_dir.clone(),
        paths.logs_dir.clone(),
        paths.cache_dir.clone(),
    ];

    let mut created_dirs = Vec::new();

    for dir in dirs {
        let existed = dir.exists();
        fs::create_dir_all(&dir)?;

        if !existed {
            created_dirs.push(dir.display().to_string());
        }
    }

    Ok(InitResult {
        home: paths.home.display().to_string(),
        created_dirs,
    })
}
