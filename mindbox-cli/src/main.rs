mod client;
mod commands;
mod ui;

use clap::{Parser, Subcommand};

use crate::{
    client::MindboxClient,
    commands::{sandbox, task},
};

#[derive(Parser, Debug)]
#[command(name = "mindbox")]
#[command(about = "Mindbox CLI")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Sandbox(sandbox::SandboxCommand),
    Task(task::TaskCommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = MindboxClient::new(cli.server);

    match cli.command {
        Commands::Sandbox(cmd) => sandbox::execute(cmd).await,
        Commands::Task(cmd) => task::execute(cmd, &client).await,
    }
}
