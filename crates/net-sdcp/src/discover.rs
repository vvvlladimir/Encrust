use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::SdcpError;
use crate::printer::{Printer, Status, Transport};

/// The port every board listens on for the discovery probe, see docs/formats/sdcp.md.
const DISCOVERY_PORT: u16 = 3000;

/// The probe itself: printers answer this string and nothing else.
const PROBE: &[u8] = b"M99999";

/// A reply can arrive at any time inside the window, so the socket wakes up often.
const POLL: Duration = Duration::from_millis(200);

/// Ask every printer on the local network to introduce itself, for `window`.
///
/// Answers are collected until the window closes, because there is no count to wait for.
/// Fails only when the probe could not go out on any interface.
pub fn discover(window: Duration) -> Result<Vec<Printer>, SdcpError> {
    let socket = probe_socket()?;
    let mut sent = false;
    let mut failure = None;
    for address in broadcast_addresses() {
        match socket.send_to(PROBE, SocketAddr::from((address, DISCOVERY_PORT))) {
            Ok(_) => sent = true,
            Err(error) => {
                tracing::debug!(%address, %error, "the discovery probe did not go out");
                failure.get_or_insert(error);
            }
        }
    }
    if let (false, Some(error)) = (sent, failure) {
        return Err(error.into());
    }
    Ok(collect(&socket, window)
        .into_iter()
        .map(|answer| answer.printer)
        .collect())
}

/// Ask one printer by address, for when broadcast does not cross the network.
pub fn probe(address: IpAddr, window: Duration) -> Result<Option<Printer>, SdcpError> {
    let socket = probe_socket()?;
    socket.send_to(PROBE, SocketAddr::from((address, DISCOVERY_PORT)))?;
    Ok(collect(&socket, window)
        .into_iter()
        .next()
        .map(|answer| answer.printer))
}

/// What the board at `address` says it is doing in its answer to the probe.
///
/// Only version 1 says anything there, which is what lets that generation be watched
/// without a broker; a version 3 board answers with who it is and nothing else.
pub fn report(address: IpAddr, window: Duration) -> Result<Status, SdcpError> {
    let socket = probe_socket()?;
    socket.send_to(PROBE, SocketAddr::from((address, DISCOVERY_PORT)))?;
    collect(&socket, window)
        .into_iter()
        .find_map(|answer| answer.status)
        .ok_or(SdcpError::TimedOut {
            what: "the board's answer to the probe",
            seconds: window.as_secs().max(1),
        })
}

fn probe_socket() -> Result<UdpSocket, SdcpError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(POLL))?;
    Ok(socket)
}

/// Each interface's own broadcast address, then the limited one, which macOS refuses
/// with `EHOSTUNREACH`; see docs/formats/sdcp.md.
fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut addresses: Vec<Ipv4Addr> = interfaces()
        .into_iter()
        .filter_map(|(ip, netmask)| directed_broadcast(ip, netmask))
        .collect();
    addresses.sort_unstable();
    addresses.dedup();
    addresses.push(Ipv4Addr::BROADCAST);
    addresses
}

/// Every IPv4 address this machine holds off the loopback, with its netmask.
#[cfg(not(target_arch = "wasm32"))]
fn interfaces() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let listed = if_addrs::get_if_addrs().unwrap_or_else(|error| {
        tracing::debug!(%error, "the interfaces could not be listed");
        Vec::new()
    });
    listed
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(v4) => Some((v4.ip, v4.netmask)),
            if_addrs::IfAddr::V6(_) => None,
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn interfaces() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    Vec::new()
}

/// The address every host of a subnet hears. A point-to-point link, `/31` or `/32`, has
/// no broadcast to send to.
fn directed_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Option<Ipv4Addr> {
    let mask = u32::from(netmask);
    (mask.leading_ones() < 31).then(|| Ipv4Addr::from(u32::from(ip) | !mask))
}

/// One board's answer to the probe: who it is, and what it is doing when it says.
pub(crate) struct Answer {
    pub(crate) printer: Printer,
    pub(crate) status: Option<Status>,
}

/// One board may answer twice when it holds two addresses, so replies key on its ID.
fn collect(socket: &UdpSocket, window: Duration) -> Vec<Answer> {
    let deadline = Instant::now() + window;
    let mut found: BTreeMap<String, Answer> = BTreeMap::new();
    let mut datagram = [0u8; 4096];
    while Instant::now() < deadline {
        let Ok((read, from)) = socket.recv_from(&mut datagram) else {
            continue;
        };
        match parse(&datagram[..read], from.ip()) {
            Ok(answer) => {
                found.insert(answer.printer.mainboard_id.clone(), answer);
            }
            Err(error) => tracing::debug!(%from, %error, "ignoring a reply to discovery"),
        }
    }
    found.into_values().collect()
}

/// The board reports its own address; the datagram's is the fallback when it omits one.
///
/// Both generations answer the same probe. Version 3 puts the fields straight under `Data`;
/// version 1 nests them under `Data.Attributes`, beside a `Data.Status` this does not read.
/// That nesting is what says which transport the board speaks, because it is structural —
/// a board that reports an unfamiliar `ProtocolVersion` is still placed by its own shape.
pub(crate) fn parse(datagram: &[u8], from: IpAddr) -> Result<Answer, SdcpError> {
    let reply: Reply = serde_json::from_slice(datagram)?;
    let nested = reply.data.attributes.is_some();
    let greeting = reply.data.attributes.unwrap_or(reply.data.flat);
    let address = greeting
        .mainboard_ip
        .and_then(|text| text.parse().ok())
        .unwrap_or(from);
    let transport = if nested || greeting.protocol_version.starts_with("V1") {
        Transport::Mqtt
    } else {
        Transport::WebSocket
    };
    let printer = Printer {
        name: greeting.name,
        model: greeting.machine_name,
        brand: greeting.brand_name,
        address,
        mainboard_id: greeting.mainboard_id,
        brand_id: reply.id,
        firmware: greeting.firmware_version,
        protocol: greeting.protocol_version,
        transport,
    };
    Ok(Answer {
        printer,
        status: reply.data.status,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Reply {
    #[serde(rename = "Id")]
    id: String,
    data: Envelope,
}

/// One struct for both shapes: the nested block when the board sends one, the loose fields
/// beside it when it does not.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Envelope {
    #[serde(default)]
    attributes: Option<Greeting>,
    #[serde(default)]
    status: Option<Status>,
    #[serde(flatten)]
    flat: Greeting,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Greeting {
    #[serde(default)]
    name: String,
    #[serde(default)]
    machine_name: String,
    #[serde(default)]
    brand_name: String,
    #[serde(rename = "MainboardIP")]
    mainboard_ip: Option<String>,
    #[serde(default, rename = "MainboardID")]
    mainboard_id: String,
    #[serde(default)]
    protocol_version: String,
    #[serde(default)]
    firmware_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = r#"{
        "Id": "6b1c4a2e6b1c4a2e6b1c4a2e6b1c4a2e",
        "Data": {
            "Name": "Saturn",
            "MachineName": "Saturn 4 Ultra",
            "BrandName": "ELEGOO",
            "MainboardIP": "192.168.1.42",
            "MainboardID": "000000000001d354",
            "ProtocolVersion": "V3.0.0",
            "FirmwareVersion": "V1.2.3"
        }
    }"#;

    fn sender() -> IpAddr {
        IpAddr::from([10, 0, 0, 9])
    }

    #[test]
    fn a_greeting_becomes_a_printer() {
        let printer = parse(REPLY.as_bytes(), sender())
            .expect("the reply is well formed")
            .printer;
        assert_eq!(printer.model, "Saturn 4 Ultra");
        assert_eq!(printer.mainboard_id, "000000000001d354");
        assert_eq!(printer.address, IpAddr::from([192, 168, 1, 42]));
    }

    #[test]
    fn the_datagram_address_stands_in_for_a_missing_one() {
        let without = REPLY.replace(r#""MainboardIP": "192.168.1.42","#, "");
        let printer = parse(without.as_bytes(), sender())
            .expect("the address is optional")
            .printer;
        assert_eq!(printer.address, sender());
    }

    /// A Saturn 3 Ultra, whose reply nests everything a level deeper and carries a status
    /// block this does not read. Taken from `vvuk/cassini`.
    const OLDER_REPLY: &str = r#"{
        "Id": "0a69ee780fbd40d7bfb95b312250bf46",
        "Data": {
            "Attributes": {
                "Name": "Saturn3Ultra",
                "MachineName": "ELEGOO Saturn 3 Ultra",
                "ProtocolVersion": "V1.0.0",
                "FirmwareVersion": "V1.4.2",
                "Resolution": "11520x5120",
                "MainboardIP": "192.168.7.128",
                "MainboardID": "ABCD1234ABCD1234",
                "SDCPStatus": 0,
                "Capabilities": ["FILE_TRANSFER", "PRINT_CONTROL"]
            },
            "Status": {
                "CurrentStatus": 0,
                "PrintInfo": { "Status": 16, "CurrentLayer": 310, "TotalLayer": 310 },
                "FileTransferInfo": { "Status": 0 }
            }
        }
    }"#;

    #[test]
    fn a_version_three_board_is_talked_to_over_its_websocket() {
        let printer = parse(REPLY.as_bytes(), sender())
            .expect("the reply is well formed")
            .printer;
        assert_eq!(printer.transport, Transport::WebSocket);
    }

    #[test]
    fn a_version_one_board_is_read_out_of_its_nested_reply() {
        let printer = parse(OLDER_REPLY.as_bytes(), sender())
            .expect("the reply is well formed")
            .printer;
        assert_eq!(printer.model, "ELEGOO Saturn 3 Ultra");
        assert_eq!(printer.mainboard_id, "ABCD1234ABCD1234");
        assert_eq!(printer.address, IpAddr::from([192, 168, 7, 128]));
        assert_eq!(printer.firmware, "V1.4.2");
        assert_eq!(
            printer.transport,
            Transport::Mqtt,
            "the older generation connects out to us"
        );
    }

    #[test]
    fn a_nested_reply_decides_the_transport_even_under_an_unfamiliar_version() {
        let future = OLDER_REPLY.replace(r#""V1.0.0""#, r#""V9.9.9""#);
        let printer = parse(future.as_bytes(), sender())
            .expect("the shape is what is read")
            .printer;
        assert_eq!(printer.transport, Transport::Mqtt);
    }

    #[test]
    fn only_a_version_one_board_says_what_it_is_doing_when_greeted() {
        let older = parse(OLDER_REPLY.as_bytes(), sender()).expect("the reply is well formed");
        let status = older
            .status
            .expect("version 1 nests a status beside its attributes");
        assert_eq!(status.print_info.current_layer, 310);
        let newer = parse(REPLY.as_bytes(), sender()).expect("the reply is well formed");
        assert!(newer.status.is_none());
    }

    #[test]
    fn a_subnet_is_broadcast_to_at_its_own_top_address() {
        let ip = Ipv4Addr::new(192, 168, 1, 107);
        assert_eq!(
            directed_broadcast(ip, Ipv4Addr::new(255, 255, 255, 0)),
            Some(Ipv4Addr::new(192, 168, 1, 255)),
            "a /24 keeps the first three octets"
        );
        assert_eq!(
            directed_broadcast(Ipv4Addr::new(10, 4, 7, 9), Ipv4Addr::new(255, 255, 0, 0)),
            Some(Ipv4Addr::new(10, 4, 255, 255)),
            "a /16 keeps the first two"
        );
        let tunnel = Ipv4Addr::BROADCAST;
        assert_eq!(
            directed_broadcast(ip, tunnel),
            None,
            "a /32 has nobody else"
        );
    }

    #[test]
    fn the_limited_broadcast_is_always_tried_last() {
        assert_eq!(broadcast_addresses().last(), Some(&Ipv4Addr::BROADCAST));
    }

    #[test]
    fn a_reply_that_is_not_json_is_an_error() {
        assert!(matches!(
            parse(b"M99999", sender()),
            Err(SdcpError::Malformed(_))
        ));
    }
}
