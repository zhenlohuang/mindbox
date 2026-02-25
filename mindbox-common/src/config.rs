use std::{env, path::PathBuf};

use crate::{MindboxError, Result};

#[derive(Debug, Clone)]
pub struct MindboxConfig {
    pub kernel: String,
    pub data_root: PathBuf,
    pub port: u16,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
}

impl MindboxConfig {
    pub fn from_env() -> Result<Self> {
        let kernel = env::var("MINDBOX_KERNEL").unwrap_or_else(|_| "claude-code".to_string());
        let data_root = env::var("MINDBOX_DATA_ROOT").unwrap_or_else(|_| "/mindbox".to_string());

        let port = env::var("MINDBOX_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|e| MindboxError::Config(format!("invalid PORT: {e}")))?;

        Ok(Self {
            kernel,
            data_root: PathBuf::from(data_root),
            port,
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
        })
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.data_root.join("tasks")
    }

    pub fn datasets_dir(&self) -> PathBuf {
        self.data_root.join("datasets")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_root.join("models")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_resolved() {
        let cfg = MindboxConfig::from_env().expect("config");
        assert!(!cfg.kernel.is_empty());
        assert!(cfg.port > 0);
    }
}
