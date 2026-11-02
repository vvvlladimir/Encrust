//! `encrust-mcp`: the server a desktop client starts, speaking MCP on stdin and stdout.

use std::path::PathBuf;

use anyhow::{Context, Result};
use encrust_mcp::{Cli, Jobs, Server};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

const USAGE: &str = "usage: encrust-mcp [--encrust PATH] [--jobs DIR]";

#[tokio::main]
async fn main() -> Result<()> {
    let (cli, jobs) = parse(std::env::args().skip(1))?;
    let service = Server::new(cli, jobs)
        .serve(stdio())
        .await
        .context("starting the MCP server on stdio")?;
    service.waiting().await.context("serving MCP")?;
    Ok(())
}

/// `--encrust` names the command line to drive, `--jobs` where jobs are kept.
fn parse(mut args: impl Iterator<Item = String>) -> Result<(Cli, Jobs)> {
    let server = std::env::current_exe().context("finding this server's own binary")?;
    let mut cli = Cli::beside(&server);
    let mut jobs = default_jobs_dir()?;
    while let Some(arg) = args.next() {
        let value = || anyhow::anyhow!("{arg} takes a value; {USAGE}");
        match arg.as_str() {
            "--encrust" => cli = Cli::new(args.next().ok_or_else(value)?.into()),
            "--jobs" => jobs = args.next().ok_or_else(value)?.into(),
            _ => anyhow::bail!("unknown argument {arg}; {USAGE}"),
        }
    }
    Ok((cli, Jobs::new(jobs)))
}

fn default_jobs_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Encrust")
        .context("this system names no cache directory; pass --jobs DIR")?;
    Ok(dirs.cache_dir().join("mcp-jobs"))
}
