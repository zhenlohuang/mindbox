pub mod artifacts;
pub mod logs;
pub mod status;
pub mod system;
pub mod tasks;

use axum::Router;

use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(tasks::router())
        .merge(logs::router())
        .merge(artifacts::router())
        .merge(status::router())
        .merge(system::router())
        .with_state(state)
}
