use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use net_prusalink::Link;
use net_sdcp::{Printer, Transport};
use printer_link::{SendError, State, Wire};
use serde::Serialize;

use crate::commands::GlobalArgs;
use crate::exit::{Cancelled, Exit, Stop};
use crate::{json, progress};

#[derive(clap::Subcommand, Debug)]
pub enum PrinterCommand {
    /// Ask every Elegoo board on the network, and each --address, to introduce itself. A
    /// Prusa machine does not answer; it is named by host in `status` and `send`.
    Discover(DiscoverArgs),
    /// What a printer is doing now.
    Status(MachineArgs),
    /// Send a sliced file to a printer, and start it with --start.
    Send(SendArgs),
}

#[derive(clap::Args, Debug)]
pub struct DiscoverArgs {
    /// A board to ask by address, for a network the broadcast does not cross. Repeatable.
    #[arg(long, value_name = "IP")]
    pub address: Vec<IpAddr>,

    /// How long to listen for answers, seconds.
    #[arg(long, value_name = "S", default_value_t = 2.0)]
    pub wait: f32,
}

/// Which printer, and how it is reached.
#[derive(clap::Args, Debug)]
pub struct MachineArgs {
    /// An Elegoo board's IP address, or with --prusalink a Prusa machine's host.
    pub address: String,

    /// The printer speaks PrusaLink, not SDCP: give --key, or --password and --user.
    #[arg(long)]
    pub prusalink: bool,

    /// The API key a Prusa machine shows when its digest login is off.
    #[arg(long, env = "ENCRUST_PRUSALINK_KEY", hide_env_values = true)]
    pub key: Option<String>,

    /// The digest login's user.
    #[arg(long, env = "ENCRUST_PRUSALINK_USER", default_value = net_prusalink::DEFAULT_USER)]
    pub user: String,

    /// The digest login's password, as the machine's screen shows it. Prefer the variable,
    /// which stays out of the shell's history.
    #[arg(long, env = "ENCRUST_PRUSALINK_PASSWORD", hide_env_values = true)]
    pub password: Option<String>,

    /// How long a board is given to answer, seconds.
    #[arg(long, value_name = "S", default_value_t = 2.0)]
    pub wait: f32,
}

#[derive(clap::Args, Debug)]
pub struct SendArgs {
    /// The sliced file to send. It lands under its own name.
    pub file: PathBuf,

    #[command(flatten)]
    pub machine: MachineArgs,

    /// Start printing it once it is there.
    #[arg(long)]
    pub start: bool,
}

impl PrinterCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        match self {
            Self::Discover(args) => discover(args, global),
            Self::Status(machine) => status(machine, global),
            Self::Send(args) => send(args, global, stop),
        }
        .map(|()| Exit::Success)
    }
}

impl MachineArgs {
    /// Reaches the printer: a board is asked who it is, a Prusa machine is described.
    fn wire(&self) -> Result<Wire> {
        if self.prusalink {
            return Ok(Wire::Prusa(self.link()?));
        }
        let address: IpAddr = self.address.parse().with_context(|| {
            format!(
                "{} is not an IP address; a board is reached by its address, a Prusa \
                 machine by --prusalink",
                self.address
            )
        })?;
        let wait = seconds(self.wait)?;
        net_sdcp::probe(address, wait)
            .with_context(|| format!("cannot ask {address}"))?
            .map(Wire::Sdcp)
            .ok_or_else(|| anyhow!("no board answered at {address} within {} s", self.wait))
    }

    fn link(&self) -> Result<Link> {
        match (&self.key, &self.password) {
            (Some(key), _) => Ok(Link::api_key(&self.address, key)),
            (None, Some(password)) => Ok(Link::digest(&self.address, &self.user, password)),
            (None, None) => bail!(
                "a Prusa machine wants --key or --password, or ENCRUST_PRUSALINK_KEY or \
                 ENCRUST_PRUSALINK_PASSWORD"
            ),
        }
    }
}

fn seconds(value: f32) -> Result<Duration> {
    Duration::try_from_secs_f32(value).with_context(|| format!("{value} s is not a wait"))
}

/// Who the printer is, in every document this command prints.
#[derive(Serialize)]
struct Machine {
    printer: String,
    protocol: &'static str,
}

impl Machine {
    fn of(wire: &Wire) -> Self {
        let protocol = match wire {
            Wire::Sdcp(Printer {
                transport: Transport::WebSocket,
                ..
            }) => "sdcp-3",
            Wire::Sdcp(_) => "sdcp-1",
            Wire::Prusa(_) => "prusalink",
        };
        Self {
            printer: wire.name().to_owned(),
            protocol,
        }
    }
}

fn discover(args: &DiscoverArgs, global: &GlobalArgs) -> Result<()> {
    let found = printer_link::scan(seconds(args.wait)?, &args.address, &[]);
    if global.json {
        return json::print(&serde_json::json!({ "printers": found.boards }));
    }
    if found.boards.is_empty() && global.talks() {
        println!("No board answered within {} s", args.wait);
    }
    for board in &found.boards {
        let wire = Wire::Sdcp(board.clone());
        let protocol = Machine::of(&wire).protocol;
        println!(
            "{}  {}  {}  {protocol}",
            board.address, board.name, board.model
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct StatusReport {
    #[serde(flatten)]
    machine: Machine,
    #[serde(flatten)]
    state: State,
}

fn status(args: &MachineArgs, global: &GlobalArgs) -> Result<()> {
    let wire = args.wire()?;
    let state = printer_link::state(&wire)
        .with_context(|| format!("cannot ask {} what it is doing", wire.name()))?;
    if global.json {
        return json::print(&StatusReport {
            machine: Machine::of(&wire),
            state,
        });
    }
    println!("{}: {}", wire.name(), described(&state));
    Ok(())
}

/// One line for a state: what, on which file, how far, and what went wrong.
fn described(state: &State) -> String {
    let mut line = state.state.clone();
    if let Some(file) = &state.file {
        line += &format!(", {file}");
    }
    if let Some(progress) = state.progress {
        line += &format!(", {:.0}%", progress * 100.0);
    }
    if let Some(remaining_s) = state.remaining_s {
        line += &format!(", {} min left", remaining_s.div_ceil(60));
    }
    if let Some(error) = &state.error {
        line += &format!("; {error}");
    }
    line
}

#[derive(Serialize)]
struct SendReport {
    #[serde(flatten)]
    machine: Machine,
    file: String,
    started: bool,
}

fn send(args: &SendArgs, global: &GlobalArgs, stop: &Stop) -> Result<()> {
    if !args.file.is_file() {
        bail!("{} is not a file", args.file.display());
    }
    let wire = args.machine.wire()?;
    let filename = upload(&wire, args, global, stop)?;
    if args.start {
        stop.check()?;
        printer_link::start_print(&wire, &filename)
            .map_err(|error| cancelled_or(error, stop))
            .with_context(|| format!("cannot start {filename} on {}", wire.name()))?;
    }
    if global.json {
        return json::print(&SendReport {
            machine: Machine::of(&wire),
            file: filename,
            started: args.start,
        });
    }
    if global.talks() {
        match args.start {
            true => println!("{} is printing {filename}", wire.name()),
            false => println!("{filename} is on {}", wire.name()),
        }
    }
    Ok(())
}

/// Sends the file with a bar over its bytes, where the protocol counts them.
fn upload(wire: &Wire, args: &SendArgs, global: &GlobalArgs, stop: &Stop) -> Result<String> {
    let bar = (global.shows_progress() && wire.counts_bytes())
        .then(|| progress::bytes_bar(&format!("to {}", wire.name())));
    let mut progress = |transfer: printer_link::Transfer| {
        if let Some(bar) = &bar {
            bar.set_length(transfer.total_bytes);
            bar.set_position(transfer.sent_bytes);
        }
    };
    let sent = printer_link::upload(wire, &args.file, &mut progress, &|| stop.requested());
    if let Some(bar) = &bar {
        bar.finish_and_clear();
    }
    sent.map_err(|error| cancelled_or(error, stop))
        .with_context(|| format!("cannot send {} to {}", args.file.display(), wire.name()))
}

/// A transfer the user stopped is a cancellation, which exits 130, not a failure.
fn cancelled_or(error: SendError, stop: &Stop) -> anyhow::Error {
    match error.is_cancelled() || stop.requested() {
        true => Cancelled.into(),
        false => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(address: &str, prusalink: bool) -> MachineArgs {
        MachineArgs {
            address: address.to_owned(),
            prusalink,
            key: None,
            user: net_prusalink::DEFAULT_USER.to_owned(),
            password: None,
            wait: 0.1,
        }
    }

    #[test]
    fn a_board_is_reached_by_address_and_never_by_name() {
        let error = machine("saturn.local", false)
            .wire()
            .expect_err("a board has no name to resolve");
        assert!(error.to_string().contains("not an IP address"), "{error:#}");
    }

    #[test]
    fn a_prusa_machine_without_credentials_is_refused_before_it_is_asked() {
        assert!(machine("sl1.local", true).wire().is_err());
        let keyed = MachineArgs {
            key: Some("k3y".to_owned()),
            ..machine("sl1.local", true)
        };
        assert_eq!(
            keyed.wire().expect("a key is enough"),
            Wire::Prusa(Link::api_key("sl1.local", "k3y"))
        );
    }

    #[test]
    fn a_stopped_transfer_exits_as_cancelled_whoever_noticed_first() {
        let stop = Stop::default();
        let failed = cancelled_or(SendError::from(net_sdcp::SdcpError::BoardClosed), &stop);
        assert!(!crate::exit::is_cancelled(&failed));
        stop.request();
        let stopped = cancelled_or(SendError::from(net_sdcp::SdcpError::BoardClosed), &stop);
        assert!(crate::exit::is_cancelled(&stopped));
    }

    #[test]
    fn a_state_reads_as_one_line() {
        let state = State {
            state: "printing".to_owned(),
            file: Some("cube.goo".to_owned()),
            progress: Some(0.25),
            remaining_s: Some(61),
            error: None,
        };
        assert_eq!(described(&state), "printing, cube.goo, 25%, 2 min left");
    }
}
