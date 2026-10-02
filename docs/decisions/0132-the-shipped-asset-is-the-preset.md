# 0132. The shipped TOML is the preset, and the constructor mirrors it

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

A shipped support preset exists twice: as `assets/profiles/supports/<name>.toml`, which
the catalogue loads and the Supports window offers, and as `SupportProfile::light()`,
`medium()` and `heavy()`, which `encrust-cli --support` builds from. ADR 0131 retuned both
and left them disagreeing: Medium branched at 45 degrees over 8 mm in the asset and at 70
over 12 in the constructor, and braced in the constructor while the asset left bracing off;
Light disagreed on the branch angle, the bracing numbers and the pad diameter. The same
preset name therefore stood a plate up differently in the window than on the command line.

Heavy disagreed the other way: the asset had been given the reference 1.0 density and 45
degree overhang, the two numbers ADR 0131 keeps ours at 0.6/1.0/2.5 and 55/45/35 precisely
so that the three presets do not place supports identically.

## Decision

**The asset is the preset.** Where the TOML and the constructor disagree on a measurement,
the TOML wins and the constructor is edited to match: the constructors exist so a binary
can name a preset without a catalogue on disk, not as a second set of numbers.

`density` and `max_overhang_deg` are the exception, because they are ours rather than
the reference's and ADR 0131 states them: Heavy's asset goes back to 2.5 and 35 degrees.

`every_shipped_support_preset_matches_the_built_in_one` in `printer-profiles` pins the
parity over all three presets, so the next edit to one side fails until the other follows.

## Consequences

Retuning a shipped preset means editing the TOML and then the constructor, and a
`cargo test -p printer-profiles` says whether the pair is done. Medium ships with bracing
off and a 45 degree branch, which is lighter than what ADR 0131's consequences describe.

Reopen this when a preset's numbers are generated rather than written — a constructor that
reads the embedded TOML would make the parity test unnecessary, at the cost of making
`SupportProfile::medium()` fallible.

## Alternatives considered

### Make the constructor authoritative and rewrite the assets

The Rust is where validation and `keep_in_order` live, so the numbers are read there more
often. Rejected because a user edits a preset by copying the shipped TOML, so the file is
the one an outside reader treats as the truth.

### Delete the constructors and read the embedded asset

One source, no parity test. Rejected because every preset call site would grow a `Result`
for a file that is compiled in and already validated by a test.

### The option that won, and what it costs

Two files hold the same numbers and a test is all that keeps them equal. An edit to only
one side is caught late, at `cargo test` rather than at `cargo build`.
