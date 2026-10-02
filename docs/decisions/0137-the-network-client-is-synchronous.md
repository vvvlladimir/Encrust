# 0137. The network client is synchronous

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

`net-sdcp` has to broadcast a UDP datagram and collect replies for a window, hold a
WebSocket open long enough to send a command and read its answer, and POST a large file a
megabyte at a time. The workspace has no async runtime: every long job in the window runs
on a thread of its own and reports over an `mpsc` channel, and everything that scales runs
on `rayon`.

The obvious pull is towards `tokio`, because that is what the mature WebSocket and HTTP
clients assume.

## Decision

`net-sdcp` is synchronous and blocking: `std::net::UdpSocket` for discovery, `tungstenite`
for the control socket, `ureq` for the upload, all with default features off so no TLS
stack is linked. There is nothing to encrypt: the protocol is plain HTTP on a local
network and offers no authentication to protect.

Waiting is bounded by read timeouts and deadlines rather than by an executor, and the
window drives it exactly as it drives slicing: one thread, one channel, one handle polled
each frame.

## Consequences

Adding network sending costs three dependencies and no change to how the window runs.
`SendJob` is the same shape as `SliceJob`, down to cancellation through an `AtomicBool`,
so there is one concurrency model in the application rather than two.

The cost is one thread per errand. That is nothing at this scale — a transfer at a time,
to one printer — and would become a real objection at a print farm's worth of machines
polled continuously. That is the signal to reopen this.

Blocking also means a stalled socket is only unblocked by its timeout: cancelling a
transfer takes effect between packets, not inside one.

## Alternatives considered

### `tokio` with `tokio-tungstenite` and `reqwest`

The standard stack, better at many printers at once, and it would make a live status
stream from every discovered machine easy. It lost because it brings a runtime into a
synchronous workspace: a thread to own the executor, a bridge back to egui, and a second
way of expressing "this is running" alongside the one already there — for a feature whose
whole job is to move one file to one machine.

### Raw `TcpStream`, no new dependencies

Cheapest by the dependency graph. It lost because it makes us the authors of a WebSocket
framer and an HTTP client, including chunked responses, to save two well-used crates.

### The option that won, and what it costs

Synchronous clients are a smaller pool: `tungstenite` and `ureq` are good, but if either
stops being maintained the replacement is more likely to be async than not, and the move
would be the runtime decision made under pressure instead of deliberately.
