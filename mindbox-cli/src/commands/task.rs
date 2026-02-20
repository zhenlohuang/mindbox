use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{client::MindboxClient, ui};

#[derive(Debug, Args)]
pub struct TaskCommand {
    #[command(subcommand)]
    pub command: TaskSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TaskSubcommand {
    Start {
        #[arg(long, default_value = "default")]
        project: String,
        #[arg(long, default_value_t = false)]
        create_project: bool,
        #[arg(long)]
        dataset: String,
        #[arg(long)]
        task: String,
    },
    Stop {
        #[arg(long, default_value = "default")]
        project: String,
        task_id: String,
    },
    Attach {
        #[arg(long, default_value = "default")]
        project: String,
        task_id: String,
    },
    List {
        #[arg(long, default_value = "default")]
        project: String,
    },
}

pub async fn execute(cmd: TaskCommand, client: &MindboxClient) -> Result<()> {
    match cmd.command {
        TaskSubcommand::Start {
            project,
            create_project,
            dataset,
            task,
        } => {
            if create_project {
                let _ = client
                    .create_project(project.clone(), Some("Created from CLI".to_string()))
                    .await;
            }

            let response = client.create_task(&project, dataset, task).await?;
            println!("task started: {}", response.task.id);
            ui::attach_logs(client, &project, &response.task.id).await?;
        }
        TaskSubcommand::Stop { project, task_id } => {
            let response = client.cancel_task(&project, &task_id).await?;
            println!("task {} => {:?}", response.task_id, response.status);
        }
        TaskSubcommand::Attach { project, task_id } => {
            ui::attach_logs(client, &project, &task_id).await?;
        }
        TaskSubcommand::List { project } => {
            let response = client.list_tasks(&project).await?;
            for task in response.tasks {
                println!("{}\t{:?}\t{}", task.id, task.status, task.task_description);
            }
        }
    }

    Ok(())
}
