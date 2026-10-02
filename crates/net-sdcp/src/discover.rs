use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::SdcpError;
use crate::printer::{Printer, Transport};

/// The port every board listens on for the discovery probe, see docs/formats/sdcp.md.
const DISCOVERY_PORT: u16 = 3000;

/// The probe itself: printers answer this string and nothing else.
const PROBE: &[u8] = b"M99999";

/// A reply can arrive at any time inside the window, so the socket wakes up often.
const POLL: Duration = Duration::from_millis(200);

/// Ask every printer on the local network to introduce itself, for `window`.
///
/// Answers are collected until the window closes, because there is no count to wait for.
pub fn discover(window: Duration) -> Result<Vec<Printer>, SdcpError> {
    let socket = probe_socket()?;
    let broadcast = SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT));
    socket.send_to(PROBE, broadcast)?;
    Ok(collect(&socket, window))
}

/// Ask one printer by address, for when broadcast does not cross the network.
pub fn probe(address: IpAddr, window: Duration) -> Result<Option<Printer>, SdcpError> {
    let socket = probe_socket()?;
    socket.send_to(PROBE, SocketAddr::from((address, DISCOVERY_PORT)))?;
    Ok(collect(&socket, window).into_iter().next())
}

fn probe_socket() -> Result<UdpSocket, SdcpError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(POLL))?;
    Ok(socket)
}

/// One board may answer twice when it holds two addresses, so replies key on its ID.
fn collect(socket: &UdpSocket, window: Duration) -> Vec<Printer> {
    let deadline = Instant::now() + window;
    let mut found: BTreeMap<String, Printer> = BTreeMap::new();
    let mut datagram = [0u8; 4096];
    while Instant::now() < deadline {
        let Ok((read, from)) = socket.recv_from(&mut datagram) else {
            continue;
        };
        match parse(&datagram[..read], from.ip()) {
            Ok(printer) => {
                found.insert(printer.mainboard_id.clone(), printer);
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
pub(crate) fn parse(datagram: &[u8], from: IpAddr) -> Result<Printer, SdcpError> {
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
    Ok(Printer {
        name: greeting.name,
        model: greeting.machine_name,
        brand: greeting.brand_name,
        address,
        mainboard_id: greeting.mainboard_id,
        brand_id: reply.id,
        firmware: greeting.firmware_version,
        protocol: greeting.protocol_version,
        transport,
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
        let printer = parse(REPLY.as_bytes(), sender()).expect("the reply is well formed");
        assert_eq!(printer.model, "Saturn 4 Ultra");
        assert_eq!(printer.mainboard_id, "000000000001d354");
        assert_eq!(printer.address, IpAddr::from([192, 168, 1, 42]));
    }

    #[test]
    fn the_datagram_address_stands_in_for_a_missing_one() {
        let without = REPLY.replace(r#""MainboardIP": "192.168.1.42","#, "");
        let printer = parse(without.as_bytes(), sender()).expect("the address is optional");
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
        let printer = parse(REPLY.as_bytes(), sender()).expect("the reply is well formed");
        assert_eq!(printer.transport, Transport::WebSocket);
    }

    #[test]
    fn a_version_one_board_is_read_out_of_its_nested_reply() {
        let printer = parse(OLDER_REPLY.as_bytes(), sender()).expect("the reply is well formed");
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
        let printer = parse(future.as_bytes(), sender()).expect("the shape is what is read");
        assert_eq!(printer.transport, Transport::Mqtt);
    }

    #[test]
    fn a_reply_that_is_not_json_is_an_error() {
        assert!(matches!(
            parse(b"M99999", sender()),
            Err(SdcpError::Malformed(_))
        ));
    }
}
