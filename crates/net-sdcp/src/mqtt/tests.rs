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
