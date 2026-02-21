use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub dataset_path: String,
    pub task_description: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub matched_skill: Option<String>,
    pub framework: Option<String>,
    pub base_model: Option<String>,
    pub hardware: Option<HardwareInfo>,
    pub hyperparameters: Option<serde_json::Value>,
    pub results: Option<TaskResults>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn can_transition_to(self, next: TaskStatus) -> bool {
        use TaskStatus::{Cancelled, Completed, Failed, Pending, Running};
        match (self, next) {
            (Pending, Running | Cancelled) => true,
            (Running, Completed | Failed | Cancelled) => true,
            (Completed | Failed | Cancelled, _) => false,
            (a, b) if a == b => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub gpu_count: Option<u32>,
    pub gpu_names: Option<Vec<String>>,
    pub cpu_cores: Option<u32>,
    pub memory_gb: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResults {
    pub metrics: Vec<Metric>,
    pub model_path: Option<String>,
    pub report_path: Option<String>,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub step: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskEvent {
    StatusUpdate {
        status: TaskStatus,
        message: String,
        timestamp: DateTime<Utc>,
    },
    Log {
        level: LogLevel,
        message: String,
        timestamp: DateTime<Utc>,
    },
    Metric {
        metric: Metric,
        timestamp: DateTime<Utc>,
    },
    Error {
        message: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectsResponse {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProjectResponse {
    pub project: Project,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub dataset_path: String,
    pub task_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTaskResponse {
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTaskResponse {
    pub task_id: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryTaskResponse {
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactItem {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListArtifactsResponse {
    pub artifacts: Vec<ArtifactItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub kernel: String,
    pub running_task_id: Option<String>,
    pub projects_count: usize,
    pub tasks_count: usize,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_transition_rules_hold() {
        assert!(TaskStatus::Pending.can_transition_to(TaskStatus::Running));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Completed));
        assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::Running));
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Pending));
    }

    #[test]
    fn task_event_roundtrip() {
        let event = TaskEvent::Log {
            level: LogLevel::Info,
            message: "hello".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: TaskEvent = serde_json::from_str(&json).expect("deserialize");
        match back {
            TaskEvent::Log { message, .. } => assert_eq!(message, "hello"),
            _ => panic!("unexpected event variant"),
        }
    }
}
