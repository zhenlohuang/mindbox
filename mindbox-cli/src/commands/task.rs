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
    Create {
        #[arg(long)]
        dataset: String,
        #[arg(long)]
        desc: String,
    },
    Stop {
        task_id: String,
    },
    Attach {
        task_id: String,
    },
    List,
}

pub async fn execute(cmd: TaskCommand, client: &MindboxClient) -> Result<()> {
    match cmd.command {
        TaskSubcommand::Create { dataset, desc } => {
            let response = client.create_task(dataset, desc).await?;
            println!("{}", response.task.id);
        }
        TaskSubcommand::Stop { task_id } => {
            let response = client.cancel_task(&task_id).await?;
            println!("task {} => {:?}", response.task_id, response.status);
        }
        TaskSubcommand::Attach { task_id } => {
            ui::attach_logs(client, &task_id).await?;
        }
        TaskSubcommand::List => {
            let response = client.list_tasks().await?;
            for task in response.tasks {
                println!("{}\t{:?}\t{}", task.id, task.status, task.task_description);
            }
        }
    }

    Ok(())
}
