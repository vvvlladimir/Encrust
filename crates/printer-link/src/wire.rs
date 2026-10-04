use std::path::Path;
use std::time::Duration;

use net_prusalink::Link;
use net_sdcp::{Control, Printer, SdcpError, Transfer, Transport};

use crate::error::SendError;
use crate::state::State;

/// A printer on a local network answers at once or not at all.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How one printer is reached: the two protocols share nothing but this errand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wire {
    Sdcp(Printer),
    Prusa(Link),
}

impl Wire {
    /// What the printer is called in front of the user.
    pub fn name(&self) -> &str {
        match self {
            Self::Sdcp(printer) => &printer.name,
            Self::Prusa(link) => &link.host,
        }
    }

    /// Whether a transfer counts bytes as it goes. A `PrusaLink` upload is one PUT with no
    /// progress inside it; see ADR 0152.
    pub fn counts_bytes(&self) -> bool {
        matches!(self, Self::Sdcp(_))
    }
}

/// Sends a written file and returns the name it landed under on the printer.
///
/// `progress` hears each packet a board takes; `cancel` is asked between packets, and only
/// before the one PUT a Prusa machine takes.
pub fn upload(
    wire: &Wire,
    path: &Path,
    progress: &mut dyn FnMut(Transfer),
    cancel: &dyn Fn() -> bool,
) -> Result<String, SendError> {
    match wire {
        Wire::Sdcp(printer) => Ok(upload_sdcp(printer, path, progress, cancel)?),
        Wire::Prusa(link) => Ok(net_prusalink::upload(link, path, cancel)?),
    }
}

/// The board is asked what it takes before the file is sent, so a mismatch costs a
/// connection rather than a whole transfer.
///
/// Only a version 3 board is asked. The generation before it answers the question with its
/// status rather than its attributes, and opening a connection to it means running a broker
/// and waiting for it to dial back — a price to pay twice for an answer that is empty.
fn upload_sdcp(
    printer: &Printer,
    path: &Path,
    progress: &mut dyn FnMut(Transfer),
    cancel: &dyn Fn() -> bool,
) -> Result<String, SdcpError> {
    if printer.transport == Transport::WebSocket {
        refuse_wrong_type(printer, path)?;
    }
    net_sdcp::upload(printer, path, &mut |transfer| {
        progress(transfer);
        !cancel()
    })
}

/// Fails when the board says it does not print files of this extension.
fn refuse_wrong_type(printer: &Printer, path: &Path) -> Result<(), SdcpError> {
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
    let attributes = control.attributes()?;
    if attributes.file_types.is_empty() || attributes.accepts(&extension) {
        return Ok(());
    }
    Err(SdcpError::UnsupportedFileType {
        printer: printer.name.clone(),
        extension,
        supported: attributes.file_types.join(", "),
    })
}

/// Starts a file already on the printer, from the bottom layer.
pub fn start_print(wire: &Wire, filename: &str) -> Result<(), SendError> {
    match wire {
        Wire::Sdcp(printer) => {
            let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
            Ok(control.start_print(filename, 0)?)
        }
        Wire::Prusa(link) => Ok(net_prusalink::start_print(link, filename)?),
    }
}

/// Asks the printer what it is doing now.
pub fn state(wire: &Wire) -> Result<State, SendError> {
    match wire {
        Wire::Sdcp(printer) => {
            let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
            Ok(State::of_board(control.refresh_status()?))
        }
        Wire::Prusa(link) => Ok(State::of_prusa(net_prusalink::status(link)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    /// A board nobody answers for, on a port nothing listens on.
    fn a_silent_board() -> Wire {
        Wire::Sdcp(Printer {
            name: "Saturn".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([127, 0, 0, 1]),
            mainboard_id: "ff".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport: Transport::WebSocket,
        })
    }

    fn a_silent_prusa() -> Wire {
        Wire::Prusa(Link::api_key("127.0.0.1:1", "key"))
    }

    #[test]
    fn a_printer_that_is_not_there_fails_rather_than_hangs() {
        let path = std::env::temp_dir().join("printer-link-silent.goo");
        std::fs::write(&path, b"not a real stack").expect("the temporary directory is writable");
        for wire in [a_silent_board(), a_silent_prusa()] {
            let sent = upload(&wire, &path, &mut |_| {}, &|| false);
            assert!(
                matches!(sent, Err(ref error) if !error.is_cancelled()),
                "{sent:?}"
            );
            assert!(state(&wire).is_err(), "{} is not there", wire.name());
            assert!(start_print(&wire, "cube.goo").is_err());
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn only_a_board_counts_the_bytes_it_takes() {
        assert!(a_silent_board().counts_bytes());
        assert!(!a_silent_prusa().counts_bytes());
    }
}
