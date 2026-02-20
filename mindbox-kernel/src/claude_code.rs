use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use mindbox_common::{LogLevel, Metric, TaskStatus};
use serde_json::Value;
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::Mutex as AsyncMutex,
};

use crate::{
    Kernel, TaskContext,
    agent_instructions::{build_agent_md, ensure_agent_files},
    callback::KernelCallback,
    prompt,
};

pub struct ClaudeCodeKernel {
    running: Arc<Mutex<HashMap<String, Child>>>,
}

impl ClaudeCodeKernel {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for ClaudeCodeKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Kernel for ClaudeCodeKernel {
    async fn execute(&self, ctx: TaskContext, callback: Arc<dyn KernelCallback>) -> Result<()> {
        let task_id = ctx.task.id.clone();
        let task_dir = ctx.task_dir.clone();

        let system_prompt = prompt::build_system_prompt(&ctx);
        let user_prompt = prompt::build_user_prompt(&ctx);

        let mut cmd = Command::new("claude");
        cmd.arg("--print")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--system-prompt")
            .arg(system_prompt)
            .arg("--dangerously-skip-permissions")
            .arg(user_prompt)
            .current_dir(&ctx.task_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let agent_md = build_agent_md();
        ensure_agent_files(&task_dir, &agent_md, Some(&ctx.skills_dir)).await?;

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("failed to spawn claude: {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout from claude process"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr from claude process"))?;

        {
            let mut running = self
                .running
                .lock()
                .map_err(|_| anyhow!("running process map poisoned"))?;
            running.insert(task_id.clone(), child);
        }

        let events_path = task_dir.join("events.jsonl");
        let logs_path = task_dir.join("stdout.log");
        let events_file = Arc::new(AsyncMutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(events_path)
                .await?,
        ));
        let logs_file = Arc::new(AsyncMutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(logs_path)
                .await?,
        ));

        let cb1 = callback.clone();
        let ev1 = events_file.clone();
        let lg1 = logs_file.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = process_stream_line(&line, cb1.clone(), ev1.clone(), lg1.clone()).await;
            }
        });

        let cb2 = callback.clone();
        let ev2 = events_file.clone();
        let lg2 = logs_file.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = cb2
                    .log(LogLevel::Error, format!("claude stderr: {line}"))
                    .await;
                let _ = process_stream_line(&line, cb2.clone(), ev2.clone(), lg2.clone()).await;
            }
        });

        let status = loop {
            let maybe_status = {
                let mut running = self
                    .running
                    .lock()
                    .map_err(|_| anyhow!("running process map poisoned"))?;
                if let Some(child) = running.get_mut(&task_id) {
                    child.try_wait()?
                } else {
                    return Err(anyhow!("task process not found: {task_id}"));
                }
            };

            if let Some(status) = maybe_status {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        };

        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        {
            let mut running = self
                .running
                .lock()
                .map_err(|_| anyhow!("running process map poisoned"))?;
            running.remove(&task_id);
        }

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("claude exited with status {status}"))
        }
    }

    fn cancel(&self, task_id: &str) {
        if let Ok(mut running) = self.running.lock()
            && let Some(child) = running.get_mut(task_id)
        {
            let _ = child.start_kill();
        }
    }

    fn name(&self) -> &str {
        "claude-code"
    }
}

async fn process_stream_line(
    line: &str,
    callback: Arc<dyn KernelCallback>,
    events_file: Arc<AsyncMutex<tokio::fs::File>>,
    logs_file: Arc<AsyncMutex<tokio::fs::File>>,
) -> Result<()> {
    {
        let mut file = logs_file.lock().await;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
    }

    {
        let mut file = events_file.lock().await;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
    }

    if let Ok(value) = serde_json::from_str::<Value>(line) {
        let event_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("log")
            .to_lowercase();

        match event_type.as_str() {
            "status" | "status_update" => {
                let status = value
                    .get("status")
                    .cloned()
                    .and_then(|s| serde_json::from_value::<TaskStatus>(s).ok())
                    .unwrap_or(TaskStatus::Running);
                let message = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("status update")
                    .to_string();
                callback.status_update(status, message).await;
            }
            "metric" => {
                if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
                    let metric = Metric {
                        name: name.to_string(),
                        value: value.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        step: value.get("step").and_then(|v| v.as_u64()),
                        timestamp: Utc::now(),
                    };
                    callback.metric(metric).await;
                }
            }
            "error" => {
                let message = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or(line)
                    .to_string();
                callback.error(message).await;
            }
            _ => {
                let message = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or(line)
                    .to_string();
                callback.log(LogLevel::Info, message).await;
            }
        }
    } else {
        callback.log(LogLevel::Info, line.to_string()).await;
    }

    Ok(())
}
