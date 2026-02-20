use thiserror::Error;

#[derive(Debug, Error)]
pub enum MindboxError {
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("task not found: {0}")]
    TaskNotFound(String),

    #[error("invalid task status transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("task lock is busy")]
    TaskLockBusy,

    #[error("kernel error: {0}")]
    KernelError(String),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("task cancelled: {0}")]
    Cancelled(String),
}

pub type Result<T> = std::result::Result<T, MindboxError>;
