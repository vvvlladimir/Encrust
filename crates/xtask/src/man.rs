//! `cargo xtask man`: the command line's man pages, one per subcommand, from its own clap
//! definition so they cannot drift from `--help`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::CommandFactory;

#[derive(clap::Args)]
pub struct Args {
    /// Where the pages are written.
    #[arg(long, default_value = "target/man")]
    out: PathBuf,
}

pub fn run(args: &Args) -> Result<()> {
    std::fs::create_dir_all(&args.out)
        .with_context(|| format!("cannot create {}", args.out.display()))?;
    clap_mangen::generate_to(encrust_cli::Cli::command(), &args.out)
        .with_context(|| format!("cannot write the man pages into {}", args.out.display()))?;
    println!("{}", args.out.display());
    Ok(())
}
