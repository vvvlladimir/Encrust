//! The MQTT broker a version 1 board connects back to.
//!
//! Only what that one client does is implemented: connect, subscribe to its own request
//! topic, publish on three others, and answer a keep-alive. Packet layout is MQTT 3.1.1;
//! see `docs/formats/sdcp.md` for why this is here at all.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use crate::error::SdcpError;

const CONNECT: u8 = 1;
const CONNACK: u8 = 2;
const PUBLISH: u8 = 3;
const PUBACK: u8 = 4;
const SUBSCRIBE: u8 = 8;
const SUBACK: u8 = 9;
const PINGREQ: u8 = 12;
const PINGRESP: u8 = 13;
const DISCONNECT: u8 = 14;

/// How long a read blocks before the caller's deadline is checked again.
const POLL: Duration = Duration::from_millis(100);

/// A board sends a few hundred bytes at a time; this is a whole status message over.
const READ_CHUNK: usize = 8 * 1024;

/// A malformed remaining-length field would otherwise be read forever.
const MAX_LENGTH_BYTES: u32 = 4;

/// A socket bound and waiting, before the board has been told where to find it.
///
/// The port has to be known before the board is asked to connect, and the board only
/// connects once, so binding and accepting are two steps rather than one.
pub(crate) struct Waiting {
    listener: TcpListener,
}

impl Waiting {
    /// Binds a broker on a port the operating system picks.
    pub(crate) fn bind() -> Result<Self, SdcpError> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener })
    }

    pub(crate) fn port(&self) -> Result<u16, SdcpError> {
        Ok(self.listener.local_addr()?.port())
    }

    /// Takes the one connection this broker is for.
    pub(crate) fn accept(self, window: Duration) -> Result<Broker, SdcpError> {
        let deadline = Instant::now() + window;
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(POLL))?;
                    stream.set_nodelay(true)?;
                    return Ok(Broker {
                        stream,
                        pending: Vec::new(),
                        subscribed: Vec::new(),
                        client_id: None,
                        closed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(SdcpError::TimedOut {
                            what: "the board's connection to our broker",
                            seconds: window.as_secs(),
                        });
                    }
                    std::thread::sleep(POLL);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

/// What one step of the connection came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// The board published this payload on this topic.
    Message(String, String),
    /// A packet was answered and there is nothing for the caller in it.
    Handled,
    /// Nothing arrived before the deadline, or the board hung up.
    Idle,
}

/// The broker, once the board is on it. Dropping it drops the connection.
pub(crate) struct Broker {
    stream: TcpStream,
    /// Bytes read but not yet a whole packet.
    pending: Vec<u8>,
    subscribed: Vec<String>,
    client_id: Option<String>,
    closed: bool,
}

impl Broker {
    /// The client identifier the board connected under, which is its mainboard id.
    pub(crate) fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    /// Whether the board has hung up. A step then reports `Idle` at once rather than
    /// waiting out the deadline, so a caller must look at this to tell the two apart.
    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }

    /// Whether the board has subscribed to `topic` and can therefore be published to.
    pub(crate) fn has_subscribed(&self, topic: &str) -> bool {
        self.subscribed.iter().any(|known| known == topic)
    }

    /// Sends one message at `QoS` 0.
    ///
    /// `cassini` puts a packet identifier in the body of a `QoS` 0 publish, which the
    /// specification does not allow and which prefixes the payload with two stray bytes.
    /// This does not.
    pub(crate) fn publish(&mut self, topic: &str, payload: &str) -> Result<(), SdcpError> {
        let mut body = Vec::with_capacity(topic.len() + payload.len() + 2);
        put_string(&mut body, topic);
        body.extend_from_slice(payload.as_bytes());
        self.send(PUBLISH, 0, &body)
    }

    /// Moves the connection on by one packet.
    ///
    /// Everything that is not a publish — the handshake, a subscription, a keep-alive — is
    /// answered here and reported as `Handled`, so a caller waiting on the handshake can
    /// look at what changed between steps.
    pub(crate) fn step(&mut self, deadline: Instant) -> Result<Step, SdcpError> {
        loop {
            if let Some((kind, flags, body)) = self.take_packet() {
                return Ok(match self.handle(kind, flags, &body)? {
                    Some((topic, payload)) => Step::Message(topic, payload),
                    None => Step::Handled,
                });
            }
            if self.closed {
                return Ok(Step::Idle);
            }
            if !self.fill()? && Instant::now() >= deadline {
                return Ok(Step::Idle);
            }
        }
    }

    /// Reads whatever has arrived. Answers whether any bytes did.
    fn fill(&mut self) -> Result<bool, SdcpError> {
        let mut chunk = [0u8; READ_CHUNK];
        match self.stream.read(&mut chunk) {
            Ok(0) => {
                self.closed = true;
                Ok(false)
            }
            Ok(read) => {
                self.pending.extend_from_slice(&chunk[..read]);
                Ok(true)
            }
            Err(error) if would_block(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Splits one whole packet off the front of what has been read.
    fn take_packet(&mut self) -> Option<(u8, u8, Vec<u8>)> {
        if self.pending.len() < 2 {
            return None;
        }
        let (length, header) = remaining_length(&self.pending[1..])?;
        let header = header + 1;
        if self.pending.len() < header + length {
            return None;
        }
        let kind = self.pending[0] >> 4;
        let flags = self.pending[0] & 0x0f;
        let body = self.pending[header..header + length].to_vec();
        self.pending.drain(..header + length);
        Some((kind, flags, body))
    }

    /// Answers one packet, and reports a publish for the caller to act on.
    fn handle(
        &mut self,
        kind: u8,
        flags: u8,
        body: &[u8],
    ) -> Result<Option<(String, String)>, SdcpError> {
        match kind {
            CONNECT => {
                self.client_id = Some(client_id(body)?);
                // Session present 0, return code 0: accepted.
                self.send(CONNACK, 0, &[0x00, 0x00])?;
                Ok(None)
            }
            SUBSCRIBE => {
                let (packet_id, topics) = subscriptions(body)?;
                let granted: Vec<u8> = topics.iter().map(|(_, qos)| *qos).collect();
                for (topic, _) in topics {
                    if !self.has_subscribed(&topic) {
                        self.subscribed.push(topic);
                    }
                }
                let mut reply = packet_id.to_be_bytes().to_vec();
                reply.extend_from_slice(&granted);
                self.send(SUBACK, 0, &reply)?;
                Ok(None)
            }
            PUBLISH => {
                let qos = (flags >> 1) & 0x03;
                let (topic, packet_id, payload) = published(body, qos)?;
                if let Some(packet_id) = packet_id {
                    self.send(PUBACK, 0, &packet_id.to_be_bytes())?;
                }
                Ok(Some((topic, payload)))
            }
            PINGREQ => {
                self.send(PINGRESP, 0, &[])?;
                Ok(None)
            }
            DISCONNECT => {
                self.closed = true;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn send(&mut self, kind: u8, flags: u8, body: &[u8]) -> Result<(), SdcpError> {
        let mut packet = Vec::with_capacity(body.len() + 5);
        packet.push(kind << 4 | flags);
        put_remaining_length(&mut packet, body.len());
        packet.extend_from_slice(body);
        self.stream.write_all(&packet)?;
        self.stream.flush()?;
        Ok(())
    }
}

/// The remaining length and how many bytes it took, or `None` when it is not all here yet.
fn remaining_length(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut length = 0usize;
    let mut multiplier = 1usize;
    for (read, byte) in bytes.iter().enumerate() {
        length += usize::from(byte & 0x7f) * multiplier;
        if byte & 0x80 == 0 {
            return Some((length, read + 1));
        }
        multiplier *= 128;
        if read as u32 + 1 >= MAX_LENGTH_BYTES {
            return Some((length, read + 1));
        }
    }
    None
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

fn put_string(body: &mut Vec<u8>, text: &str) {
    let length = u16::try_from(text.len()).unwrap_or(u16::MAX);
    body.extend_from_slice(&length.to_be_bytes());
    body.extend_from_slice(&text.as_bytes()[..usize::from(length)]);
}

/// Reads a length-prefixed string, answering it and what follows it.
fn take_string(body: &[u8]) -> Result<(String, &[u8]), SdcpError> {
    let Some(header) = body.get(..2) else {
        return Err(malformed("a string with no length"));
    };
    let length = usize::from(u16::from_be_bytes([header[0], header[1]]));
    let Some(text) = body.get(2..2 + length) else {
        return Err(malformed("a string shorter than its length"));
    };
    let text = String::from_utf8_lossy(text).into_owned();
    Ok((text, &body[2 + length..]))
}

/// The client identifier out of a CONNECT.
///
/// The variable header is the protocol name, its level, the connect flags and the
/// keep-alive; the identifier is the first field of the payload after them.
fn client_id(body: &[u8]) -> Result<String, SdcpError> {
    let (name, rest) = take_string(body)?;
    if name != "MQTT" && name != "MQIsdp" {
        return Err(malformed("a connection that is not MQTT"));
    }
    let Some(rest) = rest.get(4..) else {
        return Err(malformed("a connection with no header"));
    };
    let (id, _) = take_string(rest)?;
    Ok(id)
}

/// The packet identifier and every topic of a SUBSCRIBE, each with the `QoS` asked for.
fn subscriptions(body: &[u8]) -> Result<(u16, Vec<(String, u8)>), SdcpError> {
    let Some(header) = body.get(..2) else {
        return Err(malformed("a subscription with no packet identifier"));
    };
    let packet_id = u16::from_be_bytes([header[0], header[1]]);
    let mut rest = &body[2..];
    let mut topics = Vec::new();
    while !rest.is_empty() {
        let (topic, after) = take_string(rest)?;
        let Some((qos, after)) = after.split_first() else {
            return Err(malformed("a subscription with no QoS"));
        };
        topics.push((topic, *qos & 0x03));
        rest = after;
    }
    if topics.is_empty() {
        return Err(malformed("a subscription to nothing"));
    }
    Ok((packet_id, topics))
}

/// The topic, the packet identifier when the `QoS` calls for one, and the payload.
fn published(body: &[u8], qos: u8) -> Result<(String, Option<u16>, String), SdcpError> {
    let (topic, rest) = take_string(body)?;
    if qos == 0 {
        return Ok((topic, None, String::from_utf8_lossy(rest).into_owned()));
    }
    let Some(header) = rest.get(..2) else {
        return Err(malformed("a message with no packet identifier"));
    };
    let packet_id = u16::from_be_bytes([header[0], header[1]]);
    let payload = String::from_utf8_lossy(&rest[2..]).into_owned();
    Ok((topic, Some(packet_id), payload))
}

fn malformed(what: &'static str) -> SdcpError {
    SdcpError::BadPacket { what }
}

fn would_block(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A broker with a socket pair behind it, so the packets can be driven by hand.
    fn pair() -> (Broker, TcpStream) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a local port is free");
        let address = listener.local_addr().expect("the socket is bound");
        let client = TcpStream::connect(address).expect("the listener is up");
        let (server, _) = listener.accept().expect("the client connected");
        server
            .set_read_timeout(Some(POLL))
            .expect("the socket takes a timeout");
        client
            .set_read_timeout(Some(POLL))
            .expect("the socket takes a timeout");
        (
            Broker {
                stream: server,
                pending: Vec::new(),
                subscribed: Vec::new(),
                client_id: None,
                closed: false,
            },
            client,
        )
    }

    fn packet(kind: u8, flags: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![kind << 4 | flags];
        put_remaining_length(&mut out, body.len());
        out.extend_from_slice(body);
        out
    }

    fn connect_body(id: &str) -> Vec<u8> {
        let mut body = Vec::new();
        put_string(&mut body, "MQTT");
        body.extend_from_slice(&[0x04, 0x02, 0x00, 0x3c]);
        put_string(&mut body, id);
        body
    }

    fn subscribe_body(packet_id: u16, topic: &str) -> Vec<u8> {
        let mut body = packet_id.to_be_bytes().to_vec();
        put_string(&mut body, topic);
        body.push(0);
        body
    }

    fn publish_body(topic: &str, payload: &str) -> Vec<u8> {
        let mut body = Vec::new();
        put_string(&mut body, topic);
        body.extend_from_slice(payload.as_bytes());
        body
    }

    fn soon() -> Instant {
        Instant::now() + Duration::from_secs(2)
    }

    #[test]
    fn a_length_of_two_bytes_round_trips() {
        for length in [0usize, 1, 127, 128, 16_383, 16_384, 2_097_151] {
            let mut encoded = Vec::new();
            put_remaining_length(&mut encoded, length);
            assert_eq!(
                remaining_length(&encoded),
                Some((length, encoded.len())),
                "length {length}"
            );
        }
    }

    #[test]
    fn a_length_that_has_not_all_arrived_is_not_a_length() {
        assert_eq!(remaining_length(&[0x80]), None);
        assert_eq!(remaining_length(&[]), None);
    }

    #[test]
    fn a_connection_is_acknowledged_and_names_the_board() {
        let (mut broker, mut client) = pair();
        client
            .write_all(&packet(CONNECT, 0, &connect_body("ABCD1234ABCD1234")))
            .expect("the broker is reading");
        assert_eq!(
            broker.step(soon()).expect("the handshake is not an error"),
            Step::Handled,
            "a connection is not a message for the caller"
        );
        assert_eq!(broker.client_id(), Some("ABCD1234ABCD1234"));

        let mut reply = [0u8; 4];
        client
            .read_exact(&mut reply)
            .expect("the broker answered the connection");
        assert_eq!(reply, [CONNACK << 4, 0x02, 0x00, 0x00]);
    }

    #[test]
    fn a_subscription_is_granted_at_the_quality_it_asked_for() {
        let (mut broker, mut client) = pair();
        client
            .write_all(&packet(
                SUBSCRIBE,
                0x02,
                &subscribe_body(7, "/sdcp/request/ff"),
            ))
            .expect("the broker is reading");
        assert_eq!(broker.step(soon()).expect("not an error"), Step::Handled);
        assert!(broker.has_subscribed("/sdcp/request/ff"));
        assert!(!broker.has_subscribed("/sdcp/request/other"));

        let mut reply = [0u8; 5];
        client
            .read_exact(&mut reply)
            .expect("the broker answered the subscription");
        assert_eq!(reply, [SUBACK << 4, 0x03, 0x00, 0x07, 0x00]);
    }

    #[test]
    fn a_message_the_board_publishes_reaches_the_caller_with_its_topic() {
        let (mut broker, mut client) = pair();
        client
            .write_all(&packet(
                PUBLISH,
                0,
                &publish_body("/sdcp/status/ff", r#"{"Id":"x"}"#),
            ))
            .expect("the broker is reading");
        assert_eq!(
            broker.step(soon()).expect("not an error"),
            Step::Message("/sdcp/status/ff".to_owned(), r#"{"Id":"x"}"#.to_owned())
        );
    }

    #[test]
    fn a_message_sent_at_quality_one_is_acknowledged() {
        let (mut broker, mut client) = pair();
        let mut body = Vec::new();
        put_string(&mut body, "/sdcp/status/ff");
        body.extend_from_slice(&9u16.to_be_bytes());
        body.extend_from_slice(b"{}");
        client
            .write_all(&packet(PUBLISH, 0x02, &body))
            .expect("the broker is reading");
        assert_eq!(
            broker.step(soon()).expect("not an error"),
            Step::Message("/sdcp/status/ff".to_owned(), "{}".to_owned())
        );
        let mut reply = [0u8; 4];
        client.read_exact(&mut reply).expect("a PUBACK came back");
        assert_eq!(reply, [PUBACK << 4, 0x02, 0x00, 0x09]);
    }

    #[test]
    fn two_packets_in_one_read_are_both_taken() {
        let (mut broker, mut client) = pair();
        let mut both = packet(CONNECT, 0, &connect_body("ff"));
        both.extend_from_slice(&packet(PUBLISH, 0, &publish_body("/sdcp/status/ff", "{}")));
        client.write_all(&both).expect("the broker is reading");
        assert_eq!(
            broker.step(soon()).expect("not an error"),
            Step::Handled,
            "the connection is answered first"
        );
        assert_eq!(
            broker.step(soon()).expect("not an error"),
            Step::Message("/sdcp/status/ff".to_owned(), "{}".to_owned()),
            "and the message that followed it in the same read is still there"
        );
    }

    #[test]
    fn a_keep_alive_is_answered_without_troubling_the_caller() {
        let (mut broker, mut client) = pair();
        client
            .write_all(&packet(PINGREQ, 0, &[]))
            .expect("the broker is reading");
        assert_eq!(broker.step(soon()).expect("not an error"), Step::Handled);
        let mut reply = [0u8; 2];
        client.read_exact(&mut reply).expect("a PINGRESP came back");
        assert_eq!(reply, [PINGRESP << 4, 0x00]);
    }

    #[test]
    fn a_published_message_carries_no_packet_identifier_at_quality_zero() {
        let (mut broker, mut client) = pair();
        broker
            .publish("/sdcp/request/ff", r#"{"Cmd":0}"#)
            .expect("the client is reading");
        let mut head = [0u8; 2];
        client.read_exact(&mut head).expect("a publish came over");
        assert_eq!(
            head[0],
            PUBLISH << 4,
            "QoS 0, not retained, not a duplicate"
        );
        let mut body = vec![0u8; usize::from(head[1])];
        client.read_exact(&mut body).expect("the body follows");
        let (topic, payload) = take_string(&body).expect("the topic is length prefixed");
        assert_eq!(topic, "/sdcp/request/ff");
        assert_eq!(payload, br#"{"Cmd":0}"#);
    }

    #[test]
    fn a_connection_that_is_not_mqtt_is_refused_rather_than_read() {
        let mut body = Vec::new();
        put_string(&mut body, "HTTP");
        assert!(matches!(client_id(&body), Err(SdcpError::BadPacket { .. })));
    }

    #[test]
    fn a_truncated_string_is_a_bad_packet_rather_than_a_panic() {
        assert!(matches!(
            take_string(&[0x00, 0x08, b'a']),
            Err(SdcpError::BadPacket { .. })
        ));
        assert!(matches!(
            take_string(&[0x00]),
            Err(SdcpError::BadPacket { .. })
        ));
    }

    #[test]
    fn a_board_that_hangs_up_stops_the_reader() {
        let (mut broker, client) = pair();
        drop(client);
        assert_eq!(
            broker
                .step(soon())
                .expect("a closed socket is not an error"),
            Step::Idle
        );
    }

    #[test]
    fn nothing_connecting_inside_the_window_times_out() {
        let waiting = Waiting::bind().expect("a local port is free");
        assert!(waiting.port().expect("the socket is bound") > 0);
        assert!(matches!(
            waiting.accept(Duration::from_millis(150)),
            Err(SdcpError::TimedOut { .. })
        ));
    }
}
