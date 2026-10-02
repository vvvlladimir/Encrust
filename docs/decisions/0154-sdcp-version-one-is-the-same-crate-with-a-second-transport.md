# 0154. SDCP version 1 is the same crate with a second transport

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

ADR 0136 took the older ChiTu Wi-Fi protocol — the Saturn 3 Ultra, the Mars 4 Ultra — to be
a different protocol from SDCP 3.0, and the roadmap therefore called step 19 `net-chitu`.
That reading came from the one thing about it that was known at the time: the roles invert,
because the board connects out to a broker the client runs and fetches the file from a server
the client runs.

`vvuk/cassini` records every packet of it. On the evidence it is SDCP version 1:

- The same `M99999` probe, answered by both generations, with the same fields — nested one
  level deeper, under `Data.Attributes`.
- The board reports `"ProtocolVersion": "V1.0.0"`, against `"V3.0.0"` on a Saturn 4 Ultra.
- The same topics, `sdcp/request|status|attributes|response/${MainboardID}`, differing only
  by a leading slash.
- The same envelope: `Cmd`, `Data`, `From`, `RequestID`, `MainboardID`, `TimeStamp`, `Id`.
- The same command numbers and the same `Ack` table. `128` starts a print in both.

What actually differs is the transport and the direction the file moves: MQTT instead of a
WebSocket, and command `256` pointing at a URL instead of a `POST` carrying packets.

A separate crate would therefore have to duplicate the discovery parser, the envelope, the
request identifiers, `Printer`, `Status`, `PrintInfo`, `Attributes` and the acknowledgement
table — or force a `core-net` for them, since the graph forbids one `net-*` crate depending
on another. Rule 5 of `AGENTS.md` forbids the duplication outright.

There is also a live bug in the reading being corrected: `discover` parses a version 1 reply
into a `Printer` with an empty model and an empty `mainboard_id`, so a Saturn 3 Ultra
currently shows in the window as a blank row.

## Decision

`net-sdcp` covers both versions. `Printer` carries a `Transport`, which discovery derives
from the **shape** of the reply — a nested `Data.Attributes` block means version 1 — rather
than from the version string, so a board reporting an unfamiliar version is still placed
correctly. `Control` holds either a `WebSocket` or a broker behind one `Link`, `upload`
either posts packets or serves the file and sends command `256`, and everything between the
two — envelope, topics, acknowledgements, status — is written once.

This is the same argument ADR 0146 made for `format-chitu`: one vendor family, one crate,
with the version deciding the codec rather than the crate.

ADR 0136 stands. One protocol is still one crate with no shared crate and no sideways edge;
what was wrong was counting this as a second protocol. The roadmap's step 19 is renamed from
`net-chitu` to match.

## Consequences

Version 1 costs about 700 lines — a broker, a file server, the pull half of `upload` and the
second reply shape — and no new public vocabulary beyond `Transport`, `FileTransferInfo` and
`Fetching`. `core-net` is not needed, and neither is the mechanical move of `message.rs` and
`printer.rs` that ADR 0136 said a second protocol would bill for.

Discovery now reports a Saturn 3 Ultra properly, which it did not before, and the window's
printer list gains that generation without an edit: `Target::Sdcp` covers both.

Two things in the crate are now conditional on the transport, and both are marked. The
window skips the file-type pre-flight for a version 1 board, because that generation answers
the question with its status; and `Control::connect` does far more work for version 1 — bind,
invite, wait, three commands — than opening a socket.

The cost is that `net-sdcp` is now two protocols' worth of surface in one crate, and a reader
opening `control.rs` meets both. The signal to split is a third version that shares less than
these two do: if the envelope or the command table ever diverges, the shared half stops being
shared and this becomes duplication with extra steps.

## Alternatives considered

### `net-chitu` beside `net-sdcp`, as the roadmap said

Two crates named for the two machine generations users recognise, and each readable on its
own. It lost on what the two would have to share: the envelope, the ack table, `Printer` and
the discovery parser are the same bytes, and the graph does not let one `net-*` crate use
another's.

### `net-chitu` plus a `core-net` holding the shared vocabulary

The move ADR 0136 predicted, and it keeps each protocol's transport in its own file. It lost
on proportion: a crate at the bottom of the graph, plus moving two modules into it, to
separate a WebSocket from an MQTT socket inside one protocol. The `format-*` crates settled
the same question the other way in ADR 0146 and 0147.

### Keep both, but behind a `Transport` trait rather than an enum

`Link` would be a trait with `send` and `read`, and each transport a type implementing it.
It lost because there are exactly two and they are both in this crate: an enum of two arms
reads better than a trait object, and nothing outside chooses an implementation.

### The option that won, and what it costs

One crate means version 1's oddities are in the middle of version 3's code: `Control::connect`
branches, `Incoming::parse` takes a topic that only one transport supplies, and `Request`
serialises a field only one of them wants. Each is a line or two, but a reader who only cares
about a Saturn 4 Ultra now reads them all.
