# 0161. The readers are fuzzed from a crate outside the workspace

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

Encrust parses a great deal of data nobody here wrote: STL, OBJ and 3MF meshes from model sites,
sliced files from four other slicers, textures from a pack, zip entries inside both. `SECURITY.md`
now states publicly that a malformed file returns a typed error rather than panicking, allocating
without bound or reading out of bounds. Nothing was checking that.

Unit and `proptest` tests cover the shapes we thought of. A reader's failure modes are the shapes we
did not: a layer table whose offsets point past the end, a header claiming four billion layers, a
run-length stream that decodes longer than the panel, a zip entry that expands a thousandfold.

`cargo fuzz` is the tool for that, and it builds with `-Z` flags that require a nightly toolchain.
The workspace pins `stable` in `rust-toolchain.toml`, and the product must stay there.

## Decision

Fuzzing lives in `fuzz/`, a crate with its own lockfile, listed in `[workspace] exclude` so no
workspace command ever reaches it and nightly never leaks into a normal build.

Six targets, one per decoder family, each given the file extension it expects rather than left to
discover a magic number by chance: `mesh`, `sliced_chitu`, `sliced_goo`, `sliced_anycubic`,
`sliced_sl1`, and `sniff` for the case where the name says nothing. A sliced target reads the file's
tables and then up to 256 layers, because the layer decoder is where the arithmetic is.

`fuzz.yml` runs two minutes per target on a weekly schedule and on a change to `fuzz/`, never as a
gate on an unrelated pull request. A crash uploads its input as an artifact, and the fix lands with
that input as a fixture in the owning crate's tests.

## Consequences

The promise in `SECURITY.md` is now checked by something other than optimism, and every crash the
fuzzer finds arrives with a reproducing input, so it converts directly into a regression test.

The cost is a second crate that `cargo test --workspace` does not touch, which means it can rot
unnoticed: a signature change in `core_pipeline::open` or `MeshLoader` breaks the targets and
nothing says so until the weekly run. Dependabot watches `fuzz/` monthly, and a change to the
directory runs the workflow, which is the mitigation rather than a fix. The job also needs
`cargo install cargo-fuzz` on every run, which is most of its runtime.

Revisit if a reader ever stops taking a `ReadSeek` or a path, or if the install cost grows enough to
be worth caching a prebuilt binary.

## Alternatives considered

### A fuzz target inside the workspace

One `cargo test --workspace` would then cover it. Rejected: `cargo fuzz` needs nightly `-Z` flags,
and allowing a nightly toolchain anywhere in the workspace is how a project ends up depending on one.

### `arbitrary`-driven property tests in the existing suite

No new crate, no nightly, runs on every pull request. Rejected as a replacement rather than an
addition: it explores a structured input space, which is the opposite of what is wanted here —
these decoders fail on bytes no generator of valid-ish files would produce.

### Fuzzing on every pull request, and what the chosen shape costs

Rejected because a two-minute session reaches different inputs each time, so a red pull request
would carry no information about the change in it. The cost of the schedule instead is latency: a
crash introduced on Monday is found on Tuesday of the following week, by which time it may be in a
release.
