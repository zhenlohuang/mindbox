pub mod agent_instructions;
pub mod callback;
pub mod claude_code;
pub mod codex;
pub mod mock;
pub mod prompt;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use mindbox_common::{MindboxConfig, Task};

use crate::{claude_code::ClaudeCodeKernel, codex::CodexKernel, mock::MockKernel};

#[derive(Debug, Clone)]
pub struct DatasetMetadata {
    pub dataset_path: PathBuf,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub record_count: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub task: Task,
    pub dataset: DatasetMetadata,
    pub task_dir: PathBuf,
    pub skills_dir: PathBuf,
}

#[async_trait]
pub trait Kernel: Send + Sync {
    async fn execute(
        &self,
        ctx: TaskContext,
        callback: Arc<dyn callback::KernelCallback>,
    ) -> Result<()>;

    fn cancel(&self, task_id: &str);

    fn adjust(&self, _task_id: &str, _params: HashMap<String, serde_json::Value>) {}

    fn name(&self) -> &str;
}

pub fn create_kernel(config: &MindboxConfig) -> Arc<dyn Kernel> {
    match config.kernel.to_lowercase().as_str() {
        "codex" => Arc::new(CodexKernel::new()),
        "mock" => Arc::new(MockKernel::new()),
        _ => Arc::new(ClaudeCodeKernel::new()),
    }
}
