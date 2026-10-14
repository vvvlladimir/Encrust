//! The printers this machine can reach, and what the window is sending them.

mod errands;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use core_pipeline::SlicedFormat;
use net_prusalink::Link;
use net_sdcp::Printer;
use printer_profiles::{Connection, NetworkMachine, PrinterProfile};

use printer_link::{Found, SendError, State, Wire};

pub use errands::{Errand, Extras, Picture};

use crate::job::{Action, SendJob, SendOutcome, SendRequest};
use crate::status::Status;

/// Long enough for every printer on the segment to answer, short enough to wait through.
const SCAN_WINDOW: Duration = Duration::from_secs(2);

/// How often a watched printer is asked what it is doing.
const WATCH_PERIOD: Duration = Duration::from_secs(3);

/// How long a list of printers in view goes before the network is asked again, in seconds.
const RESCAN_S: f64 = 30.0;

/// Whether a machine answered the last scan. It is the last answer rather than live
/// state, for the reason `docs/decisions/0157` gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// It answered the last scan.
    Answered,
    /// It was asked and said nothing.
    Silent,
    /// Nothing has been asked since the window opened.
    Unasked,
}

impl Reach {
    /// What the dot beside the Send button says on hover.
    pub fn hint(self, name: &str) -> String {
        match self {
            Self::Answered => format!("{name} answered the last scan"),
            Self::Silent => format!("{name} did not answer the last scan"),
            Self::Unasked => format!("{name} has not been asked yet"),
        }
    }
}

/// Why a printer could not be asked, as the window words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trouble {
    /// The failure with every cause under it, on one line.
    pub message: String,
    /// Whether the system found no route to it, which on macOS may be its privacy setting.
    pub unreachable: bool,
}

impl Trouble {
    fn of(error: SendError) -> Self {
        let unreachable = error.is_unreachable();
        Self {
            message: format!("{:#}", anyhow::Error::new(error)),
            unreachable,
        }
    }

    /// What the user can do about it, when there is something.
    pub fn advice(&self) -> Option<&'static str> {
        advice(self.unreachable && cfg!(target_os = "macos"), *IN_A_BUNDLE)
    }
}

/// Whether the window runs from an app bundle, which macOS asks about the local network
/// itself, rather than as a binary that takes the permission of whatever started it.
static IN_A_BUNDLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
});

/// What to tell a user whose printer has no route, when macOS may be the one `blocked`.
fn advice(blocked: bool, bundled: bool) -> Option<&'static str> {
    match (blocked, bundled) {
        (false, _) => None,
        (true, true) => Some(
            "If it is switched on, macOS may be keeping Encrust off the local network: allow \
             it under System Settings, Privacy & Security, Local Network.",
        ),
        (true, false) => Some(
            "If it is switched on, macOS may be keeping this window off the local network. \
             Started from a terminal, it has that terminal's permission: start it from \
             Terminal.app, allow your terminal or editor under System Settings, Privacy & \
             Security, Local Network, or run it as an app with `cargo xtask app`.",
        ),
    }
}

/// The last thing a watched printer said, or why it said nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    State(State),
    Failed(Trouble),
}

/// One printer being watched on a thread of its own, and what it said last.
struct Watch {
    wire: Wire,
    stop: Arc<AtomicBool>,
    reports: Receiver<Reading>,
    last: Option<Reading>,
}

impl Watch {
    fn spawn(wire: Wire) -> Self {
        let (sender, reports) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, watched) = (Arc::clone(&stop), wire.clone());
        crate::job::spawn(move || {
            let stopped = || flag.load(Ordering::Relaxed);
            printer_link::watch(&watched, WATCH_PERIOD, &stopped, &mut |heard| {
                let reading = match heard {
                    Ok(state) => Reading::State(state),
                    Err(error) => Reading::Failed(Trouble::of(error)),
                };
                // The window let go of this watch, so nobody is listening any more.
                if sender.send(reading).is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
            });
        });
        Self {
            wire,
            stop,
            reports,
            last: None,
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A file that reached a printer and has not been started yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub printer: String,
    pub filename: String,
}

/// A Prusa machine the user has set up, with what it called itself when last reached.
///
/// It is set up rather than discovered: `PrusaLink` announces itself over mDNS, which this
/// workspace has no resolver for, and it wants credentials either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prusa {
    pub link: Link,
    pub text: Option<String>,
}

/// A printer the window can send to, whichever protocol it speaks.
#[derive(Debug, Clone, Copy)]
pub enum Target<'a> {
    Sdcp(&'a Printer),
    Prusa(&'a Prusa),
}

impl Target<'_> {
    /// What the destination is remembered by. A board keeps its id across a rescan and a
    /// Prusa machine is only ever set up once per host, so both are stable.
    pub fn key(&self) -> String {
        match self {
            Self::Sdcp(printer) => format!("sdcp:{}", printer.mainboard_id),
            Self::Prusa(prusa) => format!("prusa:{}", prusa.link.host),
        }
    }

    /// What the button calls it.
    pub fn name(&self) -> String {
        match self {
            Self::Sdcp(printer) => printer.name.clone(),
            Self::Prusa(prusa) => prusa.link.host.clone(),
        }
    }

    /// The line under the name in the menu: where it is, or what it says it is.
    pub fn detail(&self) -> String {
        match self {
            Self::Sdcp(printer) => printer.address.to_string(),
            Self::Prusa(prusa) => prusa
                .text
                .clone()
                .unwrap_or_else(|| "not reached yet".to_owned()),
        }
    }

    /// Where it is: a board's IP address, or a Prusa machine's host.
    pub fn address(&self) -> String {
        match self {
            Self::Sdcp(printer) => printer.address.to_string(),
            Self::Prusa(prusa) => prusa.link.host.clone(),
        }
    }

    /// Whether this is a machine a profile stating `connection` can be sent to.
    pub fn speaks(&self, connection: Connection) -> bool {
        matches!(
            (self, connection),
            (Self::Sdcp(_), Connection::Sdcp) | (Self::Prusa(_), Connection::PrusaLink)
        )
    }

    /// The errand's other half: what a worker thread needs to reach this printer.
    pub fn wire(&self) -> Wire {
        match self {
            Self::Sdcp(printer) => Wire::Sdcp((*printer).clone()),
            Self::Prusa(prusa) => Wire::Prusa(prusa.link.clone()),
        }
    }
}

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

    /// Where a listed printer is on the network: the board that answered with its id or
    /// from its address, or the Prusa machine set up at its host.
    pub fn resolve(&self, machine: &NetworkMachine) -> Option<Target<'_>> {
        self.find(self.speaks, machine)
    }

    /// Where a printer some profile lists is on the network, for a profile speaking
    /// `speaks`, which need not be the one in hand.
    pub fn find(&self, speaks: Connection, machine: &NetworkMachine) -> Option<Target<'_>> {
        resolve(&self.printers, &self.prusa, speaks, machine)
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
            Some(file) => format!("{name} is printing {file}"),
            None => format!("{name} is busy: {}", state.state),
        })
    }

    /// Asks the network again when a list of printers is in view and the last time the
    /// window asked on its own was a while ago; `now_s` is the window's clock.
    pub fn keep_fresh(&mut self, now_s: f64) {
        let due = self.asked_at.is_none_or(|at| now_s - at >= RESCAN_S);
        if due && !self.hold && !self.is_scanning() {
            self.asked_at = Some(now_s);
            self.scan();
        }
    }

    /// The printer the top bar's ring stands for: the one the printer in hand is bound to,
    /// else the first one printing, else the first one listed.
    pub fn current(&self) -> Option<Target<'_>> {
        if let Some(bound) = self.target() {
            return Some(bound);
        }
        let targets = self.targets();
        let printing = targets.iter().find(
            |target| matches!(self.reading(target), Some(Reading::State(state)) if state.busy),
        );
        printing.or(targets.first()).copied()
    }

    /// What has been asked of the printer under `key` besides its state.
    pub fn extras(&self, key: &str) -> Option<&Extras> {
        self.extras.get(key)
    }

    /// Sends `errand` to the printer under `key`, on a thread of its own.
    pub fn ask(&mut self, key: &str, errand: Errand) {
        let Some(target) = self.targets().into_iter().find(|t| t.key() == key) else {
            return;
        };
        let state = match self.reading(&target) {
            Some(Reading::State(state)) => state.clone(),
            _ => State::default(),
        };
        let wire = target.wire();
        let extras = self.extras.entry(key.to_owned()).or_default();
        match &errand {
            Errand::Picture { id, .. } => {
                extras.pictures.insert(id.clone(), Picture::Asked);
            }
            Errand::History => extras.history_asked = true,
            Errand::Control(_) | Errand::Camera(_) => {}
        }
        let out = errands::Out::send(key.to_owned(), wire, state, errand);
        if out.holds_the_buttons() {
            extras.doing = Some(out.doing());
            extras.trouble = None;
        }
        self.out.push(out);
    }

    /// Puts each picture that has come in onto a texture the cards can draw.
    pub fn show_pictures(&mut self, ctx: &egui::Context) {
        for (key, extras) in &mut self.extras {
            for (id, picture) in &mut extras.pictures {
                if let Picture::Decoded(image) = picture {
                    let name = format!("printer-picture-{key}-{id}");
                    let texture =
                        ctx.load_texture(name, image.clone(), egui::TextureOptions::LINEAR);
                    *picture = Picture::Shown(texture);
                }
            }
        }
    }

    /// The pictures the cards would draw and nobody has asked for yet: the print under way
    /// on each printer, and each print in a history that has been read.
    fn wanted_pictures(&self) -> Vec<(String, Errand)> {
        let mut wanted = Vec::new();
        for target in self.targets() {
            let key = target.key();
            let extras = self.extras.get(&key);
            let known = |id: &str| extras.is_some_and(|extras| extras.pictures.contains_key(id));
            if let Some(Reading::State(state)) = self.reading(&target)
                && let Some(id) = state.job_id.as_deref()
                && state.file.is_some()
                && !known(id)
            {
                let id = id.to_owned();
                wanted.push((key.clone(), Errand::Picture { id, task: None }));
            }
            let tasks = extras
                .and_then(|extras| extras.history.as_deref())
                .unwrap_or_default();
            for task in tasks {
                if !task.thumbnail.is_empty() && !known(&task.task_id) {
                    let id = task.task_id.clone();
                    let task = Some(task.clone());
                    wanted.push((key.clone(), Errand::Picture { id, task }));
                }
            }
        }
        wanted
    }

    /// Whether any printer is being watched, which is what keeps its card repainting.
    pub fn is_watching(&self) -> bool {
        !self.watches.is_empty()
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
                self.listed.iter().any(|machine| {
                    resolve(
                        std::slice::from_ref(*printer),
                        &[],
                        Connection::Sdcp,
                        machine,
                    )
                    .is_some()
                })
            })
            .cloned()
            .collect()
    }

    /// Why the chosen printer cannot take what the window is set to write.
    ///
    /// Only a mismatch nothing has to be asked about is caught here. What an Elegoo board
    /// takes is what the board itself reports, which costs a connection, so that check
    /// happens on the errand's own thread.
    pub fn blocker(&self, format: SlicedFormat) -> Option<&'static str> {
        match self.target() {
            Some(Target::Prusa(_)) if !matches!(format, SlicedFormat::Sl1(_)) => {
                Some("A Prusa machine prints .sl1; set that as this printer's output.")
            }
            _ => None,
        }
    }

    /// Whether a scan is still listening for answers.
    pub fn is_scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Asks every board on the network to introduce itself, each manual address, and every
    /// Prusa machine that has been set up.
    pub fn scan(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let mut manual = self.manual.clone();
        // A listed board is asked at its address too, which reaches it where a broadcast
        // does not cross.
        if self.speaks == Connection::Sdcp {
            for address in self
                .listed
                .iter()
                .filter_map(|m| m.address.trim().parse().ok())
            {
                if !manual.contains(&address) {
                    manual.push(address);
                }
            }
        }
        let links: Vec<Link> = self.prusa.iter().map(|prusa| prusa.link.clone()).collect();
        crate::job::spawn(move || {
            let _ = sender.send(printer_link::scan(SCAN_WINDOW, &manual, &links));
        });
        self.scan = Some(receiver);
    }

    /// Remembers an address the user typed, and asks it who it is.
    pub fn add(&mut self, address: IpAddr) {
        if !self.manual.contains(&address) {
            self.manual.push(address);
        }
        self.scan();
    }

    /// Sets up a Prusa machine, replacing one already known at that host.
    pub fn add_prusa(&mut self, link: Link) {
        let prusa = Prusa { link, text: None };
        match self
            .prusa
            .iter_mut()
            .find(|known| known.link.host == prusa.link.host)
        {
            Some(known) => *known = prusa,
            None => self.prusa.push(prusa),
        }
        self.scan();
    }

    /// Forgets a Prusa machine's login.
    pub fn forget_prusa(&mut self, host: &str) {
        self.prusa.retain(|known| known.link.host != host);
        self.watch_all();
    }

    /// Sends a file the window has just written, and deletes it after when it was
    /// written only to be sent.
    pub fn send(&mut self, path: PathBuf, status: &mut Status) {
        let Some(target) = self.target() else {
            return;
        };
        let (name, wire) = (self.label(&target), target.wire());
        let temporary = is_temporary(&path);
        *status = Status::Info(format!("Sending to {name}"));
        self.sent = None;
        self.job = Some(SendJob::spawn(SendRequest {
            name,
            wire,
            action: Action::Upload { path, temporary },
        }));
    }

    /// Starts the file that was sent last, which is the one the user just watched land.
    pub fn start_print(&mut self, status: &mut Status) {
        let (Some(sent), Some(target)) = (self.sent.clone(), self.target()) else {
            return;
        };
        let (name, wire) = (self.label(&target), target.wire());
        *status = Status::Info(format!("Starting {} on {name}", sent.filename));
        self.job = Some(SendJob::spawn(SendRequest {
            name,
            wire,
            action: Action::StartPrint {
                filename: sent.filename,
            },
        }));
    }

    /// Drains the scan and the errand into the stage notice. Returns whether either is
    /// still running, which is what tells the window to keep repainting.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        for watch in self.watches.values_mut() {
            while let Ok(reading) = watch.reports.try_recv() {
                watch.last = Some(reading);
            }
        }
        let extras = &mut self.extras;
        self.out
            .retain(|out| !out.take(extras.entry(out.key.clone()).or_default()));
        if !self.hold {
            for (key, errand) in self.wanted_pictures() {
                self.ask(&key, errand);
            }
        }
        let errands = !self.out.is_empty();
        self.take_scan() | self.take_job(status) | errands
    }

    /// What has been asked of a printer, open for a test to fill in as if it had answered.
    #[cfg(test)]
    pub(crate) fn extras_mut(&mut self, key: &str) -> &mut Extras {
        self.extras.entry(key.to_owned()).or_default()
    }

    /// What a watch hears, put straight into the window as if its thread had said it.
    #[cfg(test)]
    pub(crate) fn hear(&mut self, key: &str, reading: Reading) {
        let (_, reports) = mpsc::channel();
        let wire = self
            .targets()
            .into_iter()
            .find(|target| target.key() == key)
            .expect("the printer heard is listed")
            .wire();
        let watch = Watch {
            wire,
            stop: Arc::new(AtomicBool::new(true)),
            reports,
            last: Some(reading),
        };
        self.watches.insert(key.to_owned(), watch);
    }

    /// Watches every printer listed, once each, and lets go of the ones gone or moved.
    fn watch_all(&mut self) {
        let wanted: BTreeMap<String, Wire> = self
            .targets()
            .iter()
            .map(|target| (target.key(), target.wire()))
            .collect();
        self.watches
            .retain(|key, watch| wanted.get(key) == Some(&watch.wire));
        self.extras.retain(|key, _| wanted.contains_key(key));
        for (key, wire) in wanted {
            self.watches
                .entry(key)
                .or_insert_with(|| Watch::spawn(wire));
        }
    }

    fn take_scan(&mut self) -> bool {
        let Some(receiver) = self.scan.as_ref() else {
            return false;
        };
        match receiver.try_recv() {
            Ok(found) => {
                self.seen = found
                    .boards
                    .iter()
                    .map(|printer| Target::Sdcp(printer).key())
                    .chain(found.reached.keys().map(|host| format!("prusa:{host}")))
                    .collect();
                self.asked = true;
                self.printers = merged(std::mem::take(&mut self.printers), found.boards);
                for prusa in &mut self.prusa {
                    prusa.text = found.reached.get(&prusa.link.host).cloned();
                }
                self.trouble = found.broadcast.map(Trouble::of);
                self.silent = found
                    .silent
                    .into_iter()
                    .map(|(address, error)| (address, error.map(Trouble::of)))
                    .collect();
                self.scan = None;
                if !self.hold {
                    self.watch_all();
                }
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => {
                self.scan = None;
                false
            }
        }
    }

    fn take_job(&mut self, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        self.job = None;
        match outcome {
            SendOutcome::Sent { printer, filename } => {
                *status = Status::Info(format!("{filename} is on {printer}"));
                self.sent = Some(Sent { printer, filename });
            }
            SendOutcome::Printing { printer, filename } => {
                *status = Status::Info(format!("{printer} is printing {filename}"));
                self.sent = None;
            }
            SendOutcome::Cancelled => *status = Status::Info("Transfer cancelled".to_owned()),
            SendOutcome::Failed(message) => {
                *status = Status::failed(&anyhow::Error::msg(message));
            }
        }
        false
    }
}

/// Where `machine` is among the boards and Prusa machines the window knows, for a
/// profile speaking `speaks`.
fn resolve<'a>(
    printers: &'a [Printer],
    prusa: &'a [Prusa],
    speaks: Connection,
    machine: &NetworkMachine,
) -> Option<Target<'a>> {
    let address = machine.address.trim();
    match speaks {
        Connection::Sdcp => printers
            .iter()
            .find(|printer| {
                let same_board = machine.board.as_deref() == Some(printer.mainboard_id.as_str());
                same_board || printer.address.to_string() == address
            })
            .map(Target::Sdcp),
        Connection::PrusaLink => prusa
            .iter()
            .find(|prusa| prusa.link.host == address)
            .map(Target::Prusa),
        Connection::None => None,
    }
}

/// The fields a Prusa machine is described by, as they are being typed.
///
/// A machine's own screen offers a username and a password, or a single key when its
/// digest login has been turned off; the draft carries whichever is filled in.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PrusaDraft {
    pub host: String,
    pub user: String,
    pub password: String,
    pub key: String,
}

impl PrusaDraft {
    /// The machine this describes, or `None` while it is not described enough to reach.
    pub fn link(&self) -> Option<Link> {
        let host = self.host.trim();
        if host.is_empty() {
            return None;
        }
        let key = self.key.trim();
        if !key.is_empty() {
            return Some(Link::api_key(host, key));
        }
        let user = match self.user.trim() {
            "" => net_prusalink::DEFAULT_USER,
            typed => typed,
        };
        match self.password.is_empty() {
            true => None,
            false => Some(Link::digest(host, user, &self.password)),
        }
    }
}

/// What a scan found, over the boards already listed: one that answered is replaced at
/// its new address, and one that did not stays listed so a bound profile keeps its
/// machine. Found boards nobody knew of go on the end.
fn merged(known: Vec<Printer>, found: Vec<Printer>) -> Vec<Printer> {
    let mut found: Vec<Option<Printer>> = found.into_iter().map(Some).collect();
    let mut printers: Vec<Printer> = known
        .into_iter()
        .map(|printer| {
            let answered = found
                .iter_mut()
                .find(|other| {
                    other
                        .as_ref()
                        .is_some_and(|other| other.mainboard_id == printer.mainboard_id)
                })
                .and_then(Option::take);
            answered.unwrap_or(printer)
        })
        .collect();
    printers.extend(found.into_iter().flatten());
    printers
}

/// Where a stack goes when it is written only to be sent.
pub fn temporary_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// A file the window wrote for itself is deleted once it has been sent; one the user
/// named is theirs and stays.
fn is_temporary(path: &Path) -> bool {
    path.starts_with(std::env::temp_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_pipeline::Sl1Flavour;

    fn a_printer(id: &str) -> Printer {
        Printer {
            name: format!("Saturn {id}"),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([192, 168, 1, 42]),
            mainboard_id: id.into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport: net_sdcp::Transport::WebSocket,
        }
    }

    fn at(last: u8, printer: Printer) -> Printer {
        Printer {
            address: IpAddr::from([192, 168, 1, last]),
            ..printer
        }
    }

    /// Stands `network` under a profile `saturn` speaking `speaks` and listing `machines`
    /// as `(address, board)`.
    fn listing(network: &mut Network, speaks: Connection, machines: &[(&str, Option<&str>)]) {
        let profile = PrinterProfile {
            connection: speaks,
            machines: machines
                .iter()
                .map(|(address, board)| NetworkMachine {
                    name: String::new(),
                    address: (*address).to_owned(),
                    board: board.map(str::to_owned),
                })
                .collect(),
            ..PrinterProfile::default()
        };
        network.bind_to(Some("saturn".to_owned()), Some(&profile));
    }

    fn a_prusa(host: &str) -> Prusa {
        Prusa {
            link: Link::digest(host, net_prusalink::DEFAULT_USER, "secret"),
            text: None,
        }
    }

    #[test]
    fn a_listed_printer_is_found_by_its_board_id_wherever_it_answers() {
        let mut network = Network {
            printers: vec![a_printer("aa"), at(77, a_printer("bb"))],
            ..Network::default()
        };
        listing(
            &mut network,
            Connection::Sdcp,
            &[("192.168.1.9", Some("bb"))],
        );
        assert_eq!(
            network.target().map(|target| target.name()),
            Some("Saturn bb".to_owned()),
            "the board moved to .77 and still answers by its id"
        );
        listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn aa".into()));
        listing(&mut network, Connection::Sdcp, &[]);
        assert!(
            network.target().is_none(),
            "a profile listing nothing writes a file"
        );
    }

    #[test]
    fn the_two_protocols_cannot_collide_on_a_key() {
        let network = Network {
            printers: vec![a_printer("sl1.local")],
            prusa: vec![a_prusa("sl1.local")],
            ..Network::default()
        };
        let keys: Vec<String> = network
            .targets()
            .iter()
            .map(|target| target.key())
            .collect();
        assert_eq!(keys, vec!["sdcp:sl1.local", "prusa:sl1.local"]);
    }

    /// A board that is switched off between two scans is still the board this profile
    /// prints on, so it stays listed rather than sending the user back to a file.
    #[test]
    fn a_board_that_did_not_answer_a_scan_stays_the_destination() {
        let (sender, receiver) = mpsc::channel();
        let mut network = Network {
            printers: vec![a_printer("aa")],
            scan: Some(receiver),
            hold: true,
            ..Network::default()
        };
        listing(
            &mut network,
            Connection::Sdcp,
            &[("192.168.1.42", Some("aa"))],
        );
        sender
            .send(Found::default())
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));
        assert_eq!(network.target().map(|t| t.key()), Some("sdcp:aa".into()));
    }

    #[test]
    fn a_board_that_answered_from_a_new_address_is_listed_once_at_that_address() {
        let moved = Printer {
            address: IpAddr::from([192, 168, 1, 77]),
            ..a_printer("aa")
        };
        let printers = merged(vec![a_printer("aa")], vec![moved, a_printer("bb")]);
        assert_eq!(printers.len(), 2);
        assert_eq!(printers[0].address, IpAddr::from([192, 168, 1, 77]));
        assert_eq!(printers[1].mainboard_id, "bb");
    }

    /// Each profile lists its own printers and keeps the one it last sent to, so switching
    /// profiles puts each back on its own machine.
    #[test]
    fn each_printer_profile_keeps_the_printer_it_last_sent_to() {
        let mut network = Network {
            printers: vec![at(10, a_printer("aa")), at(11, a_printer("bb"))],
            ..Network::default()
        };
        let both = [("192.168.1.10", None), ("192.168.1.11", None)];
        listing(&mut network, Connection::Sdcp, &both);
        assert_eq!(
            network.target().map(|t| t.name()),
            Some("Saturn aa".into()),
            "nothing chosen yet sends to the first listed"
        );
        network.choose("192.168.1.11".to_owned());
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn bb".into()));

        let profile = PrinterProfile::default();
        network.bind_to(Some("mars".to_owned()), Some(&profile));
        assert!(network.target().is_none(), "mars lists no printer");
        listing(&mut network, Connection::Sdcp, &both);
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn bb".into()));
    }

    /// The dot beside Send says what the last scan found, and tells a board that was
    /// asked and said nothing from one nobody has asked yet.
    #[test]
    fn a_board_is_reachable_only_while_it_keeps_answering() {
        let (sender, receiver) = mpsc::channel();
        let mut network = Network {
            printers: vec![a_printer("aa"), a_printer("bb")],
            scan: Some(receiver),
            hold: true,
            ..Network::default()
        };
        let answered = Target::Sdcp(&network.printers[0].clone());
        assert_eq!(network.reach(&answered), Reach::Unasked);

        sender
            .send(Found {
                boards: vec![a_printer("aa")],
                ..Found::default()
            })
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));

        let printers = network.printers.clone();
        let (first, second) = (Target::Sdcp(&printers[0]), Target::Sdcp(&printers[1]));
        assert_eq!(network.reach(&first), Reach::Answered);
        assert_eq!(
            network.reach(&second),
            Reach::Silent,
            "a board still listed because a profile is bound to it did not answer"
        );
    }

    #[test]
    fn a_prusa_machine_that_was_set_up_stays_a_destination_through_a_scan() {
        let (sender, receiver) = mpsc::channel();
        let mut network = Network {
            prusa: vec![a_prusa("sl1.local")],
            scan: Some(receiver),
            hold: true,
            ..Network::default()
        };
        listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
        sender
            .send(Found {
                reached: BTreeMap::from([("sl1.local".to_owned(), "Prusa SLA 1.8.0".to_owned())]),
                ..Found::default()
            })
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));
        assert_eq!(
            network.target().map(|target| target.detail()),
            Some("Prusa SLA 1.8.0".to_owned())
        );
    }

    #[test]
    fn a_prusa_machine_is_only_sent_the_container_it_prints() {
        let mut network = Network {
            prusa: vec![a_prusa("sl1.local")],
            ..Network::default()
        };
        listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
        assert!(network.blocker(SlicedFormat::Goo).is_some());
        assert!(
            network
                .blocker(SlicedFormat::Sl1(Sl1Flavour::Sl1s))
                .is_none()
        );
    }

    #[test]
    fn a_board_is_not_told_what_it_prints_before_it_is_asked() {
        let mut network = Network {
            printers: vec![a_printer("aa")],
            ..Network::default()
        };
        listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
        assert!(network.blocker(SlicedFormat::Goo).is_none());
    }

    #[test]
    fn setting_up_the_same_host_twice_replaces_it() {
        let mut network = Network::default();
        network.add_prusa(Link::api_key("sl1.local", "first"));
        network.add_prusa(Link::api_key("sl1.local", "second"));
        assert_eq!(network.prusa.len(), 1);
        assert_eq!(
            network.prusa[0].link.auth,
            net_prusalink::Auth::ApiKey("second".to_owned())
        );
    }

    #[test]
    fn forgetting_a_login_leaves_the_listed_machine_unreachable() {
        let mut network = Network {
            prusa: vec![a_prusa("sl1.local")],
            ..Network::default()
        };
        listing(&mut network, Connection::PrusaLink, &[("sl1.local", None)]);
        network.forget_prusa("sl1.local");
        assert!(network.prusa.is_empty());
        assert!(network.target().is_none());
        assert_eq!(network.listed().len(), 1, "the profile still lists it");
    }

    #[test]
    fn a_draft_needs_a_host_and_one_of_the_two_logins() {
        let mut draft = PrusaDraft::default();
        assert!(draft.link().is_none(), "a machine needs somewhere to be");
        draft.host = "sl1.local".to_owned();
        assert!(draft.link().is_none(), "and a way in");
        draft.password = "secret".to_owned();
        assert_eq!(
            draft.link(),
            Some(Link::digest(
                "sl1.local",
                net_prusalink::DEFAULT_USER,
                "secret"
            )),
            "an empty username is the one the firmware generates"
        );
        draft.key = "0123456789abcdef".to_owned();
        assert_eq!(
            draft.link(),
            Some(Link::api_key("sl1.local", "0123456789abcdef")),
            "a key answers on its own, so it wins over a half-typed login"
        );
    }

    #[test]
    fn only_a_file_the_window_wrote_for_itself_is_temporary() {
        assert!(is_temporary(&temporary_path("model.goo")));
        assert!(!is_temporary(Path::new("/Users/someone/model.goo")));
    }

    #[test]
    fn sending_without_a_destination_starts_nothing() {
        let mut network = Network::default();
        network.send(temporary_path("model.goo"), &mut Status::default());
        assert!(network.job.is_none());
    }

    fn printing(file: &str) -> State {
        State {
            state: "printing".to_owned(),
            busy: true,
            file: Some(file.to_owned()),
            progress: Some(0.5),
            ..State::default()
        }
    }

    #[test]
    fn a_bound_printer_in_the_middle_of_a_print_holds_the_send_back() {
        let mut network = Network {
            printers: vec![a_printer("aa")],
            ..Network::default()
        };
        listing(&mut network, Connection::Sdcp, &[("192.168.1.42", None)]);
        assert_eq!(
            network.busy(),
            None,
            "nothing heard yet is nothing to hold back"
        );
        network.hear("sdcp:aa", Reading::State(printing("dragon.goo")));
        assert_eq!(
            network.busy().as_deref(),
            Some("Saturn aa is printing dragon.goo")
        );
        let idle = State {
            state: "idle".to_owned(),
            ..State::default()
        };
        network.hear("sdcp:aa", Reading::State(idle));
        assert_eq!(network.busy(), None);
    }

    /// The dot follows the watch once it has heard anything, rather than a scan that may
    /// be half a minute old.
    #[test]
    fn what_the_watch_heard_overrides_the_last_scan() {
        let mut network = Network {
            printers: vec![a_printer("aa")],
            ..Network::default()
        };
        let trouble = Trouble {
            message: "network call failed".to_owned(),
            unreachable: false,
        };
        network.hear("sdcp:aa", Reading::Failed(trouble));
        let printers = network.printers.clone();
        assert_eq!(network.reach(&Target::Sdcp(&printers[0])), Reach::Silent);
        network.hear("sdcp:aa", Reading::State(printing("cube.goo")));
        assert_eq!(network.reach(&Target::Sdcp(&printers[0])), Reach::Answered);
    }

    #[test]
    fn a_broadcast_that_went_nowhere_is_kept_with_why() {
        let (sender, receiver) = mpsc::channel();
        let mut network = Network {
            scan: Some(receiver),
            manual: vec![IpAddr::from([192, 168, 1, 42])],
            hold: true,
            ..Network::default()
        };
        let no_route = std::io::Error::from(std::io::ErrorKind::HostUnreachable);
        sender
            .send(Found {
                broadcast: Some(net_sdcp::SdcpError::Io(no_route).into()),
                silent: vec![(IpAddr::from([192, 168, 1, 42]), None)],
                ..Found::default()
            })
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));
        let trouble = network.trouble.clone().expect("the broadcast failed");
        assert!(trouble.unreachable);
        assert!(
            trouble.message.contains("network call failed"),
            "{}",
            trouble.message
        );
        assert_eq!(
            network.silent,
            vec![(IpAddr::from([192, 168, 1, 42]), None)]
        );
    }

    #[test]
    fn only_no_route_on_macos_is_blamed_on_the_privacy_setting() {
        let trouble = Trouble {
            message: String::new(),
            unreachable: true,
        };
        assert_eq!(trouble.advice().is_some(), cfg!(target_os = "macos"));
        assert_eq!(advice(false, true), None);
        assert_eq!(advice(false, false), None);
    }

    #[test]
    fn a_window_started_from_a_terminal_is_told_whose_permission_it_has() {
        let bare = advice(true, false).expect("a blocked window is told what to do");
        assert!(bare.contains("terminal"), "{bare}");
        let bundled = advice(true, true).expect("a blocked app is told what to do");
        assert!(
            !bundled.contains("terminal"),
            "an app is asked about itself: {bundled}"
        );
    }

    #[test]
    fn a_held_window_does_not_ask_the_network_on_its_own() {
        let mut network = Network {
            hold: true,
            ..Network::default()
        };
        network.keep_fresh(0.0);
        assert!(!network.is_scanning());
    }

    #[test]
    fn the_list_in_view_is_asked_again_only_once_it_is_stale() {
        let mut network = Network::default();
        network.keep_fresh(100.0);
        assert!(network.is_scanning(), "the first look goes out at once");
        network.scan = None;
        network.keep_fresh(100.0 + RESCAN_S / 2.0);
        assert!(!network.is_scanning(), "a fresh list is left alone");
        network.keep_fresh(100.0 + RESCAN_S);
        assert!(network.is_scanning());
    }

    #[test]
    fn a_printer_is_called_by_the_name_a_profile_gave_it_everywhere() {
        let mut network = Network {
            printers: vec![a_printer("aa"), at(50, a_printer("bb"))],
            ..Network::default()
        };
        let mars = PrinterProfile {
            machines: vec![
                NetworkMachine {
                    name: "Bench A".to_owned(),
                    address: "192.168.1.9".to_owned(),
                    board: Some("aa".to_owned()),
                },
                NetworkMachine {
                    name: "Bench B".to_owned(),
                    address: "192.168.1.50".to_owned(),
                    board: None,
                },
            ],
            ..PrinterProfile::default()
        };
        network.learn_names([&mars]);
        let printers = network.printers.clone();
        assert_eq!(
            network.label(&Target::Sdcp(&printers[0])),
            "Bench A",
            "by its board"
        );
        assert_eq!(
            network.label(&Target::Sdcp(&printers[1])),
            "Bench B",
            "by its address"
        );
        network.learn_names([]);
        assert_eq!(network.label(&Target::Sdcp(&printers[0])), "Saturn aa");
    }

    #[test]
    fn the_print_under_way_and_each_remembered_one_want_a_picture_once() {
        let mut network = Network {
            printers: vec![a_printer("aa")],
            ..Network::default()
        };
        let state = State {
            job_id: Some("0e8a".to_owned()),
            ..printing("dragon.goo")
        };
        network.hear("sdcp:aa", Reading::State(state));
        let wanted = network.wanted_pictures();
        assert!(
            matches!(&wanted[..], [(key, Errand::Picture { id, task: None })]
                if key == "sdcp:aa" && id == "0e8a"),
            "{wanted:?}"
        );
        let extras = network.extras_mut("sdcp:aa");
        extras.pictures.insert("0e8a".to_owned(), Picture::Missing);
        extras.history = Some(vec![printer_link::Task {
            task_id: "3add".to_owned(),
            thumbnail: "http://192.168.1.42:3030/media/3add.bmp".to_owned(),
            ..printer_link::Task::default()
        }]);
        let wanted = network.wanted_pictures();
        assert!(
            matches!(&wanted[..], [(_, Errand::Picture { id, task: Some(_) })] if id == "3add"),
            "only the remembered print is still missing: {wanted:?}"
        );
    }

    #[test]
    fn the_ring_stands_for_the_bound_printer_else_the_one_printing_else_the_first() {
        let mut network = Network {
            printers: vec![a_printer("aa"), a_printer("bb")],
            ..Network::default()
        };
        assert_eq!(network.current().map(|t| t.key()), Some("sdcp:aa".into()));
        network.hear("sdcp:bb", Reading::State(printing("dragon.goo")));
        assert_eq!(network.current().map(|t| t.key()), Some("sdcp:bb".into()));
        listing(
            &mut network,
            Connection::Sdcp,
            &[("192.168.1.42", Some("aa"))],
        );
        assert_eq!(
            network.current().map(|t| t.key()),
            Some("sdcp:aa".into()),
            "the printer in hand's own machine wins over a busy one"
        );
        assert!(Network::default().current().is_none());
    }
}
