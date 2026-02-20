pub mod artifacts;
pub mod logs;
pub mod projects;
pub mod status;
pub mod tasks;

use axum::Router;

use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(projects::router())
        .merge(tasks::router())
        .merge(logs::router())
        .merge(artifacts::router())
        .merge(status::router())
        .with_state(state)
}
