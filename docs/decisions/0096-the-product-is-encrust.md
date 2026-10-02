# 0096. Name the product Encrust, and its project file `.encrust`

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

`RustSlicer` named the implementation language, not the tool. The design layer has been
called Encrust since ADR 0024, so the window, the tokens and the mockup already carried
one name while the binary, the crates and the written `.goo` header carried another.
Step 14 is about to put a project file on the user's disk and a name inside it, and an
extension chosen now is one we would have to keep reading forever.

Three of the old name's uses are not cosmetic: the user's profile directory
(`<config>/RustSlicer/profiles`), the `ENCRUST_PROFILE_DIR` override that used to read
`RUSTSLICER_PROFILE_DIR`, and the 32-byte software field `format-goo` writes into every
sliced file.

## Decision

The product is Encrust. The window binary is `encrust`, the front-end crates are
`encrust-app` and `encrust-cli`, and the `.goo` software field reads `Encrust`. The
`core-*`, `format-*` and `printer-profiles` crates are named for their work and do not
change.

A project file is `.encrust`. The extension is the product name, as every other slicer's
project extension is, so a file on disk names the tool that opens it with no lookup table.

The user's profile directory moves to `<config>/Encrust/profiles` and the old one is not
read. At version 0.0.1 nobody has a catalogue worth a migration path.

## Consequences

A printed `.goo` from this commit differs from an earlier one by the software field; any
byte comparison against a file written before it fails, correctly. An installed user's
saved printer and resin profiles stop being found and have to be moved by hand — the
release notes have to say so, and this is the last release where saying so is enough.

The repository and its clone URL still read `RustSlicer`; renaming those is a GitHub
operation, not a code change, and until it happens `Cargo.toml` and `README.md` keep the
working URL.

## Alternatives considered

### `.rsproj`, as the roadmap first wrote it

Chosen when the product was RustSlicer, where `rs` read as Rust. Under the new name it
names nothing, and a generic `proj` suffix invites a collision.

### A short three-letter extension beside `.goo` and `.ctb`

`.ecr` is taken by AWS and by Ecere, `.enc` reads as encrypted. Neither buys anything: a
project file is opened by a human in a file manager, not parsed by a printer's firmware,
so the four saved characters have no reader.

### Read the old profile directory as a fallback

A dozen lines in `catalogue.rs` and a test, forever, so that a pre-0.1 user with a
hand-written profile does not have to move a file once. The cost is permanent and the
benefit expires.

### The option that won, and what it costs

`.encrust` is seven characters, longer than every other extension this workspace touches,
and it will look verbose beside `model.goo` in the same directory. Breaking the profile
directory also spends the one free rename this project gets: doing it a second time after
1.0 would need the migration code we are declining to write now.
