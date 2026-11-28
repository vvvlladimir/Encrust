//! `encrust-mcp`: the server a desktop client starts, speaking MCP on stdin and stdout and
//! driving the `encrust` command line. See `docs/design/ai-integration.md`.

mod agents;
mod findings;
mod jobs;
mod params;
mod prompts;
mod recipe;
mod runner;
mod schema;
mod server;
mod trace;
mod window;

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use agent_link::cache_dir;
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
        let mut value = || -> Result<Option<PathBuf>> {
            let value = args
                .next()
                .with_context(|| format!("{} takes a value; {USAGE}", arg.to_string_lossy()))?;
            Ok(given(value).map(PathBuf::from))
        };
        match arg.to_str() {
            Some("--encrust") => program = value()?.unwrap_or(program),
            Some("--jobs") => jobs = value()?,
            Some("--trace") => trace = value()?,
            Some("--window") => window = Some(value()?),
            Some("--no-window") => window = None,
            Some("--agents") => agents = value()?,
            Some("--app") => app = value()?,
            Some("--timeout-s") => {
                if let Some(seconds) = value()? {
                    let seconds: u64 = seconds
                        .to_str()
                        .and_then(|text| text.parse().ok())
                        .with_context(|| format!("--timeout-s takes whole seconds; {USAGE}"))?;
                    timeout = Duration::from_secs(seconds);
                }
            }
            _ => bail!("unknown argument {}; {USAGE}", arg.to_string_lossy()),
        }
    }
    let jobs = match jobs {
        Some(jobs) => jobs,
        None => cache_dir()
            .context("this system names no cache directory; pass --jobs DIR")?
            .join(agent_link::JOBS),
    };
    // The open window writes its file beside the jobs, in the user's cache.
    let agents = match (&window, agents) {
        (_, Some(dir)) => Some(dir),
        (Some(_), None) => cache_dir().map(|dir| dir.join(agent_link::AGENTS)),
        (None, None) => None,
    };
    let window =
        window.and_then(|file| file.or_else(|| Some(cache_dir()?.join(agent_link::WINDOW_FILE))));
    // On stderr, which a client keeps in its log of the server, so a stale build is told.
    eprintln!(
        "encrust-mcp {}: jobs in {}, agents in {}, window file {}",
        env!("CARGO_PKG_VERSION"),
        jobs.display(),
        agents
            .as_ref()
            .map_or_else(|| "none".into(), |dir| dir.display().to_string()),
        window
            .as_ref()
            .map_or_else(|| "none".into(), |file| file.display().to_string()),
    );
    let shown = (
        Trace::new(trace),
        Window::new(window, apps(app, &server)),
        Presence::new(agents),
    );
    Ok((Cli::new(program, timeout), Jobs::new(jobs), shown))
}

/// A flag's value, or none where it is empty or a `${...}` placeholder its client left unfilled:
/// a bundle's setting the user never set reaches the server so (ADR 0455).
fn given(value: OsString) -> Option<OsString> {
    let text = value.to_string_lossy();
    let unfilled = text.starts_with("${") && text.ends_with('}');
    (!text.is_empty() && !unfilled).then_some(value)
}

/// The programs `show_in_window` starts the window by, in turn: `--app`, else the window beside
/// this server, on the `PATH`, and where the installers of macOS and Windows put it.
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
    // The Windows installer's folder per user, which it puts on no `PATH`.
    if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)) {
        apps.push(PathBuf::from(local).join("Encrust").join(&name));
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unfilled_or_empty_setting_is_no_value() {
        assert_eq!(given("${user_config.app}".into()), None);
        assert_eq!(given(OsString::new()), None);
        assert_eq!(
            given("/opt/encrust-gui".into()),
            Some("/opt/encrust-gui".into())
        );
    }

    #[test]
    fn a_setting_left_unfilled_leaves_its_flag_unset() {
        let args = [
            "--no-window",
            "--app",
            "${user_config.app}",
            "--timeout-s",
            "${user_config.timeout_s}",
        ];
        let parsed = parse(args.into_iter().map(OsString::from));
        assert!(parsed.is_ok(), "{:?}", parsed.err());
    }
}
