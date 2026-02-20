use anyhow::Result;
use clap::{Args, Subcommand};

use crate::client::MindboxClient;

#[derive(Debug, Args)]
pub struct ProjectCommand {
    #[command(subcommand)]
    pub command: ProjectSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectSubcommand {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    List,
}

pub async fn execute(cmd: ProjectCommand, client: &MindboxClient) -> Result<()> {
    match cmd.command {
        ProjectSubcommand::Create { name, description } => {
            let project = client.create_project(name, description).await?;
            println!("created project {}", project.id);
        }
        ProjectSubcommand::List => {
            let resp = client.list_projects().await?;
            for project in resp.projects {
                println!("{}\t{}", project.id, project.description);
            }
        }
    }

    Ok(())
}
