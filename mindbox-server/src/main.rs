mod error;
mod routes;
mod services;
mod state;

use std::sync::Arc;

use mindbox_common::MindboxConfig;
use mindbox_kernel::create_kernel;
use services::task_lock::TaskLockService;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{routes::create_router, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(MindboxConfig::from_env()?);
    let kernel = create_kernel(&config);
    let task_lock = Arc::new(TaskLockService::new());
    let (event_tx, _) = broadcast::channel(2048);

    tokio::fs::create_dir_all(config.tasks_dir()).await?;
    tokio::fs::create_dir_all(config.datasets_dir()).await?;
    tokio::fs::create_dir_all(config.skills_dir()).await?;
    tokio::fs::create_dir_all(config.models_dir()).await?;

    let state = AppState::new(config.clone(), kernel, task_lock, event_tx);
    let app = create_router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("mindbox-server listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
