use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use mindbox_common::{CreateProjectRequest, GetProjectResponse, ListProjectsResponse};

use crate::{
    error::ApiResult,
    services::{project_service::ProjectService, task_service::TaskService},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", post(create_project).get(list_projects))
        .route("/api/v1/projects/{project_id}", get(get_project))
}

async fn create_project(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<Json<mindbox_common::Project>> {
    let service = ProjectService::new(state.config.clone());
    let project = service.create_project(&req.name, req.description).await?;
    Ok(Json(project))
}

async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<ListProjectsResponse>> {
    let service = ProjectService::new(state.config.clone());
    let projects = service.list_projects().await?;
    Ok(Json(ListProjectsResponse { projects }))
}

async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<GetProjectResponse>> {
    let project_service = ProjectService::new(state.config.clone());
    let task_service = TaskService::new(state);

    let project = project_service.get_project(&project_id).await?;
    let tasks = task_service.list_tasks(&project_id).await?;
    Ok(Json(GetProjectResponse { project, tasks }))
}
