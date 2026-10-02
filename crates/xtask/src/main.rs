//! Repository tasks run by hand. What `gen-profiles` reads and what it may invent is in
//! `docs/design/profiles.md`.

mod containers;
mod cross_check;
mod ini;
mod machine;
mod profiles;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Repository tasks for Encrust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribes a directory of printer profiles into the shipped catalogue.
    GenProfiles(profiles::Args),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::GenProfiles(args) => profiles::run(&args),
    }
}
