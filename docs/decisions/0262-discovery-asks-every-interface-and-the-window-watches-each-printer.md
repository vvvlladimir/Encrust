# 0262. Ask every interface for printers, and watch each printer the window lists

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

Discovery sent its probe to the limited broadcast, `255.255.255.255`. On macOS that send
fails with `EHOSTUNREACH`; only the subnet's own broadcast, `192.168.1.255` on a `/24`,
leaves the machine. The scan logged the failure at `debug` and showed an empty list, so
*Scan network* and *Add* looked like they did nothing, with a Saturn 4 Ultra on the same
segment answering the subnet's broadcast every time.

The window also showed only whether a machine answered the last scan. `0157` turned down
holding a control socket per machine "for a line of text". Users now want what every
machine on the network is printing, how far it has got and how long is left, and the
board pushes that on the socket anyway.

## Decision

- `net-sdcp::discover` sends the probe to the broadcast address of every IPv4 interface
  off the loopback, worked out from its netmask, and then to the limited broadcast. It
  fails only when no send went out. The interfaces are listed by `if-addrs` 0.15, a desk
  dependency only: a browser has none to list.
- A scan that fails says so. `Found` carries why the broadcast failed and which typed
  addresses stayed silent. A failure with no route to the printer is marked as one, and
  on macOS the window says the Local Network privacy setting may be blocking it.
- `printer_link::watch` reports a printer's state every period until told to stop. It
  holds one control connection to a version 3 board, asks a version 1 board through the
  probe (its answer carries its status) and asks a Prusa machine through its status call.
  It is blocking, as ADR 0137 requires. The window runs one thread per listed printer,
  started after a scan and stopped when the printer leaves the list.
- `State` gains the layer, the time printed and left (`CurrentTicks` and `TotalTicks`, in
  ms by the specification), the temperatures, and whether the machine is busy. A busy
  bound machine greys out *Slice and send*.
- macOS lets a program onto the local network only once it is allowed under Local
  Network, and a binary started from a terminal other than Terminal.app takes that
  terminal's permission, so it is often never asked. The installers' `Info.plist` therefore
  states `NSLocalNetworkUsageDescription`, and `cargo xtask app` builds the window as
  `Encrust Dev.app`: id `app.encrust.dev`, signed ad hoc, so macOS asks about it under its
  own name. The window's advice names whichever case it is running in.
- The list in view is scanned again every 30 s. The Export tool shows the bound machine
  alone when there is one. Otherwise it lists every machine as a card, and a click binds
  the printer profile in hand to that machine.

## Consequences

Discovery works on macOS, and a failed scan explains itself. The reachability dot is live
state rather than the last scan's word, so it no longer goes stale.

Each listed version 3 board holds one of its few control connections while the window
is open; the vendor's boards are reported to take about four. A send opens one more
beside it. Each version 1 board is probed every period. If boards start refusing a
second connection, the watch has to pause while a file goes up.

An ad hoc signature changes with every build, so macOS may ask again after a rebuild of
the development bundle. The installers are not code-signed either (ADR 0160), and a
signed release would make the permission stick across updates.

`TotalTicks` units come from the specification, not from a board seen mid-print. If a
board reports seconds, the time left will read a thousand times too short, and the
conversion moves into `net-sdcp`.

## Alternatives considered

### Work the broadcast out from our own address, assuming a /24

No dependency. It is wrong on any other subnet size, and finding our own address takes a
connected socket trick that picks only the default route's interface.

### `getifaddrs` through `libc`

`libc` is already in the lockfile. It is `unsafe`, and needs a second path for Windows,
which is what `if-addrs` already wraps.

### Reconnecting every period instead of holding the socket

Simpler, and a board sees no idle connection. It costs a handshake every few seconds per
board, and opening and closing connections makes the connection limit more likely to
bite, not less.

### The option that won, and what it costs

A new dependency, a thread and a socket per printer for as long as the window is open,
a repaint every second while anything is watched, and on macOS a development window that
reaches printers only when run as a bundle or from Terminal.app.
