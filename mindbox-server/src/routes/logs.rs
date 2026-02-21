use std::{cmp::Ordering, convert::Infallible, time::Duration};

use async_stream::stream;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Response, sse::Event, sse::KeepAlive, sse::Sse},
    routing::get,
};
use chrono::{DateTime, Utc};
use mindbox_common::{
    format_stream_event, format_task_event, parse_log_timestamp, task_event_timestamp,
};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::{error::ApiResult, services::task_service::TaskService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/projects/{project_id}/tasks/{task_id}/logs",
        get(get_task_logs),
    )
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    follow: Option<bool>,
}

#[derive(Debug)]
struct ReplayLine {
    timestamp: Option<DateTime<Utc>>,
    sequence: usize,
    text: String,
}

async fn get_task_logs(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<Response> {
    let service = TaskService::new(state.clone());

    if query.follow.unwrap_or(false) {
        let existing_logs = service.read_logs(&project_id, &task_id).await?;
        let existing_events = service.list_events(&project_id, &task_id).await?;
        let existing = merge_replay_lines(existing_logs, existing_events);

        let mut rx = state.event_tx.subscribe();
        let project_filter = project_id.clone();
        let task_filter = task_id.clone();

        let body_stream = stream! {
            for line in existing {
                yield Ok::<Event, Infallible>(Event::default().data(line));
            }

            loop {
                match rx.recv().await {
                    Ok(evt) => {
                        if evt.project_id != project_filter || evt.task_id != task_filter {
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
        .read_logs(&project_id, &task_id)
        .await?
        .lines()
        .map(format_stream_event)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text.into_response())
}

fn merge_replay_lines(raw_logs: String, events: Vec<mindbox_common::TaskEvent>) -> Vec<String> {
    let mut sequence = 0usize;
    let mut lines = Vec::new();

    for line in raw_logs.lines() {
        lines.push(ReplayLine {
            timestamp: parse_log_timestamp(line),
            sequence,
            text: format_stream_event(line),
        });
        sequence += 1;
    }

    for event in events {
        lines.push(ReplayLine {
            timestamp: Some(task_event_timestamp(&event)),
            sequence,
            text: format_task_event(&event),
        });
        sequence += 1;
    }

    lines.sort_by(compare_replay_lines);
    lines.into_iter().map(|line| line.text).collect()
}

fn compare_replay_lines(left: &ReplayLine, right: &ReplayLine) -> Ordering {
    match (left.timestamp, right.timestamp) {
        (Some(a), Some(b)) => a.cmp(&b).then(left.sequence.cmp(&right.sequence)),
        _ => left.sequence.cmp(&right.sequence),
    }
}
