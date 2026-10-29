# 0298. Read and write the encrypted `.ctb` in a crate of its own

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

The newer Chitu boards take only an encrypted `.ctb`: the plain container's fields in one
settings block enciphered with AES-256 under a key every file shares, and signed. A user who
owns such a printer could not print from Encrust, and a resin in such a
file could not be imported (ADR 0293). The container is documented by open-source readers
and writers that have shipped for years; the key is in their code. Reading and writing it is
interoperability with a machine the user owns, and is described that way here. The cipher
needs AES and SHA-256, which nothing in the workspace has.

## Decision

`format-ctb-encrypted` reads and writes the container, depending on `core-format` and
`core-raster` like every format crate, and on `aes`, `cbc` and `sha2`. The key is a
constant in its `cipher` module, masked so a plain search of the source does not find it. What it shares with `format-chitu` — the layer cipher and
the notice every version 4 file carries — moves down into `core-format`, as the Chitu codecs
did in ADR 0168. `core-pipeline` opens a `.ctb` with this reader when its magic is
`0x12FD0107`, and writes it for `SlicedFormat::CtbEncrypted`, which a printer profile picks
with `output = "ctb-encrypted"` and an output name alone never does. No shipped profile asks
for it until a machine is confirmed by a print. The layout is in
`docs/formats/ctb-encrypted.md`.

## Consequences

Such a file opens, builds back into a model and gives up its resin. A
profile can write for a printer that takes nothing else. Everything legally sensitive is in
one crate, which can be dropped by one commit if a notice requires it, and a mirror of the
repository off its main host is what keeps it reachable meanwhile. Reopen when a firmware is found that refuses what we write, or
if a notice is received.

## Alternatives considered

### A module in `format-chitu`

The same family and one crate fewer, but the cipher's dependencies would come into every
build of the plain container, and the sensitive part could not be taken out alone.

### The key supplied by the user, or fetched at run time

Neither changes what the program does with the key, which is what the law looks at; a key
fetched to survive a notice reads as intent to evade one. Both break the offline program
and the browser build.

### Our own AES and SHA-256

No dependency, but a few hundred lines of cryptography to keep correct, where the RustCrypto
crates are reviewed and run under wasm.

### The option that won, and what it costs

A key in the source, which is what a notice would point at, and three cryptographic crates in
the build.
