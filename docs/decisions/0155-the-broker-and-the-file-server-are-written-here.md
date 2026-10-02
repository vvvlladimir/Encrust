# 0155. The broker and the file server are written here

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

A version 1 board is driven the other way round: it connects to an MQTT broker and fetches
the file from an HTTP server, both of which the slicer has to run (ADR 0154). So `net-sdcp`
needs two servers.

Both have exactly one client, for the length of one transfer:

- The broker sees one connection, from one board, which sends `CONNECT`, one `SUBSCRIBE`,
  publishes on three topics, answers a keep-alive and disconnects. No retained messages, no
  wildcards, no sessions, no QoS 2, no authentication, no other subscriber.
- The file server serves one file at one path, answering a `HEAD` and a `GET`. No ranges, no
  keep-alive, no compression, no other route.

The crates on offer are built for the general case. `rumqttd` is a full broker on `tokio`,
which ADR 0137 rules out: the workspace has no async runtime and every long job runs on a
thread of its own. A synchronous HTTP server crate — `tiny_http` and its like — is a smaller
ask but still a dependency, its own threading model and its own error type, for one route.

## Decision

Both are written in `net-sdcp`: `mqtt.rs` implements MQTT 3.1.1 framing for the packets that
one board sends, and `serve.rs` serves one file on a thread of its own. Neither is exported.
The broker is stepped by the same loop that reads the control connection, so it needs no
thread. The file server takes one, because the board fetches while that loop is pumping
status, and dropping the server stops the thread and takes the file off the network.

Anything the board sends that is not in that list is answered as far as the specification
requires and otherwise ignored.

## Consequences

Two files, about 400 lines together, with no dependency added and nothing async. Every packet
the broker handles is a pure function of bytes, so the framing, the handshake, the
subscription and the publish are all unit-tested without a socket; the whole version 1
sequence is then driven end to end in `tests/version_one.rs` by a fake board that speaks MQTT
back at us.

What is implemented is what one firmware does, not what the specification says. A board that
sends a `PUBREL`, a wildcard subscription or a QoS 2 message is not served, and the failure
would look like a transfer that stalls rather than an error that names the packet. The signal
to reopen this is a second firmware that does something in that list; the answer then is to
extend `mqtt.rs`, not to take on a broker.

The file server has no authentication, because the protocol gives it nowhere to put any: the
board is handed a URL and fetches it. The route is a 32-character random name so that a URL
from an earlier transfer fetches nothing, and the server only exists for the length of one
transfer, but anything on the segment that sees the URL can read the file while it is up.
Sending over SDCP is unauthenticated in both directions and in both versions.

## Alternatives considered

### `rumqttd` for the broker

A real broker, maintained, and a great deal more correct than this. It lost to ADR 0137: it
is built on `tokio`, and bringing an async runtime into the workspace for one board on one
connection would be the largest dependency in the graph by far.

### A synchronous HTTP server crate for the file half

Smaller than the broker question, and serving a file is a solved problem. It lost because
what is needed is a `TcpListener`, a request line and a `Content-Length` — about 60 lines —
while a crate brings a router, a threading model and an error type to convert. The MQTT half
had to be hand-written either way, so this would have mixed the two approaches for no gain.

### One thread for both servers, or none

The broker could have run on its own thread with a channel to the control loop, which is what
`cassini` does with its event loop. It lost because the control loop already blocks on reads
with a short timeout: stepping the broker from it costs nothing and keeps every report in one
place. The file server cannot join it, because the board fetches while that loop is reading.

### The option that won, and what it costs

Two network servers in a slicer is two attack surfaces that did not exist before, both
unauthenticated, both on ports chosen by the operating system. They are up only while a
transfer runs, and the route is unguessable, but this is the first code in the workspace that
accepts an inbound connection at all. A user on a network they do not trust should send over
USB.
