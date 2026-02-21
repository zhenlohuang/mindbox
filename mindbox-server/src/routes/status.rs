use axum::{Json, Router, extract::State, routing::get};
use chrono::Utc;
use mindbox_common::StatusResponse;
use serde::Serialize;

use crate::{error::ApiResult, services::task_service::TaskService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(status))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn status(State(state): State<AppState>) -> ApiResult<Json<StatusResponse>> {
    let task_service = TaskService::new(state.clone());
    let task_count = task_service.list_tasks().await?.len();

    let payload = StatusResponse {
        kernel: state.kernel.name().to_string(),
        running_task_id: state.task_lock.current().await,
        tasks_count: task_count,
        timestamp: Utc::now(),
    };

    Ok(Json(payload))
}
