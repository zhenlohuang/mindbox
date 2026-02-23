use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow};
use async_trait::async_trait;

use crate::{
    Kernel, TaskContext,
    agent_instructions::{build_agent_md, ensure_agent_files},
    callback::KernelCallback,
};

pub struct CodexKernel;

impl CodexKernel {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Kernel for CodexKernel {
    async fn execute(&self, ctx: TaskContext, _callback: Arc<dyn KernelCallback>) -> Result<()> {
        let agent_md = build_agent_md();
        ensure_agent_files(&ctx.task_dir, &agent_md).await?;
        Err(anyhow!("codex kernel is not implemented yet"))
    }

    fn cancel(&self, _task_id: &str) {}

    fn adjust(&self, _task_id: &str, _params: HashMap<String, serde_json::Value>) {}

    fn name(&self) -> &str {
        "codex"
    }
}
