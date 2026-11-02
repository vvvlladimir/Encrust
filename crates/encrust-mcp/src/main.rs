//! `encrust-mcp`: the server a desktop client starts, speaking MCP on stdin and stdout and
//! driving the `encrust` command line. See `docs/design/ai-integration.md`.

mod jobs;
mod params;
mod recipe;
mod runner;
mod server;

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use crate::jobs::Jobs;
use crate::runner::Cli;
use crate::server::Server;

const USAGE: &str = "usage: encrust-mcp [--encrust PATH] [--jobs DIR] [--timeout-s SECONDS]";

/// How long one run of the command line may take before it is stopped.
const TIMEOUT: Duration = Duration::from_mins(30);

#[tokio::main]
async fn main() -> Result<()> {
    let (cli, jobs) = parse(std::env::args_os().skip(1))?;
    let service = Server::new(cli, jobs)
        .serve(stdio())
        .await
        .context("starting the MCP server on stdio")?;
    service.waiting().await.context("serving MCP")?;
    Ok(())
}

fn parse(mut args: impl Iterator<Item = OsString>) -> Result<(Cli, Jobs)> {
    let server = std::env::current_exe().context("finding this server's own binary")?;
    let mut program = Cli::program_beside(&server);
    let mut jobs = None;
    let mut timeout = TIMEOUT;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("{} takes a value; {USAGE}", arg.to_string_lossy()))
        };
        match arg.to_str() {
            Some("--encrust") => program = value()?.into(),
            Some("--jobs") => jobs = Some(PathBuf::from(value()?)),
            Some("--timeout-s") => {
                let seconds = value()?;
                let seconds: u64 = seconds
                    .to_str()
                    .and_then(|text| text.parse().ok())
                    .with_context(|| format!("--timeout-s takes whole seconds; {USAGE}"))?;
                timeout = Duration::from_secs(seconds);
            }
            _ => bail!("unknown argument {}; {USAGE}", arg.to_string_lossy()),
        }
    }
    let jobs = match jobs {
        Some(jobs) => jobs,
        None => default_jobs_dir()?,
    };
    Ok((Cli::new(program, timeout), Jobs::new(jobs)))
}

fn default_jobs_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Encrust")
        .context("this system names no cache directory; pass --jobs DIR")?;
    Ok(dirs.cache_dir().join("mcp-jobs"))
}
