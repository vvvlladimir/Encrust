//! A version 1 board, played against the real client: it is invited over UDP, connects to
//! the broker, answers every command, fetches the file over HTTP and reports as it goes.
//!
//! This is the only place the whole sequence is exercised without hardware. The board here
//! does what `vvuk/cassini` recorded a Saturn 3 Ultra doing; see `docs/formats/sdcp.md`.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;

use net_sdcp::{Printer, Transfer, Transport};

/// The port the protocol fixes the invitation on.
const DISCOVERY_PORT: u16 = 3000;

/// Both tests play a board on that one port, so they take it in turns.
static PORT: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Binds the invitation socket, holding the port against the other test.
fn listening() -> (std::sync::MutexGuard<'static, ()>, UdpSocket) {
    let held = PORT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, DISCOVERY_PORT)).unwrap_or_else(|error| {
        panic!(
            "UDP port {DISCOVERY_PORT} on loopback is taken ({error}); the protocol fixes it there"
        )
    });
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("the socket takes a timeout");
    (held, socket)
}

const BOARD: &str = "ABCD1234ABCD1234";
const BRAND: &str = "0a69ee780fbd40d7bfb95b312250bf46";

const CONNECT: u8 = 1;
const CONNACK: u8 = 2;
const PUBLISH: u8 = 3;
const SUBSCRIBE: u8 = 8;
const SUBACK: u8 = 9;

fn a_printer() -> Printer {
    Printer {
        name: "Saturn3Ultra".into(),
        model: "ELEGOO Saturn 3 Ultra".into(),
        brand: "ELEGOO".into(),
        address: IpAddr::from(Ipv4Addr::LOCALHOST),
        mainboard_id: BOARD.into(),
        brand_id: BRAND.into(),
        firmware: "V1.4.2".into(),
        protocol: "V1.0.0".into(),
        transport: Transport::Mqtt,
    }
}

fn a_file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, bytes).expect("the temporary directory is writable");
    path
}

#[test]
fn a_version_one_board_is_invited_told_where_to_fetch_and_fetches() {
    let stack = b"a sliced stack, as far as the board is concerned".repeat(64);
    let path = a_file("encrust-sdcp-v1-upload.goo", &stack);

    // Bound before the client runs, because the invitation is a single datagram.
    let (_held, invitations) = listening();

    let printer = a_printer();
    let sending = std::thread::spawn({
        let path = path.clone();
        move || {
            let mut seen = Vec::new();
            let landed = net_sdcp::upload(&printer, &path, &mut |transfer: Transfer| {
                seen.push(transfer);
                true
            });
            (landed, seen)
        }
    });

    let mut board = Board::invited(&invitations);
    board.handshake();
    let fetched = board.serve_until_done();

    let (landed, reported) = sending.join().expect("the client thread did not panic");
    assert_eq!(
        landed.expect("the board accepted the file"),
        "encrust-sdcp-v1-upload.goo"
    );
    assert_eq!(fetched, stack, "the board fetched the file byte for byte");
    assert_eq!(
        reported.last().map(|transfer| transfer.fraction()),
        Some(1.0),
        "and the last thing reported is a finished transfer"
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn a_board_that_gives_up_fetching_is_reported_rather_than_waited_on() {
    let path = a_file("encrust-sdcp-v1-failed.goo", b"a stack");
    let (_held, invitations) = listening();

    let printer = a_printer();
    let sending = std::thread::spawn({
        let path = path.clone();
        move || net_sdcp::upload(&printer, &path, &mut |_| true)
    });

    let mut board = Board::invited(&invitations);
    board.handshake();
    board.answer_commands();
    board.report_transfer(0, 7, 3);

    let error = sending
        .join()
        .expect("the client thread did not panic")
        .expect_err("a board that gave up is not a transfer that worked");
    assert!(error.to_string().contains("gave up"), "got {error}");
    std::fs::remove_file(path).ok();
}

#[test]
fn a_board_that_hangs_up_mid_transfer_ends_the_send() {
    let path = a_file("encrust-sdcp-v1-hangup.goo", b"a stack");
    let (_held, invitations) = listening();

    let printer = a_printer();
    let sending = std::thread::spawn({
        let path = path.clone();
        move || net_sdcp::upload(&printer, &path, &mut |_| true)
    });

    let mut board = Board::invited(&invitations);
    board.handshake();
    board.answer_commands();
    drop(board);

    let error = sending
        .join()
        .expect("the client thread did not panic")
        .expect_err("a board that left is not a transfer that worked");
    assert!(
        error.to_string().contains("closed the connection"),
        "got {error}"
    );
    std::fs::remove_file(path).ok();
}

/// The board's side of the conversation.
struct Board {
    link: TcpStream,
    pending: Vec<u8>,
    /// The URL command 256 named, once it has been sent one.
    url: Option<String>,
}

impl Board {
    /// Waits for the invitation and connects back to the broker it names.
    fn invited(invitations: &UdpSocket) -> Self {
        let mut datagram = [0u8; 64];
        let (read, _) = invitations
            .recv_from(&mut datagram)
            .expect("the client invites the board over UDP");
        let text = String::from_utf8_lossy(&datagram[..read]).into_owned();
        let port: u16 = text
            .strip_prefix("M66666 ")
            .expect("the invitation is M66666 and a port")
            .trim()
            .parse()
            .expect("the port is a number");

        let link = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .expect("the broker is listening on the port it named");
        link.set_read_timeout(Some(Duration::from_secs(10)))
            .expect("the socket takes a timeout");
        Self {
            link,
            pending: Vec::new(),
            url: None,
        }
    }

    /// Connects and subscribes, as the board does before anything else.
    fn handshake(&mut self) {
        let mut connect = Vec::new();
        put_string(&mut connect, "MQTT");
        connect.extend_from_slice(&[0x04, 0x02, 0x00, 0x3c]);
        put_string(&mut connect, BOARD);
        self.send(CONNECT, 0, &connect);
        let (kind, _) = self.packet();
        assert_eq!(kind, CONNACK, "the broker accepts the connection");

        let mut subscribe = 1u16.to_be_bytes().to_vec();
        put_string(&mut subscribe, &format!("/sdcp/request/{BOARD}"));
        subscribe.push(0);
        self.send(SUBSCRIBE, 0x02, &subscribe);
        let (kind, _) = self.packet();
        assert_eq!(kind, SUBACK, "and grants the subscription");
    }

    /// Acknowledges every command until command 256 has named a URL.
    fn answer_commands(&mut self) {
        while self.url.is_none() {
            let (kind, body) = self.packet();
            assert_eq!(kind, PUBLISH, "the client only publishes");
            let (_, payload) = take_string(&body);
            let request: serde_json::Value =
                serde_json::from_slice(payload).expect("a command is JSON");
            let data = &request["Data"];
            let request_id = data["RequestID"]
                .as_str()
                .expect("every command carries a request id")
                .to_owned();
            if data["Cmd"].as_u64() == Some(256) {
                self.url = Some(
                    data["Data"]["URL"]
                        .as_str()
                        .expect("command 256 names a URL")
                        .to_owned(),
                );
                assert!(
                    data["Data"]["FileSize"].as_u64().is_some(),
                    "and the size to expect"
                );
            }
            self.acknowledge(&request_id);
        }
    }

    /// Fetches the file the client put up, reporting as a board would, and answers done.
    fn serve_until_done(&mut self) -> Vec<u8> {
        self.answer_commands();
        let url = self
            .url
            .clone()
            .expect("the client named a URL to fetch from")
            // The board substitutes the address it is connected to; here that is loopback.
            .replace("${ipaddr}", "127.0.0.1");

        let body = ureq::get(&url)
            .call()
            .expect("the client is serving the file")
            .into_body()
            .read_to_vec()
            .expect("and the whole body arrives");

        let size = body.len() as u64;
        self.report_transfer(size / 2, size, 0);
        self.report_transfer(size, size, 2);
        body
    }

    /// One status report, in the shape version 1 sends one.
    fn report_transfer(&mut self, offset: u64, total: u64, state: u32) {
        let report = serde_json::json!({
            "Id": BRAND,
            "Data": {
                "Status": {
                    "CurrentStatus": u32::from(state == 0),
                    "PreviousStatus": 0,
                    "PrintInfo": { "Status": 0, "CurrentLayer": 0, "TotalLayer": 0 },
                    "FileTransferInfo": {
                        "Status": state,
                        "DownloadOffset": offset,
                        "CheckOffset": 0,
                        "FileTotalSize": total,
                        "Filename": "encrust-sdcp-v1-upload.goo",
                    },
                },
                "MainboardID": BOARD,
                "TimeStamp": 8_629_636,
            },
        });
        self.publish(&format!("/sdcp/status/{BOARD}"), &report.to_string());
    }

    fn acknowledge(&mut self, request_id: &str) {
        let response = serde_json::json!({
            "Id": BRAND,
            "Data": {
                "Data": { "Ack": 0 },
                "RequestID": request_id,
                "MainboardID": BOARD,
                "TimeStamp": 8_567_213,
            },
        });
        self.publish(&format!("/sdcp/response/{BOARD}"), &response.to_string());
    }

    fn publish(&mut self, topic: &str, payload: &str) {
        let mut body = Vec::new();
        put_string(&mut body, topic);
        body.extend_from_slice(payload.as_bytes());
        self.send(PUBLISH, 0, &body);
    }

    fn send(&mut self, kind: u8, flags: u8, body: &[u8]) {
        let mut packet = vec![kind << 4 | flags];
        put_remaining_length(&mut packet, body.len());
        packet.extend_from_slice(body);
        self.link.write_all(&packet).expect("the broker is reading");
        self.link.flush().expect("the broker is reading");
    }

    /// The next whole packet the broker sent, as its kind and its body.
    fn packet(&mut self) -> (u8, Vec<u8>) {
        loop {
            if let Some(packet) = self.take() {
                return packet;
            }
            let mut chunk = [0u8; 4096];
            let read = self
                .link
                .read(&mut chunk)
                .expect("the broker keeps the connection open");
            assert!(read > 0, "the broker hung up mid-conversation");
            self.pending.extend_from_slice(&chunk[..read]);
        }
    }

    fn take(&mut self) -> Option<(u8, Vec<u8>)> {
        if self.pending.len() < 2 {
            return None;
        }
        let (length, header) = remaining_length(&self.pending[1..])?;
        let header = header + 1;
        if self.pending.len() < header + length {
            return None;
        }
        let kind = self.pending[0] >> 4;
        let body = self.pending[header..header + length].to_vec();
        self.pending.drain(..header + length);
        Some((kind, body))
    }
}

fn put_string(body: &mut Vec<u8>, text: &str) {
    let length = u16::try_from(text.len()).expect("a topic fits in two bytes");
    body.extend_from_slice(&length.to_be_bytes());
    body.extend_from_slice(text.as_bytes());
}

fn take_string(body: &[u8]) -> (String, &[u8]) {
    let length = usize::from(u16::from_be_bytes([body[0], body[1]]));
    let text = String::from_utf8_lossy(&body[2..2 + length]).into_owned();
    (text, &body[2 + length..])
}

fn put_remaining_length(packet: &mut Vec<u8>, mut length: usize) {
    loop {
        let mut digit = (length % 128) as u8;
        length /= 128;
        if length > 0 {
            digit |= 0x80;
        }
        packet.push(digit);
        if length == 0 {
            return;
        }
    }
}

fn remaining_length(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut length = 0usize;
    let mut multiplier = 1usize;
    for (read, byte) in bytes.iter().enumerate() {
        length += usize::from(byte & 0x7f) * multiplier;
        if byte & 0x80 == 0 {
            return Some((length, read + 1));
        }
        multiplier *= 128;
    }
    None
}
