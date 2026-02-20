use tokio::sync::Mutex;

use mindbox_common::{MindboxError, Result};

#[derive(Default)]
pub struct TaskLockService {
    current: Mutex<Option<String>>,
}

impl TaskLockService {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }

    pub async fn try_acquire(&self, task_id: String) -> Result<()> {
        let mut guard = self.current.lock().await;
        if guard.is_some() {
            return Err(MindboxError::TaskLockBusy);
        }
        *guard = Some(task_id);
        Ok(())
    }

    pub async fn release(&self, task_id: &str) {
        let mut guard = self.current.lock().await;
        if guard.as_deref() == Some(task_id) {
            *guard = None;
        }
    }

    pub async fn current(&self) -> Option<String> {
        self.current.lock().await.clone()
    }
}
