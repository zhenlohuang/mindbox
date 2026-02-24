use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};
use futures::StreamExt;
use mindbox_common::{SystemResources, Task, TaskEvent};
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
    LogLine { filename: String, line: String },
    LogFileDiscovered(String),
    ScrollUp,
    ScrollDown,
    Tick,
    SystemResources(Box<SystemResources>),
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

pub fn spawn_resource_poller(client: MindboxClient, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut poll_interval = time::interval(Duration::from_secs(2));
        poll_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        loop {
            poll_interval.tick().await;

            match client.get_system_resources().await {
                Ok(resources) => {
                    if tx
                        .send(AppEvent::SystemResources(Box::new(resources)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    if tx
                        .send(AppEvent::RawLog(format!(
                            "[error] failed to poll system resources: {err}"
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

pub fn spawn_log_dir_watcher(task_id: String, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_millis(500));
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let mut logs_dir: Option<PathBuf> = None;
        let mut file_states: HashMap<String, (u64, String)> = HashMap::new();

        loop {
            ticker.tick().await;

            if logs_dir.is_none() {
                logs_dir = resolve_logs_dir(&task_id).await;
                if logs_dir.is_none() {
                    continue;
                }
            }

            let current_logs_dir = match logs_dir.as_ref() {
                Some(path) => path,
                None => continue,
            };

            let Ok(mut entries) = fs::read_dir(current_logs_dir).await else {
                logs_dir = None;
                file_states.clear();
                continue;
            };

            let mut files = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let Some(filename) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(ToOwned::to_owned)
                else {
                    continue;
                };

                if filename == "kernel.log" {
                    continue;
                }

                let Ok(metadata) = entry.metadata().await else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }

                files.push((filename, path));
            }
            files.sort_by(|left, right| left.0.cmp(&right.0));

            let present_files: HashSet<&str> = files
                .iter()
                .map(|(filename, _)| filename.as_str())
                .collect();
            file_states.retain(|filename, _| present_files.contains(filename.as_str()));

            for (filename, _) in &files {
                if file_states.contains_key(filename) {
                    continue;
                }
                file_states.insert(filename.clone(), (0, String::new()));
                if tx
                    .send(AppEvent::LogFileDiscovered(filename.clone()))
                    .await
                    .is_err()
                {
                    return;
                }
            }

            for (filename, path) in files {
                let (mut offset, mut pending) = file_states
                    .remove(&filename)
                    .unwrap_or_else(|| (0, String::new()));

                let keep_running =
                    tail_log_file(&path, &filename, &tx, &mut offset, &mut pending).await;

                file_states.insert(filename, (offset, pending));
                if !keep_running {
                    return;
                }
            }
        }
    })
}

async fn tail_log_file(
    path: &Path,
    filename: &str,
    tx: &mpsc::Sender<AppEvent>,
    offset: &mut u64,
    pending: &mut String,
) -> bool {
    let Ok(metadata) = fs::metadata(path).await else {
        *offset = 0;
        pending.clear();
        return true;
    };

    if metadata.len() < *offset {
        *offset = 0;
        pending.clear();
    }

    let Ok(mut file) = File::open(path).await else {
        return true;
    };

    if file.seek(std::io::SeekFrom::Start(*offset)).await.is_err() {
        return true;
    }

    let mut chunk = Vec::new();
    if file.read_to_end(&mut chunk).await.is_err() || chunk.is_empty() {
        return true;
    }

    *offset = (*offset).saturating_add(chunk.len() as u64);
    pending.push_str(&String::from_utf8_lossy(&chunk));

    while let Some(pos) = pending.find('\n') {
        let line = pending[..pos].trim_end_matches('\r').to_string();
        let remaining = pending[pos + 1..].to_string();
        *pending = remaining;
        if tx
            .send(AppEvent::LogLine {
                filename: filename.to_string(),
                line,
            })
            .await
            .is_err()
        {
            return false;
        }
    }

    true
}

pub async fn resolve_logs_dir(task_id: &str) -> Option<PathBuf> {
    let fixed_candidates = [
        format!("data/mindbox/tasks/{task_id}/logs"),
        format!("data/mindbox/projects/default/tasks/{task_id}/logs"),
        format!("/mindbox/tasks/{task_id}/logs"),
        format!("/mindbox/projects/default/tasks/{task_id}/logs"),
    ];

    for candidate in fixed_candidates {
        let path = PathBuf::from(candidate);
        let Ok(metadata) = fs::metadata(&path).await else {
            continue;
        };
        if metadata.is_dir() {
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
                .join("logs");
            let Ok(metadata) = fs::metadata(&path).await else {
                continue;
            };
            if metadata.is_dir() {
                return Some(path);
            }
        }
    }

    None
}

fn is_terminal_message(message: &str) -> bool {
    if let Ok(event) = serde_json::from_str::<mindbox_common::TaskEvent>(message)
        && let mindbox_common::TaskEvent::StatusUpdate { status, .. } = event
    {
        return matches!(
            status,
            mindbox_common::TaskStatus::Completed
                | mindbox_common::TaskStatus::Failed
                | mindbox_common::TaskStatus::Cancelled
        );
    }
    false
}
