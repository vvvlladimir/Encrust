use net_sdcp::{Control, Task};

use crate::error::SendError;
use crate::state::State;
use crate::wire::{CONNECT_TIMEOUT, Wire};

/// What can be done to a print under way, the same on both protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Pause,
    Resume,
    Stop,
}

impl Command {
    /// The word on its button.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::Resume => "Resume",
            Self::Stop => "Stop",
        }
    }
}

/// Pauses, resumes or stops the print under way.
///
/// A board takes the command on a connection of its own; a Prusa machine is asked which
/// job is running first, because the command is addressed to it.
pub fn control(wire: &Wire, command: Command) -> Result<(), SendError> {
    match wire {
        Wire::Sdcp(printer) => {
            let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
            match command {
                Command::Pause => control.pause()?,
                Command::Resume => control.resume()?,
                Command::Stop => control.stop()?,
            }
            Ok(())
        }
        Wire::Prusa(link) => {
            let printing = net_prusalink::job(link)?.ok_or_else(|| SendError::Idle {
                printer: link.host.clone(),
            })?;
            match command {
                Command::Pause => net_prusalink::pause(link, printing.id)?,
                Command::Resume => net_prusalink::resume(link, printing.id)?,
                Command::Stop => net_prusalink::stop(link, printing.id)?,
            }
            Ok(())
        }
    }
}

/// The `count` most recent prints the printer remembers, most recent first. A Prusa
/// machine keeps no history its API serves, so it has none.
pub fn history(wire: &Wire, count: usize) -> Result<Vec<Task>, SendError> {
    let Wire::Sdcp(printer) = wire else {
        return Ok(Vec::new());
    };
    let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
    let mut ids = control.history()?;
    ids.truncate(count);
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(control.tasks(&ids)?)
}

/// The picture of the plate a printer keeps for a print, as the bytes it serves: a task's
/// thumbnail from the history, or the print under way when `task` is `None`.
pub fn picture(
    wire: &Wire,
    state: &State,
    task: Option<&Task>,
) -> Result<Option<Vec<u8>>, SendError> {
    match wire {
        Wire::Sdcp(printer) => {
            let url = match (task, &state.job_id) {
                (Some(task), _) => Some(task.thumbnail.clone()),
                (None, Some(id)) => {
                    let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
                    let tasks = control.tasks(std::slice::from_ref(id))?;
                    tasks.into_iter().next().map(|task| task.thumbnail)
                }
                (None, None) => None,
            };
            match url.filter(|url| !url.is_empty()) {
                Some(url) => Ok(Some(net_sdcp::picture(printer, &url)?)),
                None => Ok(None),
            }
        }
        Wire::Prusa(link) => {
            let path = net_prusalink::job(link)?
                .and_then(|printing| printing.file)
                .and_then(|file| file.refs.thumbnail);
            match path {
                Some(path) => Ok(Some(net_prusalink::picture(link, &path)?)),
                None => Ok(None),
            }
        }
    }
}

/// Turns the printer's camera on or off, answering the RTSP URL it streams to while on.
/// A printer without one answers `None`.
pub fn camera(wire: &Wire, on: bool) -> Result<Option<String>, SendError> {
    match wire {
        Wire::Sdcp(printer) => {
            let mut control = Control::connect(printer, CONNECT_TIMEOUT)?;
            Ok(control.video(on)?)
        }
        Wire::Prusa(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_prusalink::Link;
    use net_sdcp::{Printer, Transport};
    use std::net::IpAddr;

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

    #[test]
    fn a_printer_that_is_not_there_fails_every_errand() {
        for wire in [
            a_silent_board(),
            Wire::Prusa(Link::api_key("127.0.0.1:1", "key")),
        ] {
            for command in [Command::Pause, Command::Resume, Command::Stop] {
                assert!(control(&wire, command).is_err(), "{command:?}");
            }
            let printing = State {
                job_id: Some("0e8a".to_owned()),
                ..State::default()
            };
            assert!(picture(&wire, &printing, None).is_err());
        }
        assert!(history(&a_silent_board(), 5).is_err());
        assert!(camera(&a_silent_board(), true).is_err());
    }

    #[test]
    fn a_prusa_machine_has_no_history_and_no_camera_to_ask_about() {
        let prusa = Wire::Prusa(Link::api_key("127.0.0.1:1", "key"));
        assert!(history(&prusa, 5).expect("nothing is asked").is_empty());
        assert_eq!(camera(&prusa, true).expect("nothing is asked"), None);
    }

    /// A machine that answers the job query with an empty 204, as one printing nothing does.
    #[test]
    fn a_prusa_machine_printing_nothing_has_nothing_to_pause() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback binds");
        let port = listener
            .local_addr()
            .expect("a bound socket has an address")
            .port();
        let machine = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("the client connects");
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).expect("the client asks");
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .expect("the client is listening");
        });
        let wire = Wire::Prusa(Link::api_key(&format!("127.0.0.1:{port}"), "key"));
        let paused = control(&wire, Command::Pause);
        assert!(matches!(paused, Err(SendError::Idle { .. })), "{paused:?}");
        machine.join().expect("the machine answered");
    }

    #[test]
    fn a_board_printing_nothing_has_no_picture_to_fetch() {
        let idle = State::default();
        assert_eq!(
            picture(&a_silent_board(), &idle, None).expect("nothing is asked"),
            None
        );
    }
}
