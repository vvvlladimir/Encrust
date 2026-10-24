use super::reach::Trouble;
use super::target::{Prusa, Target};
use super::{Network, SCAN_WINDOW};
use net_prusalink::Link;
use net_sdcp::Printer;
use printer_profiles::Connection;
use std::net::IpAddr;
use std::sync::mpsc::{self, TryRecvError};

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
/// machine, unless another board answered from its address. New boards go on the end.
pub(super) fn merged(known: Vec<Printer>, found: Vec<Printer>) -> Vec<Printer> {
    let taken: Vec<IpAddr> = found.iter().map(|printer| printer.address).collect();
    let mut found: Vec<Option<Printer>> = found.into_iter().map(Some).collect();
    let mut printers: Vec<Printer> = known
        .into_iter()
        .filter_map(|printer| {
            let answered = found
                .iter_mut()
                .find(|other| {
                    other
                        .as_ref()
                        .is_some_and(|other| other.mainboard_id == printer.mainboard_id)
                })
                .and_then(Option::take);
            match answered {
                Some(answered) => Some(answered),
                None if taken.contains(&printer.address) => None,
                None => Some(printer),
            }
        })
        .collect();
    printers.extend(found.into_iter().flatten());
    printers
}

impl Network {
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
        // does not cross; one known to have moved is asked where it went.
        let boards = self.listed.iter().filter_map(|m| match self.resolve(m) {
            Some(Target::Sdcp(board)) => Some(board.address),
            Some(Target::Prusa(_)) => None,
            None if self.speaks == Connection::Sdcp || m.board.is_some() => {
                m.address.trim().parse().ok()
            }
            None => None,
        });
        for address in boards.collect::<Vec<IpAddr>>() {
            if !manual.contains(&address) {
                manual.push(address);
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

    /// Stops asking at an address typed by hand, once the printer there is taken off a list.
    pub fn forget(&mut self, address: &str) {
        let Ok(address) = address.trim().parse::<IpAddr>() else {
            return;
        };
        self.manual.retain(|typed| *typed != address);
        self.silent.retain(|(typed, _)| *typed != address);
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
        if !self.hold {
            self.watch_all();
        }
    }

    pub(super) fn take_scan(&mut self) -> bool {
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
}
