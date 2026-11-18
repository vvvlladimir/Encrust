//! `encrust-mcp`: the server a desktop client starts, speaking MCP on stdin and stdout and
//! driving the `encrust` command line. See `docs/design/ai-integration.md`.

mod agents;
mod findings;
mod jobs;
mod params;
mod recipe;
mod runner;
mod server;
mod trace;
mod window;

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use crate::agents::{BEAT, Presence};
use crate::jobs::Jobs;
use crate::runner::Cli;
use crate::server::Server;
use crate::trace::Trace;
use crate::window::Window;

const USAGE: &str = "usage: encrust-mcp [--encrust PATH] [--jobs DIR] [--timeout-s SECONDS] \
                     [--trace FILE] [--window FILE | --no-window] [--agents DIR] [--app PATH]";

/// How long one run of the command line may take before it is stopped.
const TIMEOUT: Duration = Duration::from_mins(30);

#[tokio::main]
async fn main() -> Result<()> {
    let (cli, jobs, shown) = parse(std::env::args_os().skip(1))?;
    let presence = shown.2.clone();
    let beating = presence.clone();
    let beat = tokio::spawn(async move {
        let mut every = tokio::time::interval(BEAT);
        loop {
            every.tick().await;
            beating.beat();
        }
    });
    let service = Server::new(cli, jobs, shown)
        .serve(stdio())
        .await
        .context("starting the MCP server on stdio")?;
    let served = service.waiting().await.context("serving MCP");
    beat.abort();
    presence.close();
    served.map(drop)
}

fn parse(
    mut args: impl Iterator<Item = OsString>,
) -> Result<(Cli, Jobs, (Trace, Window, Presence))> {
    let server = std::env::current_exe().context("finding this server's own binary")?;
    let mut program = Cli::program_beside(&server);
    let mut jobs = None;
    let mut trace = None;
    let mut timeout = TIMEOUT;
    let mut window = Some(None);
    let mut agents = None;
    let mut app = None;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("{} takes a value; {USAGE}", arg.to_string_lossy()))
        };
        match arg.to_str() {
            Some("--encrust") => program = value()?.into(),
            Some("--jobs") => jobs = Some(PathBuf::from(value()?)),
            Some("--trace") => trace = Some(PathBuf::from(value()?)),
            Some("--window") => window = Some(Some(PathBuf::from(value()?))),
            Some("--no-window") => window = None,
            Some("--agents") => agents = Some(PathBuf::from(value()?)),
            Some("--app") => app = Some(PathBuf::from(value()?)),
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
        None => cache_dir()
            .context("this system names no cache directory; pass --jobs DIR")?
            .join("mcp-jobs"),
    };
    // The open window writes its file beside the jobs, in the user's cache.
    let agents = match (&window, agents) {
        (_, Some(dir)) => Some(dir),
        (Some(_), None) => cache_dir().map(|dir| dir.join("agents")),
        (None, None) => None,
    };
    let window = window.and_then(|file| file.or_else(|| Some(cache_dir()?.join("window.json"))));
    let shown = (
        Trace::new(trace),
        Window::new(window, apps(app, &server)),
        Presence::new(agents),
    );
    Ok((Cli::new(program, timeout), Jobs::new(jobs), shown))
}

fn cache_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Encrust")?;
    Some(dirs.cache_dir().to_owned())
}

/// The programs `show_in_window` starts the window by, in turn: `--app`, else the window beside
/// this server, on the `PATH`, and where macOS installs it.
fn apps(app: Option<PathBuf>, server: &std::path::Path) -> Vec<PathBuf> {
    if let Some(app) = app {
        return vec![app];
    }
    let name = format!("encrust-gui{}", std::env::consts::EXE_SUFFIX);
    let mut apps: Vec<PathBuf> = server
        .parent()
        .map(|dir| dir.join(&name))
        .into_iter()
        .collect();
    apps.push(PathBuf::from(&name));
    if cfg!(target_os = "macos") {
        apps.push(PathBuf::from(
            "/Applications/Encrust.app/Contents/MacOS/encrust-gui",
        ));
    }
    apps
}
