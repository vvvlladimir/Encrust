# 0153. A Prusa machine is set up, not discovered

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

An Elegoo board answers a UDP broadcast and needs no credentials, so the printer list fills
itself and the destination menu is a list of what answered (ADR 0138). PrusaLink does
neither. It announces itself over mDNS, which needs a resolver this workspace does not
carry, and every call to it needs either a digest login or an API key.

So the window now has two kinds of printer in one menu: one that appears on its own and one
that has to be described. What describes it — a host, a username, a password, or a key —
has to live somewhere between runs, or nobody will ever send to it twice.

There are three places it could live. A printer profile is on disk under the user's profile
directory, is shared with the CLI, and is a file users copy and send to each other. The
window's own `preferences.json` sits beside that directory and is read by nothing else. The
platform credential store is safest and is three backends for one text field.

`net-sdcp` and `net-prusalink` cannot share a printer type: ADR 0136 puts each protocol in a
crate that depends on nothing else in the workspace, and neither may reach sideways.

## Decision

A Prusa machine is typed into the destination menu — host, then either a username and
password or a single key — and kept in the window's `preferences.json` as a
`net_prusalink::Link`, credentials in the clear. It is not a printer profile.

The two kinds meet in `encrust-app`, in a `Target` enum over `&net_sdcp::Printer` and the
window's own `Prusa`. A `Target` answers three questions — what it is keyed by, what it is
called, what to say under the name — and hands the worker thread a `Wire`, which is the
owned half the errand needs. The destination is remembered by that key, `sdcp:<board id>` or
`prusa:<host>`, so the two can never collide.

A scan asks both: the broadcast for boards, and one `GET /api/version` per Prusa machine, to
fill in what each calls itself.

## Consequences

The enum is in the window, where both protocols are already linked, so neither `net-*` crate
learns about the other and the dependency graph does not change. This is the edit ADR 0136
predicted, and it stayed the size ADR 0136 predicted: the window calls discover, upload and
start print, and nothing else.

A machine the user set up stays in the menu whether it answered the last scan or not — it is
configuration, not a sighting — while a board that stopped answering drops out and takes the
destination with it. `forget_prusa` is therefore a button, because nothing else removes one.

The credentials are plaintext in a file in the user's config directory. The vendor's own
client holds the same field the same way, so this is the level of protection a user of
these machines already has, but it is worth stating rather than implying. Reopen this if the
window ever holds a secret that is not a local printer password.

Because a Prusa machine takes only `.sl1`, the destination now has a say in what the Slice
button is allowed to write: `Network::blocker` greys the button when the container and the
destination disagree. What an Elegoo board takes is still asked of the board itself, on the
errand's thread, because that costs a connection.

## Alternatives considered

### The address and credentials as fields of a printer profile

A machine would be one thing — its panel, its resins and its address together — and the CLI
could grow a send later for free. It lost on the password: profiles are TOML files in a
directory users share, and the catalogue is bundled with the application. It also lost on
responsibility: `printer-profiles` describes what a machine prints like, and a network login
is not that.

### An mDNS resolver so a Prusa machine is discovered too

Both kinds of printer would appear by themselves and the menu would have one shape. It lost
because credentials still have to be typed, so discovery saves a host name and nothing else,
in exchange for a resolver and a multicast socket.

### A `PrinterLink` trait in a new `core-net`, implemented by both crates

The window would call one interface and not know which protocol answered. It lost for now
because the two clients do not have the same shape: one counts bytes and the other cannot,
one discovers and the other is configured, one needs credentials and the other has no
notion of them. A trait over that is three methods and four `Option`s. The `Target` enum
says the same thing in the one place that needs it.

### The OS keychain for the password

The right answer for a secret worth protecting. It lost on proportion: a new dependency with
a backend per platform, and a permission prompt on macOS, for a password that unlocks a
printer on the same LAN.

### The option that won, and what it costs

An enum at the call site means every future protocol edits `network.rs` and `job/send.rs`
rather than adding a file. With two arms that is right; at four it will not be, and the
signal to reopen it is a third arm that has to add a variant to `Wire`, `Target` and
`Failure` before it can do anything.
