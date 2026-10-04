# 0182. Sending is a crate both front ends share, and a browser downloads

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

ADR 0153 put the choice between the two protocols in the window: a `Wire` enum over a
`net_sdcp::Printer` and a `net_prusalink::Link`, with the pre-flight a version 3 board gets,
the scan that asks both, and an error flattened from either. The command line now needs the
same — `encrust printer discover`, `status` and `send --start` — and rule 5 forbids a second
copy. ADR 0136 forbids a `net-*` crate depending on another, and `core-engine` is neither the
place for what two callers share (ADR 0174) nor able to link sockets, since it builds for a
browser (ADR 0177).

A browser cannot run that code at all. Discovery is a UDP broadcast and version 1 needs a
broker and a file server, none of which a page may open. Chromium 154 lets a page open a
WebSocket to a private address after a Local Network Access prompt, but the upload is a
`POST` whose answer a page reads only if the board sends CORS headers, which no source
documents. A Prusa machine sends none, and its login is a header that needs a preflight.

## Decision

`printer-link` is a crate above `net-sdcp` and `net-prusalink`, depending on them and nothing
else in the workspace. It holds `Wire`, `upload` with the version 3 pre-flight, `start_print`,
`state` — a `State` either protocol's status is read into — `scan` over both and
`SendError`. The window and the command line call it; each keeps only what is its own, the
menu, the bindings and the remembered machines in the window, flags in the CLI.

On the command line a board is named by its IP address and asked who it is; a Prusa machine
by `--prusalink` and its host, its key or password from a flag or from
`ENCRUST_PRUSALINK_KEY`/`ENCRUST_PRUSALINK_PASSWORD`, which turns on clap's `env` feature.

In a browser the sliced file is downloaded. The Network card says why, and the Send button
stays hidden.

## Consequences

The enum stays exactly the size ADR 0153 drew, now one crate down; a third protocol adds a
variant there rather than in two front ends. `net-*` crates still depend on nothing, and the
graph gains `printer-link` under both binaries. `net-prusalink` gains `status`, which the
window does not call yet.

The web build reaches no printer. Reopen it with a version 3 board in hand: if the board
answers the upload with CORS headers, a browser transport for `net-sdcp` — WebSocket and
`fetch` through `web-sys`, IP typed in, Chromium only — is worth writing, as a `cfg`
module of that crate, since the envelope and the acknowledgements are the same bytes.

## Alternatives considered

### The enum in `core-engine`, as the plan had it

One crate fewer, but `core-engine` would link `ureq` and `tungstenite` into the browser build
and take on a job that is not running a plate.

### A second match in the command line

Two arms, about sixty lines, and nothing new to name. It lost to rule 5: the pre-flight, the
error flattening and the scan would drift apart in two copies.

### Sending from the page now, over a WebSocket and `fetch`

Possible in Chromium 154 for the control socket. It lost on what cannot be checked: without
CORS on the board the upload's answer is unreadable, and splitting `net-sdcp` into a protocol
and two transports is the larger part of the work.

### The option that won, and what it costs

A crate that is only a dispatch over two others, and a web build that tells the user to
carry a file over by hand. Credentials typed on a command line land in shell history; the
variables are the documented way, and nothing enforces them.
