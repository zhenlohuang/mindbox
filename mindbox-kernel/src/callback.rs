use async_trait::async_trait;
use mindbox_common::{LogLevel, Metric, TaskStatus};

#[async_trait]
pub trait KernelCallback: Send + Sync {
    async fn status_update(&self, status: TaskStatus, message: String);
    async fn log(&self, level: LogLevel, message: String);
    async fn metric(&self, metric: Metric);
    async fn error(&self, message: String);
}
