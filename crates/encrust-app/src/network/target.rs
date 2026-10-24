use crate::i18n::tr;
use net_prusalink::Link;
use net_sdcp::Printer;
use printer_link::Wire;
use printer_profiles::{Connection, NetworkMachine};

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
                .unwrap_or_else(|| tr!("network-not-reached")),
        }
    }

    /// Where it is: a board's IP address, or a Prusa machine's host.
    pub fn address(&self) -> String {
        match self {
            Self::Sdcp(printer) => printer.address.to_string(),
            Self::Prusa(prusa) => prusa.link.host.clone(),
        }
    }

    /// What this machine takes a file over.
    pub fn protocol(&self) -> Connection {
        match self {
            Self::Sdcp(_) => Connection::Sdcp,
            Self::Prusa(_) => Connection::PrusaLink,
        }
    }

    /// Whether this is a machine a profile stating `connection` can be sent to.
    pub fn speaks(&self, connection: Connection) -> bool {
        self.protocol() == connection
    }

    /// The errand's other half: what a worker thread needs to reach this printer.
    pub fn wire(&self) -> Wire {
        match self {
            Self::Sdcp(printer) => Wire::Sdcp((*printer).clone()),
            Self::Prusa(prusa) => Wire::Prusa(prusa.link.clone()),
        }
    }
}

/// Where `machine` is among the boards and Prusa machines the window knows, whatever the
/// profile listing it is set to speak: whether it can be sent to is `Target::speaks`. A
/// board whose id is known is found by that id alone, never by another board's address.
pub(super) fn resolve<'a>(
    printers: &'a [Printer],
    prusa: &'a [Prusa],
    machine: &NetworkMachine,
) -> Option<Target<'a>> {
    let address = machine.address.trim();
    let board = printers
        .iter()
        .find(|printer| match machine.board.as_deref() {
            Some(board) => printer.mainboard_id == board,
            None => printer.address.to_string() == address,
        });
    board.map(Target::Sdcp).or_else(|| {
        let set_up = prusa.iter().find(|prusa| prusa.link.host == address);
        set_up.map(Target::Prusa)
    })
}
