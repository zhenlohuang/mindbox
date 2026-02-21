use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use mindbox_common::{LogLevel, Metric, TaskStatus};

use crate::{Kernel, TaskContext, callback::KernelCallback};

pub struct MockKernel {
    cancelled: Arc<Mutex<HashSet<String>>>,
}

impl MockKernel {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.cancelled
            .lock()
            .map(|state| state.contains(task_id))
            .unwrap_or(false)
    }
}

impl Default for MockKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Kernel for MockKernel {
    async fn execute(&self, ctx: TaskContext, callback: Arc<dyn KernelCallback>) -> Result<()> {
        let task_id = ctx.task.id.clone();

        if let Ok(mut state) = self.cancelled.lock() {
            state.remove(&task_id);
        }

        callback
            .log(
                LogLevel::Info,
                format!(
                    "mock: preparing dataset {}",
                    ctx.dataset.dataset_path.display()
                ),
            )
            .await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        if self.is_cancelled(&task_id) {
            callback
                .status_update(TaskStatus::Cancelled, "task cancelled".to_string())
                .await;
            return Ok(());
        }

        callback
            .log(LogLevel::Info, "mock: running training step".to_string())
            .await;
        callback
            .metric(Metric {
                name: "train/loss".to_string(),
                value: 0.123,
                step: Some(1),
                timestamp: Utc::now(),
            })
            .await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        if self.is_cancelled(&task_id) {
            callback
                .status_update(TaskStatus::Cancelled, "task cancelled".to_string())
                .await;
            return Ok(());
        }

        callback
            .log(LogLevel::Info, "mock: task finished".to_string())
            .await;
        callback
            .status_update(
                TaskStatus::Running,
                "mock: finalizing artifacts".to_string(),
            )
            .await;

        if let Ok(mut state) = self.cancelled.lock() {
            state.remove(&task_id);
        }

        Ok(())
    }

    fn cancel(&self, task_id: &str) {
        if let Ok(mut state) = self.cancelled.lock() {
            state.insert(task_id.to_string());
        }
    }

    fn adjust(&self, _task_id: &str, _params: HashMap<String, serde_json::Value>) {}

    fn name(&self) -> &str {
        "mock"
    }
}
