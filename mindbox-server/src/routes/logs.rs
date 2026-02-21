use std::{convert::Infallible, time::Duration};

use async_stream::stream;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Response, sse::Event, sse::KeepAlive, sse::Sse},
    routing::get,
};
use mindbox_common::{format_stream_event, format_task_event};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::{error::ApiResult, services::task_service::TaskService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/{task_id}/logs", get(get_task_logs))
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    follow: Option<bool>,
}

async fn get_task_logs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<Response> {
    let service = TaskService::new(state.clone());

    if query.follow.unwrap_or(false) {
        let existing = service.read_logs(&task_id).await?;

        let mut rx = state.event_tx.subscribe();
        let task_filter = task_id.clone();

        let body_stream = stream! {
            for line in existing.lines().map(format_stream_event) {
                yield Ok::<Event, Infallible>(Event::default().data(line));
            }

            loop {
                match rx.recv().await {
                    Ok(evt) => {
                        if evt.task_id != task_filter {
                            continue;
                        }
                        let msg = format_task_event(&evt.event);
                        yield Ok::<Event, Infallible>(Event::default().data(msg));
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        };

        let sse = Sse::new(body_stream).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("keepalive"),
        );
        return Ok(sse.into_response());
    }

    let text = service
        .read_logs(&task_id)
        .await?
        .lines()
        .map(format_stream_event)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text.into_response())
}
