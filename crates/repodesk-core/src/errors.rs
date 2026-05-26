use thiserror::Error;

pub type RepoDeskResult<T> = Result<T, RepoDeskError>;

#[derive(Debug, Error)]
pub enum RepoDeskError {
    #[error("could not determine a valid RepoDesk home directory")]
    HomeDirectoryNotFound,

    #[error("project '{0}' already exists")]
    ProjectAlreadyExists(String),

    #[error("project '{0}' was not found")]
    ProjectNotFound(String),

    #[error("active project is not set")]
    ActiveProjectNotSet,

    #[error("active task is not set for project '{0}'")]
    ActiveTaskNotSet(String),

    #[error("task '{0}' was not found")]
    TaskNotFound(String),

    #[error("invalid project name '{0}'. Use letters, numbers, '-', '_' only")]
    InvalidProjectName(String),

    #[error("project path does not exist: {0}")]
    ProjectPathDoesNotExist(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}
