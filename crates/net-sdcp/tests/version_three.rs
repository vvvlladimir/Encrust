//! A version 3 board, played against the real client over its WebSocket: it acknowledges a
//! command first and reports its status afterwards, on a topic of its own, as the
//! specification describes; see `docs/formats/sdcp.md`.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::thread::JoinHandle;
use std::time::Duration;

use net_sdcp::{Control, Machine, Printer, SdcpError, Stage, Transport};
use serde_json::{Value, json};
use tungstenite::Message;

/// The port the protocol fixes the control socket on.
const CONTROL_PORT: u16 = 3030;

/// Every test plays a board on that one port, so they take it in turns.
static PORT: std::sync::Mutex<()> = std::sync::Mutex::new(());

const BOARD: &str = "ABCD1234ABCD1234";

/// How late the status report trails the acknowledgement of the command that asked for it.
const REPORT_LAG: Duration = Duration::from_millis(300);

fn a_printer() -> Printer {
    Printer {
        name: "Saturn4Ultra".into(),
        model: "ELEGOO Saturn 4 Ultra".into(),
        brand: "ELEGOO".into(),
        address: IpAddr::from(Ipv4Addr::LOCALHOST),
        mainboard_id: BOARD.into(),
        brand_id: "0a69ee780fbd40d7bfb95b312250bf46".into(),
        firmware: "V1.2.3".into(),
        protocol: "V3.0.0".into(),
        transport: Transport::WebSocket,
    }
}

/// What the board reports the `n`th time it is asked, and what it answers a start with.
#[derive(Clone, Copy)]
struct Script {
    status: fn(usize) -> Value,
    start_ack: fn(usize) -> u32,
}

/// Plays one connection of a board on the control port, and returns the acks it gave
/// each start command once the client hangs up.
fn a_board(script: Script) -> (std::sync::MutexGuard<'static, ()>, JoinHandle<Vec<u32>>) {
    let held = PORT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, CONTROL_PORT)).unwrap_or_else(|error| {
        panic!(
            "TCP port {CONTROL_PORT} on loopback is taken ({error}); the protocol fixes it there"
        )
    });
    let board = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the client connects");
        let mut socket = tungstenite::accept(stream).expect("the client opens a WebSocket");
        let (mut asked, mut starts) = (0, Vec::new());
        while let Ok(message) = socket.read() {
            let Message::Text(text) = message else {
                continue;
            };
            let request: Value = serde_json::from_str(&text).expect("a request is JSON");
            let cmd = request["Data"]["Cmd"]
                .as_u64()
                .expect("a request has a command");
            let ack = match cmd {
                128 => {
                    let ack = (script.start_ack)(starts.len());
                    starts.push(ack);
                    ack
                }
                _ => 0,
            };
            let response = json!({"Id": "x", "Data": {"Cmd": cmd, "Data": {"Ack": ack},
                "RequestID": request["Data"]["RequestID"], "MainboardID": BOARD,
                "TimeStamp": 0}, "Topic": format!("sdcp/response/{BOARD}")});
            socket
                .send(Message::text(response.to_string()))
                .expect("the client is listening");
            if cmd == 0 {
                std::thread::sleep(REPORT_LAG);
                let status = json!({"Status": (script.status)(asked), "MainboardID": BOARD,
                    "TimeStamp": 0, "Topic": format!("sdcp/status/{BOARD}")});
                asked += 1;
                socket
                    .send(Message::text(status.to_string()))
                    .expect("the client is listening");
            }
        }
        starts
    });
    (held, board)
}

fn connected() -> Control {
    Control::connect(&a_printer(), Duration::from_secs(5)).expect("the board is listening")
}

#[test]
fn a_status_is_the_report_that_follows_the_acknowledgement() {
    let (_held, board) = a_board(Script {
        status: |_| {
            json!({"CurrentStatus": [1], "PrintInfo": {"Status": 3,
            "CurrentLayer": 3, "TotalLayer": 6, "Filename": "cube.goo"}})
        },
        start_ack: |_| 0,
    });
    let mut control = connected();
    let status = control.refresh_status().expect("the board reports");
    assert_eq!(
        status.machine(),
        Machine::Printing,
        "the board said it prints"
    );
    assert_eq!(status.print_info.stage(), Stage::Exposing);
    assert_eq!(status.print_info.current_layer, 3);
    drop(control);
    board.join().expect("the board ran to the end");
}

#[test]
fn a_print_starts_once_the_board_has_checked_the_file_it_was_sent() {
    let (_held, board) = a_board(Script {
        status: |asked| match asked {
            0 => json!({"CurrentStatus": [0], "PrintInfo": {"Status": 10}}),
            _ => json!({"CurrentStatus": [0], "PrintInfo": {"Status": 0}}),
        },
        // Settled by its own report, the board still refuses the first start once.
        start_ack: |tried| u32::from(tried == 0),
    });
    let mut control = connected();
    control
        .start_print("cube.goo", 0)
        .expect("the board starts once it is free");
    drop(control);
    let starts = board.join().expect("the board ran to the end");
    assert_eq!(
        starts,
        vec![1, 0],
        "no start while it checks, one more after busy"
    );
}

#[test]
fn a_board_printing_something_else_refuses_at_once() {
    let (_held, board) = a_board(Script {
        status: |_| json!({"CurrentStatus": [1], "PrintInfo": {"Status": 3}}),
        start_ack: |_| 1,
    });
    let mut control = connected();
    let refused = control.start_print("cube.goo", 0);
    assert!(
        matches!(refused, Err(SdcpError::PrintRefused { ack: 1, .. })),
        "{refused:?}"
    );
    drop(control);
    let starts = board.join().expect("the board ran to the end");
    assert_eq!(starts, vec![1], "asked once, not until the window closes");
}
