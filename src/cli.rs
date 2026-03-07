use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lol-autoq",
    version,
    about = "Auto-accept queues and pick champions in League of Legends"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the auto-queue: accepts ready checks and picks champions automatically.
    Start,
    /// Interactively configure champion preferences per lane.
    Configure,
}
