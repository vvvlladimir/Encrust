//! The printers this machine can reach, and what the window is sending them.

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
use printer_profiles::Connection;

use printer_link::{Found, SendError, State, Wire};

use crate::job::{Action, SendJob, SendOutcome, SendRequest};
use crate::status::Status;

/// Long enough for every printer on the segment to answer, short enough to wait through.
const SCAN_WINDOW: Duration = Duration::from_secs(2);

/// How often a watched printer is asked what it is doing.
const WATCH_PERIOD: Duration = Duration::from_secs(3);

/// How long a list of printers in view goes before the network is asked again, in seconds.
const RESCAN_S: f64 = 30.0;

/// Where the Slice button puts what it writes.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Destination {
    #[default]
    File,
    /// The key of the chosen printer, which is what survives a rescan.
    Printer(String),
}

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
        (self.unreachable && cfg!(target_os = "macos")).then_some(
            "If it is switched on, macOS may be keeping Encrust off the local network: allow \
             it under System Settings, Privacy & Security, Local Network.",
        )
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
    destination: Destination,
    /// Where each printer profile's files go, by catalogue id. A machine chosen once is
    /// chosen for good; see `docs/decisions/0156`.
    bound: BTreeMap<String, Destination>,
    /// The catalogue id of the printer in hand, which is what a choice is bound under.
    profile: Option<String>,
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
    /// Whether the window keeps off the network unless told to: the panel pictures must not
    /// depend on what is on the desk's network.
    pub hold: bool,
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

    /// The printer the Slice button is pointed at, or `None` when it writes a file.
    pub fn target(&self) -> Option<Target<'_>> {
        let Destination::Printer(key) = &self.destination else {
            return None;
        };
        self.targets()
            .into_iter()
            .find(|target| target.key() == *key)
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
        state.busy.then(|| match &state.file {
            Some(file) => format!("{} is printing {file}", target.name()),
            None => format!("{} is busy: {}", target.name(), state.state),
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

    /// Whether any printer is being watched, which is what keeps its card repainting.
    pub fn is_watching(&self) -> bool {
        !self.watches.is_empty()
    }

    /// Binds the printer profile in hand to the machine under `key`.
    pub fn bind_here(&mut self, key: String) {
        if let Some(profile) = self.profile.clone() {
            self.bind(&profile, Destination::Printer(key));
        }
    }

    /// Points one printer profile at a destination, whether or not it is the one in hand.
    pub fn bind(&mut self, profile: &str, destination: Destination) {
        match destination.clone() {
            Destination::File => self.bound.remove(profile),
            chosen => self.bound.insert(profile.to_owned(), chosen),
        };
        if self.profile.as_deref() == Some(profile) {
            self.destination = destination;
        }
    }

    /// Where one printer profile sends what it slices.
    pub fn bound_to(&self, profile: &str) -> Destination {
        self.bound.get(profile).cloned().unwrap_or_default()
    }

    /// Stands the window under the printer profile now in hand, pointing it back at the
    /// machine that profile was last sent to.
    pub fn bind_to(&mut self, profile: Option<String>) {
        self.destination = profile
            .as_deref()
            .and_then(|id| self.bound.get(id))
            .cloned()
            .unwrap_or_default();
        self.profile = profile;
    }

    /// What each printer profile is bound to, for the preferences file.
    pub fn bindings(&self) -> &BTreeMap<String, Destination> {
        &self.bound
    }

    /// Puts back what the preferences file remembered, boards and bindings together.
    pub fn restore(&mut self, boards: Vec<Printer>, bound: BTreeMap<String, Destination>) {
        self.printers = boards;
        self.bound = bound;
    }

    /// The boards worth writing to the preferences file: the ones a printer profile is
    /// bound to. A board that only answered a scan announces itself again next run.
    pub fn bound_boards(&self) -> Vec<Printer> {
        self.printers
            .iter()
            .filter(|printer| {
                let key = Target::Sdcp(printer).key();
                self.bound.values().any(|bound| match bound {
                    Destination::Printer(chosen) => *chosen == key,
                    Destination::File => false,
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
        let manual = self.manual.clone();
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

    /// Forgets a Prusa machine, and stops sending to it if that is where it was going.
    pub fn forget_prusa(&mut self, host: &str) {
        self.prusa.retain(|known| known.link.host != host);
        let gone = format!("prusa:{host}");
        self.bound
            .retain(|_, bound| *bound != Destination::Printer(gone.clone()));
        if self.target().is_none() {
            self.destination = Destination::File;
        }
        self.watch_all();
    }

    /// Sends a file the window has just written, and deletes it after when it was
    /// written only to be sent.
    pub fn send(&mut self, path: PathBuf, status: &mut Status) {
        let Some(target) = self.target() else {
            return;
        };
        let (name, wire) = (target.name(), target.wire());
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
        let (name, wire) = (target.name(), target.wire());
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
        self.take_scan() | self.take_job(status)
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
                // A printer that answered no longer is not a destination any more.
                if self.target().is_none() {
                    self.destination = Destination::File;
                }
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

    fn a_prusa(host: &str) -> Prusa {
        Prusa {
            link: Link::digest(host, net_prusalink::DEFAULT_USER, "secret"),
            text: None,
        }
    }

    #[test]
    fn the_destination_names_a_printer_by_its_board() {
        let mut network = Network {
            printers: vec![a_printer("aa"), a_printer("bb")],
            destination: Destination::Printer("sdcp:bb".to_owned()),
            ..Network::default()
        };
        assert_eq!(
            network.target().map(|target| target.name()),
            Some("Saturn bb".to_owned())
        );
        network.destination = Destination::File;
        assert!(network.target().is_none());
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
            destination: Destination::Printer("sdcp:aa".to_owned()),
            scan: Some(receiver),
            hold: true,
            ..Network::default()
        };
        sender
            .send(Found::default())
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));
        assert_eq!(network.destination, Destination::Printer("sdcp:aa".into()));
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

    /// Binding a profile to a board is what makes the window send there without being
    /// asked again, and switching profiles puts each back on its own machine.
    #[test]
    fn each_printer_profile_keeps_the_machine_it_was_bound_to() {
        let mut network = Network {
            printers: vec![a_printer("aa"), a_printer("bb")],
            ..Network::default()
        };
        network.bind("saturn", Destination::Printer("sdcp:aa".into()));
        network.bind("mars", Destination::Printer("sdcp:bb".into()));

        network.bind_to(Some("mars".to_owned()));
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn bb".into()));
        network.bind_to(Some("saturn".to_owned()));
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn aa".into()));
        network.bind_to(Some("phrozen".to_owned()));
        assert!(
            network.target().is_none(),
            "a profile nobody bound writes a file"
        );
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
            destination: Destination::Printer("prusa:sl1.local".to_owned()),
            scan: Some(receiver),
            hold: true,
            ..Network::default()
        };
        sender
            .send(Found {
                reached: BTreeMap::from([("sl1.local".to_owned(), "Prusa SLA 1.8.0".to_owned())]),
                ..Found::default()
            })
            .expect("the scan holds the receiver");
        assert!(!network.poll(&mut Status::default()));
        assert_eq!(
            network.destination,
            Destination::Printer("prusa:sl1.local".to_owned())
        );
        assert_eq!(
            network.target().map(|target| target.detail()),
            Some("Prusa SLA 1.8.0".to_owned())
        );
    }

    #[test]
    fn a_prusa_machine_is_only_sent_the_container_it_prints() {
        let network = Network {
            prusa: vec![a_prusa("sl1.local")],
            destination: Destination::Printer("prusa:sl1.local".to_owned()),
            ..Network::default()
        };
        assert!(network.blocker(SlicedFormat::Goo).is_some());
        assert!(
            network
                .blocker(SlicedFormat::Sl1(Sl1Flavour::Sl1s))
                .is_none()
        );
    }

    #[test]
    fn a_board_is_not_told_what_it_prints_before_it_is_asked() {
        let network = Network {
            printers: vec![a_printer("aa")],
            destination: Destination::Printer("sdcp:aa".to_owned()),
            ..Network::default()
        };
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
    fn forgetting_a_machine_sends_the_file_back_to_disk() {
        let mut network = Network {
            prusa: vec![a_prusa("sl1.local")],
            destination: Destination::Printer("prusa:sl1.local".to_owned()),
            ..Network::default()
        };
        network.forget_prusa("sl1.local");
        assert!(network.prusa.is_empty());
        assert_eq!(network.destination, Destination::File);
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
            destination: Destination::Printer("sdcp:aa".to_owned()),
            ..Network::default()
        };
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
        let mut trouble = Trouble {
            message: String::new(),
            unreachable: true,
        };
        assert_eq!(trouble.advice().is_some(), cfg!(target_os = "macos"));
        trouble.unreachable = false;
        assert_eq!(trouble.advice(), None);
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
    fn binding_from_the_list_binds_the_printer_profile_in_hand() {
        let mut network = Network {
            printers: vec![a_printer("aa")],
            ..Network::default()
        };
        network.bind_here("sdcp:aa".to_owned());
        assert!(
            network.target().is_none(),
            "no profile in hand, nothing to bind"
        );
        network.bind_to(Some("saturn".to_owned()));
        network.bind_here("sdcp:aa".to_owned());
        assert_eq!(
            network.bound_to("saturn"),
            Destination::Printer("sdcp:aa".into())
        );
        assert_eq!(network.target().map(|t| t.name()), Some("Saturn aa".into()));
    }
}
