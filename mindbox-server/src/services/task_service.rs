use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::Utc;
use mindbox_common::{
    ArtifactItem, CreateTaskRequest, LogLevel, Metric, MindboxError, Project, Result, Task,
    TaskEvent, TaskResults, TaskStatus,
};
use mindbox_kernel::{DatasetMetadata, TaskContext, callback::KernelCallback};
use tracing::{error, warn};

use crate::state::{AppState, BroadcastEvent};

use super::project_service::ProjectService;

#[derive(Clone)]
pub struct TaskService {
    state: AppState,
    projects: ProjectService,
}

impl TaskService {
    pub fn new(state: AppState) -> Self {
        let projects = ProjectService::new(state.config.clone());
        Self { state, projects }
    }

    pub async fn create_and_start_task(
        &self,
        project_id: &str,
        req: CreateTaskRequest,
    ) -> Result<Task> {
        let project = self.projects.get_project(project_id).await?;
        let task_id = self.generate_task_id(project_id).await?;

        self.state.task_lock.try_acquire(task_id.clone()).await?;

        let task_dir = self.task_dir(project_id, &task_id);
        tokio::fs::create_dir_all(task_dir.join("artifacts")).await?;
        tokio::fs::create_dir_all(task_dir.join("workspace")).await?;
        tokio::fs::create_dir_all(task_dir.join("logs")).await?;

        let now = Utc::now();
        let task = Task {
            id: task_id.clone(),
            project_id: project_id.to_string(),
            dataset_path: req.dataset_path,
            task_description: req.task_description,
            status: TaskStatus::Running,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
            matched_skill: None,
            framework: None,
            base_model: None,
            hardware: None,
            hyperparameters: None,
            results: None,
            error_message: None,
        };
        self.save_task(project_id, &task).await?;

        let state = self.state.clone();
        let task_for_run = task.clone();
        let task_dir_for_run = task_dir.clone();
        tokio::spawn(async move {
            if let Err(e) = run_kernel(state, project, task_for_run, task_dir_for_run).await {
                error!("failed to run kernel task: {e}");
            }
        });

        Ok(task)
    }

    pub async fn list_tasks(&self, project_id: &str) -> Result<Vec<Task>> {
        self.projects.get_project(project_id).await?;

        let mut tasks = Vec::new();
        let tasks_dir = self.projects.tasks_dir(project_id);
        if !tasks_dir.exists() {
            return Ok(tasks);
        }

        let mut entries = tokio::fs::read_dir(tasks_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let task_file = entry.path().join("task.yaml");
            if !task_file.exists() {
                continue;
            }
            let content = tokio::fs::read_to_string(task_file).await?;
            let task: Task = serde_yaml::from_str(&content)?;
            tasks.push(task);
        }

        tasks.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(tasks)
    }

    pub async fn get_task(&self, project_id: &str, task_id: &str) -> Result<Task> {
        self.projects.get_project(project_id).await?;

        let task_file = self.task_dir(project_id, task_id).join("task.yaml");
        if !task_file.exists() {
            return Err(MindboxError::TaskNotFound(task_id.to_string()));
        }
        let content = tokio::fs::read_to_string(task_file).await?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub async fn cancel_task(&self, project_id: &str, task_id: &str) -> Result<Task> {
        let mut task = self.get_task(project_id, task_id).await?;
        if !matches!(task.status, TaskStatus::Running | TaskStatus::Pending) {
            return Err(MindboxError::InvalidStateTransition {
                from: format!("{:?}", task.status),
                to: format!("{:?}", TaskStatus::Cancelled),
            });
        }

        self.state.kernel.cancel(task_id);
        transition_status(&mut task, TaskStatus::Cancelled)?;
        task.completed_at = Some(Utc::now());
        task.error_message = Some("cancel requested".to_string());
        self.save_task(project_id, &task).await?;

        self.state.task_lock.release(task_id).await;

        Ok(task)
    }

    pub async fn retry_task(&self, project_id: &str, task_id: &str) -> Result<Task> {
        let task = self.get_task(project_id, task_id).await?;
        if !matches!(task.status, TaskStatus::Failed | TaskStatus::Cancelled) {
            return Err(MindboxError::InvalidStateTransition {
                from: format!("{:?}", task.status),
                to: format!("{:?}", TaskStatus::Pending),
            });
        }

        self.create_and_start_task(
            project_id,
            CreateTaskRequest {
                dataset_path: task.dataset_path,
                task_description: task.task_description,
            },
        )
        .await
    }

    pub async fn read_logs(&self, project_id: &str, task_id: &str) -> Result<String> {
        self.get_task(project_id, task_id).await?;

        let logs_path = self.task_dir(project_id, task_id).join("logs/kernel.log");
        if !logs_path.exists() {
            return Ok(String::new());
        }

        Ok(tokio::fs::read_to_string(logs_path).await?)
    }

    pub async fn list_artifacts(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Vec<ArtifactItem>> {
        self.get_task(project_id, task_id).await?;

        let artifacts_dir = self.task_dir(project_id, task_id).join("artifacts");
        if !artifacts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut out = collect_artifacts(&artifacts_dir).await?;
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    pub async fn artifact_path(
        &self,
        project_id: &str,
        task_id: &str,
        path: &str,
    ) -> Result<PathBuf> {
        self.get_task(project_id, task_id).await?;

        let base = self.task_dir(project_id, task_id).join("artifacts");
        let relative = PathBuf::from(path);
        if relative
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(MindboxError::Config("invalid artifact path".to_string()));
        }

        let full = base.join(relative);
        if !full.exists() {
            return Err(MindboxError::TaskNotFound(format!(
                "artifact not found: {path}"
            )));
        }

        Ok(full)
    }

    pub fn task_dir(&self, project_id: &str, task_id: &str) -> PathBuf {
        self.projects.tasks_dir(project_id).join(task_id)
    }

    async fn save_task(&self, project_id: &str, task: &Task) -> Result<()> {
        let task_file = self.task_dir(project_id, &task.id).join("task.yaml");
        if let Some(parent) = task_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_yaml::to_string(task)?;
        tokio::fs::write(task_file, data).await?;
        Ok(())
    }

    async fn generate_task_id(&self, project_id: &str) -> Result<String> {
        let today = Utc::now().format("%Y%m%d").to_string();
        let prefix = format!("task-{today}-");
        let tasks_dir = self.projects.tasks_dir(project_id);
        tokio::fs::create_dir_all(&tasks_dir).await?;

        let mut max_seq: u32 = 0;
        let mut entries = tokio::fs::read_dir(tasks_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            if !name.starts_with(&prefix) {
                continue;
            }
            let Some(raw_seq) = name.strip_prefix(&prefix) else {
                continue;
            };
            if let Ok(seq) = raw_seq.parse::<u32>() {
                max_seq = max_seq.max(seq);
            }
        }

        Ok(format!("{}{:03}", prefix, max_seq + 1))
    }
}

struct BroadcastCallback {
    project_id: String,
    task_id: String,
    tx: tokio::sync::broadcast::Sender<BroadcastEvent>,
}

impl BroadcastCallback {
    fn new(
        project_id: String,
        task_id: String,
        tx: tokio::sync::broadcast::Sender<BroadcastEvent>,
    ) -> Self {
        Self {
            project_id,
            task_id,
            tx,
        }
    }

    async fn emit_event(&self, event: TaskEvent) {
        let _ = self.tx.send(BroadcastEvent {
            project_id: self.project_id.clone(),
            task_id: self.task_id.clone(),
            event,
        });
    }
}

#[async_trait]
impl KernelCallback for BroadcastCallback {
    async fn status_update(&self, status: TaskStatus, message: String) {
        self.emit_event(TaskEvent::StatusUpdate {
            status,
            message,
            timestamp: Utc::now(),
        })
        .await;
    }

    async fn log(&self, level: LogLevel, message: String) {
        self.emit_event(TaskEvent::Log {
            level,
            message,
            timestamp: Utc::now(),
        })
        .await;
    }

    async fn metric(&self, metric: Metric) {
        self.emit_event(TaskEvent::Metric {
            metric,
            timestamp: Utc::now(),
        })
        .await;
    }

    async fn error(&self, message: String) {
        self.emit_event(TaskEvent::Error {
            message,
            timestamp: Utc::now(),
        })
        .await;
    }
}

async fn run_kernel(
    state: AppState,
    project: Project,
    task: Task,
    task_dir: PathBuf,
) -> Result<()> {
    let callback = Arc::new(BroadcastCallback::new(
        project.id.clone(),
        task.id.clone(),
        state.event_tx.clone(),
    ));

    callback
        .status_update(TaskStatus::Running, "task started".to_string())
        .await;

    let dataset = collect_dataset_metadata(Path::new(&task.dataset_path)).await;
    let context = TaskContext {
        project: project.clone(),
        task: task.clone(),
        dataset,
        task_dir: task_dir.clone(),
        skills_dir: state.config.skills_dir(),
    };

    let result = state.kernel.execute(context, callback.clone()).await;

    let task_file = task_dir.join("task.yaml");
    let mut latest = load_task(&task_file).await.unwrap_or(task);

    match result {
        Ok(()) => {
            if latest.status != TaskStatus::Cancelled {
                transition_status(&mut latest, TaskStatus::Completed)?;
                latest.completed_at = Some(Utc::now());
                latest.results = Some(TaskResults {
                    metrics: Vec::new(),
                    model_path: None,
                    report_path: None,
                    artifacts: Vec::new(),
                });
                save_task_to_file(&task_file, &latest).await?;
                callback
                    .status_update(TaskStatus::Completed, "task completed".to_string())
                    .await;
            }
        }
        Err(err) => {
            warn!("task {} kernel failure: {err}", latest.id);
            if latest.status != TaskStatus::Cancelled {
                transition_status(&mut latest, TaskStatus::Failed)?;
                latest.completed_at = Some(Utc::now());
                latest.error_message = Some(err.to_string());
                save_task_to_file(&task_file, &latest).await?;
                callback.error(err.to_string()).await;
                callback
                    .status_update(TaskStatus::Failed, "task failed".to_string())
                    .await;
            }
        }
    }

    state.task_lock.release(&latest.id).await;
    Ok(())
}

async fn collect_dataset_metadata(dataset_path: &Path) -> DatasetMetadata {
    let metadata = tokio::fs::metadata(dataset_path).await.ok();
    let size_bytes = metadata.as_ref().map(|m| m.len());
    let exists = metadata.is_some();

    let record_count = if exists {
        tokio::fs::read_to_string(dataset_path)
            .await
            .ok()
            .map(|c| c.lines().count() as u64)
    } else {
        None
    };

    DatasetMetadata {
        dataset_path: dataset_path.to_path_buf(),
        exists,
        size_bytes,
        record_count,
    }
}

async fn collect_artifacts(root: &Path) -> Result<Vec<ArtifactItem>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let meta = entry.metadata().await?;
            if meta.is_dir() {
                stack.push(path);
                continue;
            }

            let relative = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());

            out.push(ArtifactItem {
                path: relative,
                size_bytes: meta.len(),
            });
        }
    }

    Ok(out)
}

async fn load_task(task_file: &Path) -> Result<Task> {
    let content = tokio::fs::read_to_string(task_file).await?;
    Ok(serde_yaml::from_str(&content)?)
}

async fn save_task_to_file(task_file: &Path, task: &Task) -> Result<()> {
    let data = serde_yaml::to_string(task)?;
    tokio::fs::write(task_file, data).await?;
    Ok(())
}

fn transition_status(task: &mut Task, next: TaskStatus) -> Result<()> {
    if !task.status.can_transition_to(next) {
        return Err(MindboxError::InvalidStateTransition {
            from: format!("{:?}", task.status),
            to: format!("{:?}", next),
        });
    }
    task.status = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn task_id_format_uses_sequence() {
        let root =
            std::env::temp_dir().join(format!("mindbox-task-id-test-{}", uuid::Uuid::new_v4()));
        let config = Arc::new(mindbox_common::MindboxConfig {
            kernel: "claude-code".to_string(),
            data_root: root,
            port: 8080,
            anthropic_api_key: None,
            openai_api_key: None,
        });
        let state = AppState::new(
            config.clone(),
            Arc::new(mindbox_kernel::codex::CodexKernel::new()),
            Arc::new(crate::services::task_lock::TaskLockService::new()),
            tokio::sync::broadcast::channel(16).0,
        );
        let service = TaskService::new(state);
        tokio::fs::create_dir_all(service.projects.tasks_dir("default"))
            .await
            .expect("mkdir");

        let id = service.generate_task_id("default").await.expect("id");
        assert!(id.starts_with(&format!("task-{}-", Utc::now().format("%Y%m%d"))));
    }
}
