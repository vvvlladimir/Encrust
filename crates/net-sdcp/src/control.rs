use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

use serde::Serialize;
use tungstenite::client::IntoClientRequest as _;
use tungstenite::handshake::client::ClientHandshake;
use tungstenite::{Message, WebSocket};

use crate::error::{ACK_BUSY, SdcpError, print_ack};
use crate::message::{Incoming, Request, request_topic};
use crate::mqtt::{Broker, Step, Waiting};
use crate::printer::{Attributes, Machine, Printer, Status, Transport};

/// The board serves its control socket here, see docs/formats/sdcp.md.
const CONTROL_PORT: u16 = 3030;

/// Where a version 1 board is told which broker to connect to, on the discovery port.
const DISCOVERY_PORT: u16 = 3000;

pub(crate) const CMD_STATUS: u32 = 0;
pub(crate) const CMD_ATTRIBUTES: u32 = 1;
pub(crate) const CMD_START_PRINT: u32 = 128;
pub(crate) const CMD_UPLOAD: u32 = 256;
const CMD_REPORT_PERIOD: u32 = 512;

/// How often a version 1 board is asked to report while it pulls a file, in ms. The vendor
/// client sends this, and a board that is not asked reports far less often.
const REPORT_PERIOD_MS: u32 = 5000;

/// A printer answers a command in well under a second when it is not busy.
const REPLY_WINDOW: Duration = Duration::from_secs(8);

/// How long a read blocks before the caller's deadline is checked again.
const POLL: Duration = Duration::from_millis(200);

/// How long a board that has just taken a file is given to check it before it will print.
const READY_WINDOW: Duration = Duration::from_secs(60);

/// How long to wait before asking a board that is still busy again.
const RETRY_PAUSE: Duration = Duration::from_secs(2);

/// The control connection to one printer: what it is doing, and what to do next.
pub struct Control {
    printer: Printer,
    link: Link,
    status: Status,
    /// Whether a status report arrived since the last time one was asked for.
    reported: bool,
    attributes: Attributes,
}

/// The two ways a board is reached. Version 3 serves a socket; version 1 connects to a
/// broker we run and is published to on a topic of its own.
enum Link {
    Socket(Box<WebSocket<TcpStream>>),
    Broker { broker: Broker, topic: String },
}

impl Control {
    /// Opens the control connection, which stays open until this is dropped.
    ///
    /// For a version 1 board that means binding a broker, telling the board where it is and
    /// waiting for it to connect back; see `docs/formats/sdcp.md`.
    pub fn connect(printer: &Printer, timeout: Duration) -> Result<Self, SdcpError> {
        let link = match printer.transport {
            Transport::WebSocket => Link::Socket(Box::new(socket(printer, timeout)?)),
            Transport::Mqtt => broker(printer, timeout)?,
        };
        let mut control = Self {
            printer: printer.clone(),
            link,
            status: Status::default(),
            reported: false,
            attributes: Attributes::default(),
        };
        if matches!(control.printer.transport, Transport::Mqtt) {
            control.greet()?;
        }
        Ok(control)
    }

    pub fn printer(&self) -> &Printer {
        &self.printer
    }

    /// The last state the printer reported, without asking for a new one.
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Asks for a fresh state and waits for it.
    ///
    /// The acknowledgement carries no state: the board sends it afterwards on its status
    /// topic, so the wait goes on until that report is in.
    pub fn refresh_status(&mut self) -> Result<&Status, SdcpError> {
        self.reported = false;
        self.request(CMD_STATUS, ())?;
        let deadline = Instant::now() + REPLY_WINDOW;
        while !self.reported {
            if Instant::now() >= deadline {
                return Err(SdcpError::TimedOut {
                    what: "the printer's status",
                    seconds: REPLY_WINDOW.as_secs(),
                });
            }
            self.pump(None, (Instant::now() + POLL).min(deadline))?;
        }
        Ok(&self.status)
    }

    /// Asks what the printer is and what it can do.
    ///
    /// A version 1 board answers this with its status rather than its attributes, so what
    /// comes back is empty and the caller has nothing to check against. Discovery is where
    /// that generation says what it is.
    pub fn attributes(&mut self) -> Result<&Attributes, SdcpError> {
        self.request(CMD_ATTRIBUTES, ())?;
        // The attributes arrive on their own topic, which may trail the acknowledgement.
        if self.attributes.model.is_empty() {
            self.pump(None, Instant::now() + POLL * 5)?;
        }
        Ok(&self.attributes)
    }

    /// Starts a print of a file already on the printer, from the bottom layer up.
    ///
    /// A board checks a file it has just taken and refuses to print until it is done, so
    /// the start waits that out and asks again while the board says it is busy.
    pub fn start_print(&mut self, filename: &str, start_layer: u32) -> Result<(), SdcpError> {
        let deadline = Instant::now() + READY_WINDOW;
        loop {
            let status = self.refresh_status()?;
            let printing = status.machine() == Machine::Printing && !status.is_settling();
            if !status.is_settling() {
                let data = StartPrint {
                    filename,
                    start_layer,
                };
                let ack = self.request(CMD_START_PRINT, data)?;
                // A board printing something else stays busy, so asking again only stalls.
                if ack != ACK_BUSY || printing || Instant::now() >= deadline {
                    return print_ack(ack);
                }
            } else if Instant::now() >= deadline {
                return Err(SdcpError::TimedOut {
                    what: "the printer checking the file",
                    seconds: READY_WINDOW.as_secs(),
                });
            }
            self.pump(None, Instant::now() + RETRY_PAUSE)?;
        }
    }

    /// Takes in whatever the printer has reported since the last call, without waiting.
    pub fn poll(&mut self) -> Result<(), SdcpError> {
        self.pump(None, Instant::now()).map(|_| ())
    }

    /// Sends one command and waits for its acknowledgement.
    pub(crate) fn request<T: Serialize>(&mut self, cmd: u32, data: T) -> Result<u32, SdcpError> {
        let (request, request_id) = Request::new(&self.printer, cmd, data);
        let text = serde_json::to_string(&request)?;
        self.link.send(&text)?;
        let deadline = Instant::now() + REPLY_WINDOW;
        self.pump(Some(&request_id), deadline)?
            .ok_or(SdcpError::TimedOut {
                what: "the printer's reply",
                seconds: REPLY_WINDOW.as_secs(),
            })
    }

    /// Reads reports until `awaited` is answered or the deadline passes, whichever is
    /// first; an unsolicited report updates what this connection knows either way.
    pub(crate) fn pump(
        &mut self,
        awaited: Option<&str>,
        deadline: Instant,
    ) -> Result<Option<u32>, SdcpError> {
        loop {
            let Some((topic, text)) = self.link.read(deadline)? else {
                // Nothing arrived. Only the deadline ends the wait: a caller passing one
                // already in the past is the one asking for a look rather than a wait.
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                continue;
            };
            if let Some(ack) = self.take(&text, topic.as_deref(), awaited) {
                return Ok(Some(ack));
            }
            if Instant::now() >= deadline && awaited.is_none() {
                return Ok(None);
            }
        }
    }

    /// Files one report, and reports the acknowledgement the caller is waiting for.
    fn take(&mut self, text: &str, topic: Option<&str>, awaited: Option<&str>) -> Option<u32> {
        match Incoming::parse(text, topic) {
            Ok(Incoming::Status(status)) => {
                self.status = *status;
                self.reported = true;
            }
            Ok(Incoming::Attributes(attributes)) => self.attributes = *attributes,
            Ok(Incoming::Error(code)) => {
                tracing::warn!(printer = %self.printer.name, code, "the printer reported an error");
            }
            Ok(Incoming::Response { request_id, ack }) => {
                if awaited == Some(request_id.as_str()) {
                    return Some(ack.unwrap_or_default());
                }
            }
            Ok(Incoming::Other) => {}
            // A heartbeat is a bare "pong", and firmware sends fields we do not read.
            Err(error) => tracing::debug!(%error, "ignoring a report from the printer"),
        }
        None
    }

    /// The three commands a vendor client sends a version 1 board before it asks for anything.
    ///
    /// The first two carry no data and are not documented anywhere; the third sets how
    /// often the board reports, which is what the transfer's progress is read from.
    fn greet(&mut self) -> Result<(), SdcpError> {
        self.request(CMD_STATUS, ())?;
        self.request(CMD_ATTRIBUTES, ())?;
        self.request(
            CMD_REPORT_PERIOD,
            ReportPeriod {
                time_period: REPORT_PERIOD_MS,
            },
        )?;
        Ok(())
    }
}

impl Link {
    fn send(&mut self, text: &str) -> Result<(), SdcpError> {
        match self {
            Self::Socket(socket) => Ok(socket.send(Message::text(text.to_owned()))?),
            Self::Broker { broker, topic } => broker.publish(topic, text),
        }
    }

    /// One report, with the topic it arrived on when the transport knows it.
    fn read(&mut self, deadline: Instant) -> Result<Option<(Option<String>, String)>, SdcpError> {
        match self {
            Self::Socket(socket) => match socket.read() {
                Ok(Message::Text(text)) => Ok(Some((None, text.to_string()))),
                Ok(_) => Ok(None),
                Err(error) if would_block(&error) => Ok(None),
                Err(error) => Err(error.into()),
            },
            Self::Broker { broker, .. } => match broker.step(deadline)? {
                Step::Message(topic, payload) => Ok(Some((Some(topic), payload))),
                // A closed broker reports `Idle` at once, so waiting on it would spin.
                Step::Idle if broker.is_closed() => Err(SdcpError::BoardClosed),
                Step::Handled | Step::Idle => Ok(None),
            },
        }
    }
}

fn socket(printer: &Printer, timeout: Duration) -> Result<WebSocket<TcpStream>, SdcpError> {
    let address = SocketAddr::from((printer.address, CONTROL_PORT));
    let stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    let url = format!("ws://{address}/websocket");
    let (socket, _) =
        tungstenite::client(url.into_client_request()?, stream).map_err(handshake_failed)?;
    socket.get_ref().set_read_timeout(Some(POLL))?;
    Ok(socket)
}

/// Binds a broker, tells the board its port with `M66666`, and waits for it to connect and
/// subscribe. A board that connects under another id is not the board we asked.
fn broker(printer: &Printer, timeout: Duration) -> Result<Link, SdcpError> {
    let waiting = Waiting::bind()?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    let invitation = format!("M66666 {}", waiting.port()?);
    socket.send_to(
        invitation.as_bytes(),
        SocketAddr::from((printer.address, DISCOVERY_PORT)),
    )?;

    let mut broker = waiting.accept(timeout)?;
    let topic = request_topic(printer);
    let deadline = Instant::now() + timeout;
    while !broker.has_subscribed(&topic) {
        if broker.step(deadline)? == Step::Idle {
            return Err(SdcpError::TimedOut {
                what: "the board subscribing to its own topic",
                seconds: timeout.as_secs(),
            });
        }
    }
    match broker.client_id() {
        Some(id) if id == printer.mainboard_id => Ok(Link::Broker { broker, topic }),
        other => Err(SdcpError::WrongBoard {
            expected: printer.mainboard_id.clone(),
            connected: other.unwrap_or_default().to_owned(),
        }),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct StartPrint<'a> {
    filename: &'a str,
    start_layer: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ReportPeriod {
    time_period: u32,
}

/// A handshake that was interrupted cannot be resumed here, because the socket is
/// blocking; either way the connection did not open.
fn handshake_failed(error: tungstenite::HandshakeError<ClientHandshake<TcpStream>>) -> SdcpError {
    match error {
        tungstenite::HandshakeError::Failure(error) => error.into(),
        tungstenite::HandshakeError::Interrupted(_) => SdcpError::TimedOut {
            what: "the control handshake",
            seconds: REPLY_WINDOW.as_secs(),
        },
    }
}

fn would_block(error: &tungstenite::Error) -> bool {
    match error {
        tungstenite::Error::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_timeout_is_not_a_broken_connection() {
        let timed_out =
            tungstenite::Error::Io(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(would_block(&timed_out));
        assert!(!would_block(&tungstenite::Error::ConnectionClosed));
    }

    #[test]
    fn a_start_print_command_names_the_file_as_the_protocol_spells_it() {
        let data = StartPrint {
            filename: "model.goo",
            start_layer: 0,
        };
        let text = serde_json::to_string(&data).expect("the command serialises");
        assert_eq!(text, r#"{"Filename":"model.goo","StartLayer":0}"#);
    }

    #[test]
    fn the_report_period_is_spelled_as_the_board_expects() {
        let text = serde_json::to_string(&ReportPeriod {
            time_period: REPORT_PERIOD_MS,
        })
        .expect("the command serialises");
        assert_eq!(text, r#"{"TimePeriod":5000}"#);
    }

    #[test]
    fn a_board_that_never_connects_to_the_broker_times_out() {
        // Nothing is listening on the discovery port of this address, so the invitation
        // goes nowhere and the accept window closes on its own.
        let printer = Printer {
            name: "Saturn".into(),
            model: "Saturn 3 Ultra".into(),
            brand: "ELEGOO".into(),
            address: std::net::IpAddr::from([127, 0, 0, 1]),
            mainboard_id: "ABCD1234".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: "V1.0.0".into(),
            transport: Transport::Mqtt,
        };
        assert!(matches!(
            Control::connect(&printer, Duration::from_millis(200)),
            Err(SdcpError::TimedOut { .. })
        ));
    }
}
