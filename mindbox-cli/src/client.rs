use anyhow::{Context, Result};
use mindbox_common::{
    CancelTaskResponse, CreateProjectRequest, CreateTaskRequest, GetTaskResponse,
    ListProjectsResponse, ListTasksResponse,
};

#[derive(Clone)]
pub struct MindboxClient {
    base_url: String,
    http: reqwest::Client,
}

impl MindboxClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn logs_follow_url(&self, project_id: &str, task_id: &str) -> String {
        format!(
            "{}/api/v1/projects/{}/tasks/{}/logs?follow=true",
            self.base_url, project_id, task_id
        )
    }

    pub async fn create_project(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<mindbox_common::Project> {
        let req = CreateProjectRequest { name, description };
        self.http
            .post(format!("{}/api/v1/projects", self.base_url))
            .json(&req)
            .send()
            .await?
            .error_for_status()?
            .json::<mindbox_common::Project>()
            .await
            .context("parse create_project response")
    }

    pub async fn list_projects(&self) -> Result<ListProjectsResponse> {
        self.http
            .get(format!("{}/api/v1/projects", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ListProjectsResponse>()
            .await
            .context("parse list_projects response")
    }

    pub async fn create_task(
        &self,
        project_id: &str,
        dataset_path: String,
        task_description: String,
    ) -> Result<GetTaskResponse> {
        let req = CreateTaskRequest {
            dataset_path,
            task_description,
        };

        self.http
            .post(format!(
                "{}/api/v1/projects/{}/tasks",
                self.base_url, project_id
            ))
            .json(&req)
            .send()
            .await?
            .error_for_status()?
            .json::<GetTaskResponse>()
            .await
            .context("parse create_task response")
    }

    pub async fn cancel_task(&self, project_id: &str, task_id: &str) -> Result<CancelTaskResponse> {
        self.http
            .post(format!(
                "{}/api/v1/projects/{}/tasks/{}/cancel",
                self.base_url, project_id, task_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json::<CancelTaskResponse>()
            .await
            .context("parse cancel_task response")
    }

    pub async fn list_tasks(&self, project_id: &str) -> Result<ListTasksResponse> {
        self.http
            .get(format!(
                "{}/api/v1/projects/{}/tasks",
                self.base_url, project_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json::<ListTasksResponse>()
            .await
            .context("parse list_tasks response")
    }
}
