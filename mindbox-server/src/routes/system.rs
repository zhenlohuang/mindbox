use axum::{Json, Router, extract::State, routing::get};
use mindbox_common::SystemResources;

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/system/resources", get(get_system_resources))
}

async fn get_system_resources(State(state): State<AppState>) -> ApiResult<Json<SystemResources>> {
    Ok(Json(state.system_monitor.snapshot().await))
}
