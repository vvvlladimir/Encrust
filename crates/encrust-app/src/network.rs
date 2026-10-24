//! The printers this machine can reach, and what the window is sending them.

mod errands;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use core_pipeline::SlicedFormat;
use net_sdcp::Printer;
use printer_profiles::{Connection, NetworkMachine, PrinterProfile};

use printer_link::Found;

pub use errands::{Errand, Extras, Picture};

use crate::i18n::tr;
use crate::job::SendJob;

mod reach;
mod scan;
mod send;
mod target;
mod watch;
pub use reach::{Reach, Trouble};
pub use scan::PrusaDraft;
pub use send::temporary_path;
use target::resolve;
pub use target::{Prusa, Sent, Target};
pub use watch::Reading;
use watch::Watch;

/// What a connection is called on its segment: a protocol by its own name, the lack of one
/// in the window's language.
pub fn connection_label(connection: Connection) -> String {
    match connection {
        Connection::None => tr!("network-usb-only"),
        Connection::Sdcp | Connection::PrusaLink => connection.label().to_owned(),
    }
}

/// Long enough for every printer on the segment to answer, short enough to wait through.
const SCAN_WINDOW: Duration = Duration::from_secs(2);

/// How often a watched printer is asked what it is doing.
const WATCH_PERIOD: Duration = Duration::from_secs(3);

/// How long a list of printers in view goes before the network is asked again, in seconds.
const RESCAN_S: f64 = 30.0;

/// What the window knows about the network: who is out there, and the errand running.
#[derive(Default)]
pub struct Network {
    /// Boards that answered the last scan, plus those reached by address and those a
    /// printer profile is bound to, which stay listed between runs.
    pub printers: Vec<Printer>,
    /// Prusa machines the user has set up, in the order they were added.
    pub prusa: Vec<Prusa>,
    /// The catalogue id of the printer in hand, which is what a choice is kept under.
    profile: Option<String>,
    /// The printers the profile in hand lists, and what they speak; see ADR 0265.
    listed: Vec<NetworkMachine>,
    speaks: Connection,
    /// Which listed printer each profile last sent to, by catalogue id, as its address.
    chosen: BTreeMap<String, String>,
    /// What the user calls each printer in any profile, by its key and by its address.
    names: BTreeMap<String, String>,
    /// Addresses typed by hand, because a broadcast does not cross every network.
    pub manual: Vec<IpAddr>,
    scan: Option<Receiver<Found>>,
    /// The keys of the machines that answered the last scan.
    seen: BTreeSet<String>,
    /// Whether any scan has finished since the window opened.
    asked: bool,
    /// When the window last asked on its own, in seconds on its clock.
    asked_at: Option<f64>,
    /// Why the last broadcast went out on no interface.
    pub trouble: Option<Trouble>,
    /// Each address typed by hand that nothing answered at the last scan.
    pub silent: Vec<(IpAddr, Option<Trouble>)>,
    /// Every printer listed, watched by its key.
    watches: BTreeMap<String, Watch>,
    /// What has been asked of each printer besides its state, by its key.
    extras: BTreeMap<String, Extras>,
    /// Errands still out on their threads.
    out: Vec<errands::Out>,
    /// Whether the window keeps off the network unless told to: the panel pictures must not
    /// depend on what is on the desk's network.
    pub hold: bool,
    /// The key of the printer whose sheet is open over the window.
    pub open: Option<String>,
    pub job: Option<SendJob>,
    pub sent: Option<Sent>,
    /// The address being typed, until it parses and is added.
    pub draft: String,
    /// The Prusa machine being described, until it is added.
    pub prusa_draft: PrusaDraft,
}

impl Network {
    /// Every address and name the window knows a machine by, including the ones being
    /// typed. A bug report keeps all of them out of the failure it quotes, which is the
    /// only reason this list exists; see `crate::report`.
    pub fn hosts(&self) -> Vec<String> {
        self.printers
            .iter()
            .map(|printer| printer.address.to_string())
            .chain(self.prusa.iter().map(|prusa| prusa.link.host.clone()))
            .chain(self.manual.iter().map(ToString::to_string))
            .chain([self.draft.clone(), self.prusa_draft.host.clone()])
            .collect()
    }

    /// Every printer the window could send to, boards first.
    pub fn targets(&self) -> Vec<Target<'_>> {
        let boards = self.printers.iter().map(Target::Sdcp);
        boards.chain(self.prusa.iter().map(Target::Prusa)).collect()
    }

    /// The printer the Slice button sends to: the listed one last chosen, else the first
    /// listed, once it has been found on the network.
    pub fn target(&self) -> Option<Target<'_>> {
        self.resolve(self.chosen_machine()?)
    }

    /// The printers the profile in hand lists, in order.
    pub fn listed(&self) -> &[NetworkMachine] {
        &self.listed
    }

    /// The listed printer the Slice button sends to, found on the network or not.
    pub fn chosen_machine(&self) -> Option<&NetworkMachine> {
        let chosen = self.profile.as_ref().and_then(|id| self.chosen.get(id));
        chosen
            .and_then(|address| self.listed.iter().find(|m| m.address == *address))
            .or(self.listed.first())
    }

    /// Where a printer some profile lists is on the network: the board that answered with
    /// its id or from its address, or the Prusa machine set up at its host.
    pub fn resolve(&self, machine: &NetworkMachine) -> Option<Target<'_>> {
        resolve(&self.printers, &self.prusa, machine)
    }

    /// Whether any scan has finished since the window opened.
    pub fn has_asked(&self) -> bool {
        self.asked
    }

    /// Whether a listed printer answers, and whether one not found was looked for.
    pub fn reach_of(&self, machine: &NetworkMachine) -> Reach {
        match self.resolve(machine) {
            Some(target) => self.reach(&target),
            None if self.asked => Reach::Silent,
            None => Reach::Unasked,
        }
    }

    /// Sends the profile in hand's files to the listed printer at `address` from now on.
    pub fn choose(&mut self, address: String) {
        if let Some(id) = self.profile.clone() {
            self.chosen.insert(id, address);
        }
    }

    /// Which listed printer each profile last sent to, for the preferences file.
    pub fn chosen(&self) -> &BTreeMap<String, String> {
        &self.chosen
    }

    /// What `target` is called in front of the user: the name a profile gave it, else
    /// what it calls itself.
    pub fn label(&self, target: &Target<'_>) -> String {
        self.names
            .get(&target.key())
            .or_else(|| self.names.get(&target.address()))
            .cloned()
            .unwrap_or_else(|| target.name())
    }

    /// Learns what every profile calls its printers, so the ring and the sheet use the
    /// names the profiles give them.
    pub fn learn_names<'a>(&mut self, profiles: impl IntoIterator<Item = &'a PrinterProfile>) {
        self.names.clear();
        for profile in profiles {
            for machine in profile
                .machines
                .iter()
                .filter(|m| !m.name.trim().is_empty())
            {
                if let Some(board) = &machine.board {
                    self.names
                        .insert(format!("sdcp:{board}"), machine.name.clone());
                }
                let address = machine.address.trim().to_owned();
                self.names.insert(address, machine.name.clone());
            }
        }
    }

    /// Whether `target` is answering: what its watch heard last, else the last scan.
    pub fn reach(&self, target: &Target<'_>) -> Reach {
        match self.reading(target) {
            Some(Reading::State(_)) => return Reach::Answered,
            Some(Reading::Failed(_)) => return Reach::Silent,
            None => {}
        }
        match (self.seen.contains(&target.key()), self.asked) {
            (true, _) => Reach::Answered,
            (false, true) => Reach::Silent,
            (false, false) => Reach::Unasked,
        }
    }

    /// What `target` said when its watch last asked, or `None` before it has answered.
    pub fn reading(&self, target: &Target<'_>) -> Option<&Reading> {
        self.watches.get(&target.key())?.last.as_ref()
    }

    /// Why the bound printer will not take a print now, as the press beside it says it.
    pub fn busy(&self) -> Option<String> {
        let target = self.target()?;
        let Some(Reading::State(state)) = self.reading(&target) else {
            return None;
        };
        let name = self.label(&target);
        state.busy.then(|| match &state.file {
            Some(file) => tr!(
                "network-printing",
                printer = name.clone(),
                file = file.clone()
            ),
            None => tr!(
                "network-busy",
                name = name.clone(),
                state = state.state.clone()
            ),
        })
    }

    /// The printers answering now, boards first: what the top bar's ring lists. One that
    /// stopped answering drops out at once, and comes back when a scan hears it again.
    pub fn in_view(&self) -> Vec<Target<'_>> {
        let targets = self.targets().into_iter();
        targets
            .filter(|target| self.reach(target) == Reach::Answered)
            .collect()
    }

    /// The printer the top bar's ring stands for, among those in view: the one the printer
    /// in hand is bound to, else the first one printing, else the first one.
    pub fn current(&self) -> Option<Target<'_>> {
        let targets = self.in_view();
        let bound = self.target().map(|bound| bound.key());
        if let Some(bound) = targets.iter().find(|t| Some(t.key()) == bound) {
            return Some(*bound);
        }
        let printing = targets.iter().find(
            |target| matches!(self.reading(target), Some(Reading::State(state)) if state.busy),
        );
        printing.or(targets.first()).copied()
    }

    /// What has been asked of the printer under `key` besides its state.
    pub fn extras(&self, key: &str) -> Option<&Extras> {
        self.extras.get(key)
    }

    /// Stands the window under the printer profile now in hand: the printers it lists,
    /// what they speak, and which of them it last sent to.
    pub fn bind_to(&mut self, id: Option<String>, profile: Option<&PrinterProfile>) {
        self.profile = id;
        self.listed = profile.map(|p| p.machines.clone()).unwrap_or_default();
        self.speaks = profile.map_or(Connection::None, |p| p.connection);
    }

    /// Puts back what the preferences file remembered: the boards last listed and which
    /// printer each profile sent to.
    pub fn restore(&mut self, boards: Vec<Printer>, chosen: BTreeMap<String, String>) {
        self.printers = boards;
        self.chosen = chosen;
    }

    /// The boards worth writing to the preferences file: the ones the profile in hand
    /// lists, so its printers are named before any scan has answered.
    pub fn bound_boards(&self) -> Vec<Printer> {
        self.printers
            .iter()
            .filter(|printer| {
                self.listed
                    .iter()
                    .any(|machine| resolve(std::slice::from_ref(*printer), &[], machine).is_some())
            })
            .cloned()
            .collect()
    }

    /// Why the chosen printer cannot take what the window is set to write.
    ///
    /// Only a mismatch nothing has to be asked about is caught here. What an Elegoo board
    /// takes is what the board itself reports, which costs a connection, so that check
    /// happens on the errand's own thread.
    pub fn blocker(&self, format: SlicedFormat) -> Option<String> {
        match self.target() {
            Some(Target::Prusa(_)) if !matches!(format, SlicedFormat::Sl1(_)) => {
                Some(tr!("network-prusa-sl1"))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
