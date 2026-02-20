use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use mindbox_common::{
    CancelTaskResponse, CreateTaskRequest, GetTaskResponse, ListTasksResponse, RetryTaskResponse,
    TaskEventsResponse,
};

use crate::{error::ApiResult, services::task_service::TaskService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/tasks",
            post(create_task).get(list_tasks),
        )
        .route(
            "/api/v1/projects/{project_id}/tasks/{task_id}",
            get(get_task),
        )
        .route(
            "/api/v1/projects/{project_id}/tasks/{task_id}/cancel",
            post(cancel_task),
        )
        .route(
            "/api/v1/projects/{project_id}/tasks/{task_id}/retry",
            post(retry_task),
        )
        .route(
            "/api/v1/projects/{project_id}/tasks/{task_id}/events",
            get(get_task_events),
        )
}

async fn create_task(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(req): Json<CreateTaskRequest>,
) -> ApiResult<Json<GetTaskResponse>> {
    let service = TaskService::new(state);
    let task = service.create_and_start_task(&project_id, req).await?;
    Ok(Json(GetTaskResponse { task }))
}

async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<ListTasksResponse>> {
    let service = TaskService::new(state);
    let tasks = service.list_tasks(&project_id).await?;
    Ok(Json(ListTasksResponse { tasks }))
}

async fn get_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> ApiResult<Json<GetTaskResponse>> {
    let service = TaskService::new(state);
    let task = service.get_task(&project_id, &task_id).await?;
    Ok(Json(GetTaskResponse { task }))
}

async fn cancel_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> ApiResult<Json<CancelTaskResponse>> {
    let service = TaskService::new(state);
    let task = service.cancel_task(&project_id, &task_id).await?;
    Ok(Json(CancelTaskResponse {
        task_id: task.id,
        status: task.status,
    }))
}

async fn retry_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> ApiResult<Json<RetryTaskResponse>> {
    let service = TaskService::new(state);
    let task = service.retry_task(&project_id, &task_id).await?;
    Ok(Json(RetryTaskResponse { task }))
}

async fn get_task_events(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> ApiResult<Json<TaskEventsResponse>> {
    let service = TaskService::new(state);
    let events = service.list_events(&project_id, &task_id).await?;
    Ok(Json(TaskEventsResponse { events }))
}
