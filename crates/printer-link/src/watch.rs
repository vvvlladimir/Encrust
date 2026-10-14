use std::time::{Duration, Instant};

use net_sdcp::{Control, Printer, Transport};

use crate::error::SendError;
use crate::state::State;
use crate::wire::{CONNECT_TIMEOUT, Wire};

/// How long a version 1 board is given to answer the probe it is watched through.
const PROBE_WINDOW: Duration = Duration::from_secs(1);

/// How often `stop` is asked while waiting for the next look.
const POLL: Duration = Duration::from_millis(200);

/// Reports what the printer is doing every `period` until `stop` answers yes.
///
/// A version 3 board is watched over one control connection held for the whole watch and
/// reopened after a failure; see ADR 0262. A version 1 board is asked through the probe,
/// which carries its status, and a Prusa machine through its status call. A failure is
/// reported like a state, and the watch goes on.
pub fn watch(
    wire: &Wire,
    period: Duration,
    stop: &dyn Fn() -> bool,
    report: &mut dyn FnMut(Result<State, SendError>),
) {
    while !stop() {
        match wire {
            Wire::Sdcp(printer) if printer.transport == Transport::WebSocket => {
                over_socket(printer, period, stop, report);
            }
            Wire::Sdcp(printer) => report(
                net_sdcp::report(printer.address, PROBE_WINDOW)
                    .map(|status| State::of_board(&status))
                    .map_err(SendError::from),
            ),
            Wire::Prusa(link) => report(
                net_prusalink::status(link)
                    .map(State::of_prusa)
                    .map_err(SendError::from),
            ),
        }
        pause(period, stop);
    }
}

/// Holds one control connection and asks it for the state every `period`, until it fails
/// or the watch is stopped.
fn over_socket(
    printer: &Printer,
    period: Duration,
    stop: &dyn Fn() -> bool,
    report: &mut dyn FnMut(Result<State, SendError>),
) {
    let mut control = match Control::connect(printer, CONNECT_TIMEOUT) {
        Ok(control) => control,
        Err(error) => return report(Err(error.into())),
    };
    while !stop() {
        match control.refresh_status() {
            Ok(status) => report(Ok(State::of_board(status))),
            Err(error) => return report(Err(error.into())),
        }
        pause(period, stop);
    }
}

/// Waits out `period`, or less when the watch is stopped meanwhile.
fn pause(period: Duration, stop: &dyn Fn() -> bool) {
    let until = Instant::now() + period;
    while !stop() && Instant::now() < until {
        std::thread::sleep(POLL.min(until.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_prusalink::Link;
    use std::cell::Cell;
    use std::net::IpAddr;

    fn a_silent_board(transport: Transport) -> Wire {
        Wire::Sdcp(Printer {
            name: "Saturn".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([127, 0, 0, 1]),
            mainboard_id: "ff".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport,
        })
    }

    /// Nobody answers on the loopback, so each kind of printer reports a failure, and the
    /// watch ends as soon as it is told to rather than after its period.
    #[test]
    fn a_printer_that_is_not_there_is_reported_and_the_watch_still_stops() {
        let wires = [
            a_silent_board(Transport::WebSocket),
            a_silent_board(Transport::Mqtt),
            Wire::Prusa(Link::api_key("127.0.0.1:1", "key")),
        ];
        for wire in wires {
            let reports = Cell::new(0);
            let started = Instant::now();
            watch(
                &wire,
                Duration::from_secs(60),
                &|| reports.get() > 0,
                &mut |got| {
                    assert!(got.is_err(), "{} is not there", wire.name());
                    reports.set(reports.get() + 1);
                },
            );
            assert_eq!(reports.get(), 1);
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the stop is heard inside the period"
            );
        }
    }
}
