use anyhow::{Result, bail};
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct SandboxCommand {
    #[command(subcommand)]
    pub command: SandboxSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SandboxSubcommand {
    Start {
        #[arg(long)]
        name: Option<String>,
    },
    Stop {
        #[arg(long)]
        name: Option<String>,
    },
    Destroy {
        #[arg(long)]
        name: Option<String>,
    },
}

pub async fn execute(cmd: SandboxCommand) -> Result<()> {
    match cmd.command {
        SandboxSubcommand::Start { .. } => run_compose(&["up", "-d"]),
        SandboxSubcommand::Stop { .. } => run_compose(&["stop"]),
        SandboxSubcommand::Destroy { .. } => run_compose(&["down", "--remove-orphans"]),
    }
}

fn run_compose(args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("docker")
        .arg("compose")
        .args(args)
        .status()?;
    if !status.success() {
        bail!("docker compose failed with status {status}");
    }
    Ok(())
}
