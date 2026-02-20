use std::sync::Arc;

use mindbox_common::{MindboxConfig, TaskEvent};
use mindbox_kernel::Kernel;
use tokio::sync::broadcast;

use crate::services::task_lock::TaskLockService;

#[derive(Debug, Clone)]
pub struct BroadcastEvent {
    pub project_id: String,
    pub task_id: String,
    pub event: TaskEvent,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<MindboxConfig>,
    pub kernel: Arc<dyn Kernel>,
    pub task_lock: Arc<TaskLockService>,
    pub event_tx: broadcast::Sender<BroadcastEvent>,
}

impl AppState {
    pub fn new(
        config: Arc<MindboxConfig>,
        kernel: Arc<dyn Kernel>,
        task_lock: Arc<TaskLockService>,
        event_tx: broadcast::Sender<BroadcastEvent>,
    ) -> Self {
        Self {
            config,
            kernel,
            task_lock,
            event_tx,
        }
    }
}
