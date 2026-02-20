use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use mindbox_common::{MindboxConfig, MindboxError, Project, Result};

#[derive(Clone)]
pub struct ProjectService {
    config: Arc<MindboxConfig>,
}

impl ProjectService {
    pub fn new(config: Arc<MindboxConfig>) -> Self {
        Self { config }
    }

    pub async fn ensure_storage_dirs(&self) -> Result<()> {
        tokio::fs::create_dir_all(self.config.projects_dir()).await?;
        tokio::fs::create_dir_all(self.config.datasets_dir()).await?;
        tokio::fs::create_dir_all(self.config.skills_dir()).await?;
        tokio::fs::create_dir_all(self.config.models_dir()).await?;
        Ok(())
    }

    pub async fn ensure_default_project(&self) -> Result<Project> {
        if let Ok(project) = self.get_project("default").await {
            return Ok(project);
        }
        self.create_project("default", Some("Default project".to_string()))
            .await
    }

    pub async fn create_project(&self, name: &str, description: Option<String>) -> Result<Project> {
        let now = Utc::now();
        let project = Project {
            id: name.to_string(),
            name: name.to_string(),
            description: description.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };

        let dir = self.project_dir(name);
        tokio::fs::create_dir_all(dir.join("tasks")).await?;
        self.save_project(&project).await?;
        Ok(project)
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let mut projects = Vec::new();
        let root = self.config.projects_dir();
        if !root.exists() {
            return Ok(projects);
        }

        let mut entries = tokio::fs::read_dir(root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path().join("project.yaml");
            if !path.exists() {
                continue;
            }
            let content = tokio::fs::read_to_string(path).await?;
            let project: Project = serde_yaml::from_str(&content)?;
            projects.push(project);
        }

        projects.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(projects)
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project> {
        let path = self.project_dir(project_id).join("project.yaml");
        if !path.exists() {
            return Err(MindboxError::ProjectNotFound(project_id.to_string()));
        }
        let content = tokio::fs::read_to_string(path).await?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn project_dir(&self, project_id: &str) -> PathBuf {
        self.config.projects_dir().join(project_id)
    }

    pub fn tasks_dir(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("tasks")
    }

    pub async fn save_project(&self, project: &Project) -> Result<()> {
        let path = self.project_dir(&project.id).join("project.yaml");
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_yaml::to_string(project)?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }
}
