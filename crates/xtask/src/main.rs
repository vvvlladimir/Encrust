//! Repository tasks run by hand. What `gen-profiles` reads and may invent is in
//! `docs/design/profiles.md`; how `web` builds the browser window, `docs/design/web-build.md`.

mod containers;
mod cross_check;
mod ini;
mod machine;
mod man;
mod profiles;
mod web;

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
    /// Builds the window for a browser, with threads, into a directory ready to serve.
    Web(web::Args),
    /// Writes the command line's man pages.
    Man(man::Args),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::GenProfiles(args) => profiles::run(&args),
        Command::Web(args) => web::run(&args),
        Command::Man(args) => man::run(&args),
    }
}
