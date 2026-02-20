use anyhow::Result;
use futures::StreamExt;
use reqwest_eventsource::{Event, EventSource};

use crate::client::MindboxClient;

pub async fn attach_logs(client: &MindboxClient, project_id: &str, task_id: &str) -> Result<()> {
    let url = client.logs_follow_url(project_id, task_id);
    let mut es = EventSource::get(url);

    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Open) => {
                println!("connected to logs stream");
            }
            Ok(Event::Message(message)) => {
                println!("{}", message.data);
            }
            Err(err) => {
                eprintln!("log stream error: {err}");
                break;
            }
        }
    }

    Ok(())
}
