# Security

## Reporting a vulnerability

Please do **not** open a public issue. Use GitHub's
[private vulnerability reporting](https://github.com/vvvlladimir/Encrust/security/advisories/new) on
this repository. You will get an acknowledgement within a few days; this is a spare-time project, so
please allow reasonable time for a fix before disclosing.

## What the application does

Encrust runs entirely on the user's machine. There is no account, no server, no telemetry and no
licence check. It opens files the user chooses, writes files the user names, and makes three kinds of
network connection, each only on request: a UDP broadcast on the local network to look for printers,
an upload of a finished file to a printer address the user gave it, and — once a day if the user
turned it on in Settings › Updates, or on "Check now" — a request to GitHub for the newest release,
followed by a download only when the user presses "Install and restart".

That shape decides the attack surface. Encrust is not a service and holds no secrets worth stealing;
what it does do is parse a great deal of untrusted binary data.

## In scope

- **Memory safety or a panic reachable from a file.** Every mesh loader (STL, OBJ, 3MF), every
  sliced-file reader (`.goo`, `.ctb`, `.cbddlp`, `.photon`, Anycubic `.pw*`, `.sl1`), the PNG and
  texture decoders and the run-length decoders all read data an attacker may have written. An
  out-of-bounds read, an integer overflow that becomes one, or an unwind out of library code is a
  bug worth reporting.
- **Resource exhaustion from a small file.** A header that claims four billion layers, a zip entry
  that expands to a terabyte, a 3MF with a cyclic reference. A malicious file should be refused,
  not allowed to fill memory or disk.
- **Anything a file can make Encrust write outside the path the user chose** — a zip entry or a
  referenced resource escaping the output directory through `..` or an absolute path.
- **Anything that makes Encrust talk to an address the user did not give it**, or that lets a file
  or a printer's reply cause a connection anywhere else.
- **Parsing of a reply from the network.** A printer on the local network is not trusted input
  either: SDCP and PrusaLink responses are JSON from a device somebody else may be impersonating.
- **Anything that makes the window install a binary the project did not sign**, or a signed one
  for another version or platform than the one it offered: a feed or archive that slips past the
  checks in [`docs/design/updates.md`](docs/design/updates.md), or an update the window applies
  without the user pressing install.

## Out of scope

- An attacker who already runs code as the user, or has administrator rights on the machine.
- A malicious printer profile the user installed deliberately into their own profile directory.
  Profiles are plain TOML with no executable content, but their values are trusted once installed.
- The absence of code signing on release builds (see below).
- A print that fails, warps or damages hardware because of settings. Encrust cannot validate an
  exposure time against physical reality; that is what [the disclaimer in the
  README](README.md#disclaimer) is for.

## Hardening that is in place

- No `unsafe` anywhere in the `core-*` or `format-*` crates, and no `unwrap()` or `expect()` on a
  path that reads a file. The few remaining `expect`s are in build scripts, `const` evaluation and
  test fixtures.
- Every reader has a fuzz target in [`fuzz/`](fuzz/), run weekly in CI: ten of them, covering the
  mesh loaders and texture decoding, every sliced-file container with its run-length decoder, and
  the sniffer that picks between them. A crash becomes a fixture and a test.
- **No buffer is sized from a number a file states.** A layer count is checked against the bytes
  the file actually holds before a row of it is reserved, a panel is checked against a ceiling no
  printer reaches, and an archive entry is grown rather than reserved from the size its directory
  claims. The three guards and what they do not cover are in
  [`docs/design/hostile-files.md`](docs/design/hostile-files.md).
- Dependencies are checked by `cargo deny` on every change to the lockfile: known advisories,
  licences the project cannot take, and crates from anywhere but crates.io all fail the build.
- The network crates carry no TLS stack and no general HTTP client beyond what the local protocols
  need, which keeps that surface small. TLS is turned on in the window alone, for the update check.
- **An update is installed only if it is signed with the project's release key**, whose public
  half is compiled in, and only if the signature names the version and the archive the window
  asked for. Whoever controls the feed or the connection can withhold an update, not replace one.

## Releases

Release builds are not code-signed yet, so macOS and Windows will warn that the application comes
from an unidentified developer. Signing is on the list. Until then, building from source is the way
to avoid trusting a binary — which is part of why the code is here.

Every release archive does carry a minisign signature beside it (`<archive>.minisig`), made with the
key in [`assets/update-key.pub`](assets/update-key.pub). It is what the window's update checks, and
anyone can check a download by hand:

```sh
minisign -Vm encrust-linux-x86_64.tar.gz -p assets/update-key.pub
```
