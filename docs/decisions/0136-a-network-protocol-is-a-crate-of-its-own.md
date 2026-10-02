# 0136. A network protocol is a crate of its own

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

Step 18 asks for network sending where the protocol is documented. Two are: SDCP 3.0,
published by the mainboard vendor and spoken by Elegoo machines from the
Saturn 4 Ultra onwards, and the older ChiTu Wi-Fi protocol of the Saturn 3 Ultra and Mars
4 Ultra, which has been reverse engineered but not published.

They are not variants of one thing. In SDCP the printer is the server: the client opens a
WebSocket to it and posts the file to its HTTP endpoint. In the older protocol the roles
invert — the printer is told to connect to an MQTT broker the client runs, and to fetch
the file from an HTTP server the client also runs. A slicer speaking the older one has to
be a server twice over, with the lifetime and firewall questions that brings.

Only SDCP is being built now. The second protocol is real and named, but not in hand.

## Decision

Each protocol is its own crate at the top of the graph, named `net-<protocol>`, depending
on nothing else in the workspace: the binaries hand it a path to a written file and it
returns what happened. `net-sdcp` is the first.

There is no shared crate and no `PrinterLink` trait yet. The vocabulary a second protocol
would share — `Printer`, `Status`, `Transfer` — sits in `net-sdcp::printer`, where it can
be lifted into a `core-net` when something else needs it.

## Consequences

The window depends on a concrete client, so adding the ChiTu protocol means an enum or a
trait at the call site in `network.rs`, written when its second arm exists and its shape
is known rather than guessed at now. That edit is bounded: the window calls discover,
upload and start print, and nothing else.

`net-sdcp` is testable without a printer, because everything above the socket — the
discovery reply, the request envelope, the multipart body, every acknowledgement — is a
pure function of bytes.

Reopen this when the second protocol lands. If lifting the vocabulary turns out to mean
rewriting it, the abstraction was wrong to skip and the ADR that replaces this one should
say what the right shape was.

## Alternatives considered

### `core-net` with a `PrinterLink` trait, and `net-sdcp` implementing it

The workspace already pairs `core-format` with `format-goo` and `format-ctb`, so the
shape is familiar and a second protocol would slot in on the day it arrives. It lost
because a trait with one implementor is an abstraction ahead of its use, which rule 2 of
`AGENTS.md` forbids, and because the one implementor would have
defined the trait: the second protocol inverts client and server, and a trait shaped
around SDCP would have had to be rewritten to admit it.

### Sending as a stage inside `core-pipeline`

No new crate, and the file is right there where it is written. It lost to ADR 0127:
`core-pipeline` is the write stage and not a place for everything two callers share. It
would also have put a socket in the crate the CLI uses to write files, which the CLI has
no reason to link.

### The option that won, and what it costs

One crate per protocol means the shared vocabulary is in the first one written, so the
second protocol either depends on the first — which the graph forbids between peers — or
forces the move to `core-net` as its first commit. That move is mechanical, but it is
real work that a `core-net` today would have paid for already.
