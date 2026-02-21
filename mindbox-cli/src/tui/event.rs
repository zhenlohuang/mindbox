use std::{path::PathBuf, time::Duration};

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};
use futures::StreamExt;
use mindbox_common::{Task, TaskEvent};
use reqwest_eventsource::{Event, EventSource};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncSeekExt},
    sync::mpsc,
    task::JoinHandle,
    time,
};

use crate::client::MindboxClient;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    TaskEvent(TaskEvent),
    RawLog(String),
    StreamConnected,
    StreamEnded,
    TaskInfo(Box<Task>),
    TrainLog(String),
    ScrollUp,
    ScrollDown,
    Tick,
}

pub fn spawn_terminal_events(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        loop {
            match event::poll(Duration::from_millis(100)) {
                Ok(false) => continue,
                Ok(true) => {}
                Err(_) => break,
            }

            let event = match event::read() {
                Ok(event) => event,
                Err(_) => break,
            };

            let app_event = match event {
                CrosstermEvent::Key(key) => AppEvent::Key(key),
                CrosstermEvent::Resize(width, height) => AppEvent::Resize(width, height),
                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => AppEvent::ScrollUp,
                    MouseEventKind::ScrollDown => AppEvent::ScrollDown,
                    _ => continue,
                },
                _ => continue,
            };

            if tx.blocking_send(app_event).is_err() {
                break;
            }
        }
    })
}

pub fn spawn_sse_reader(
    client: MindboxClient,
    task_id: String,
    tx: mpsc::Sender<AppEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let url = client.logs_follow_url(&task_id);
        let mut es = EventSource::get(url);

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {
                    if tx.send(AppEvent::StreamConnected).await.is_err() {
                        break;
                    }
                }
                Ok(Event::Message(message)) => {
                    let data = message.data;
                    if data.trim().eq_ignore_ascii_case("keepalive") {
                        continue;
                    }

                    let send_result =
                        if let Ok(task_event) = serde_json::from_str::<TaskEvent>(&data) {
                            tx.send(AppEvent::TaskEvent(task_event)).await
                        } else {
                            tx.send(AppEvent::RawLog(data.clone())).await
                        };
                    if send_result.is_err() {
                        break;
                    }

                    if is_terminal_message(&data) {
                        let _ = tx.send(AppEvent::StreamEnded).await;
                        es.close();
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx
                        .send(AppEvent::RawLog(format!("[error] log stream error: {err}")))
                        .await;
                    let _ = tx.send(AppEvent::StreamEnded).await;
                    break;
                }
            }
        }
    })
}

pub fn spawn_task_poller(
    client: MindboxClient,
    task_id: String,
    tx: mpsc::Sender<AppEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut poll_interval = time::interval(Duration::from_secs(5));
        poll_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        loop {
            poll_interval.tick().await;

            match client.get_task(&task_id).await {
                Ok(response) => {
                    if tx
                        .send(AppEvent::TaskInfo(Box::new(response.task)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    if tx
                        .send(AppEvent::RawLog(format!(
                            "[error] failed to poll task info: {err}"
                        )))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    })
}

pub fn spawn_tick(tx: mpsc::Sender<AppEvent>, duration: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = time::interval(duration);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    })
}

pub fn spawn_train_log_tailer(task_id: String, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_millis(300));
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let mut path: Option<PathBuf> = None;
        let mut offset: u64 = 0;
        let mut pending = String::new();

        loop {
            ticker.tick().await;

            if path.is_none() {
                path = resolve_train_log_path(&task_id).await;
                if path.is_none() {
                    continue;
                }
            }

            let current_path = match path.as_ref() {
                Some(path) => path,
                None => continue,
            };

            let Ok(metadata) = fs::metadata(current_path).await else {
                path = None;
                offset = 0;
                pending.clear();
                continue;
            };

            if metadata.len() < offset {
                offset = 0;
                pending.clear();
            }

            let Ok(mut file) = File::open(current_path).await else {
                continue;
            };

            if file.seek(std::io::SeekFrom::Start(offset)).await.is_err() {
                continue;
            }

            let mut chunk = Vec::new();
            if file.read_to_end(&mut chunk).await.is_err() {
                continue;
            }
            if chunk.is_empty() {
                continue;
            }

            offset = offset.saturating_add(chunk.len() as u64);
            pending.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = pending.find('\n') {
                let line = pending[..pos].trim_end_matches('\r').to_string();
                pending = pending[pos + 1..].to_string();
                if tx.send(AppEvent::TrainLog(line)).await.is_err() {
                    return;
                }
            }
        }
    })
}

async fn resolve_train_log_path(task_id: &str) -> Option<PathBuf> {
    let fixed_candidates = [
        format!("data/mindbox/tasks/{task_id}/logs/train.log"),
        format!("data/mindbox/projects/default/tasks/{task_id}/logs/train.log"),
        format!("/mindbox/tasks/{task_id}/logs/train.log"),
        format!("/mindbox/projects/default/tasks/{task_id}/logs/train.log"),
    ];

    for candidate in fixed_candidates {
        let path = PathBuf::from(candidate);
        if fs::metadata(&path).await.is_ok() {
            return Some(path);
        }
    }

    for projects_root in ["data/mindbox/projects", "/mindbox/projects"] {
        let Ok(mut entries) = fs::read_dir(projects_root).await else {
            continue;
        };

        while let Ok(Some(project_entry)) = entries.next_entry().await {
            let path = project_entry
                .path()
                .join("tasks")
                .join(task_id)
                .join("logs")
                .join("train.log");
            if fs::metadata(&path).await.is_ok() {
                return Some(path);
            }
        }
    }

    None
}

fn is_terminal_message(message: &str) -> bool {
    let text = message.trim().to_ascii_lowercase();
    matches!(
        text.as_str(),
        "task completed" | "task failed" | "task cancelled" | "task canceled"
    ) || text.starts_with("[status: completed]")
        || text.starts_with("[status: failed]")
        || text.starts_with("[status: cancelled]")
        || text.starts_with("[status: canceled]")
}
