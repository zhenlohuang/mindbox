use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};
use mindbox_common::{ListArtifactsResponse, MindboxError};

use crate::{error::ApiResult, services::task_service::TaskService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tasks/{task_id}/artifacts", get(list_artifacts))
        .route(
            "/api/v1/tasks/{task_id}/artifacts/{*path}",
            get(download_artifact),
        )
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> ApiResult<Json<ListArtifactsResponse>> {
    let service = TaskService::new(state);
    let artifacts = service.list_artifacts(&task_id).await?;
    Ok(Json(ListArtifactsResponse { artifacts }))
}

async fn download_artifact(
    State(state): State<AppState>,
    Path((task_id, path)): Path<(String, String)>,
) -> ApiResult<Response> {
    let service = TaskService::new(state);
    let file_path = service.artifact_path(&task_id, &path).await?;
    let data = tokio::fs::read(file_path)
        .await
        .map_err(MindboxError::from)?;

    let mut resp = Response::new(Body::from(data));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok(resp)
}
