# 0359. Keep the printer catalogue by hand, checked against the vendor's own profiles

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

ADR 0165 had `cargo xtask gen-profiles` write 95 of the 102 catalogue files from a public
set of `.ini` profiles, and rewrite any file carrying its marker line. That source names no
machine string, so ADR 0140's `machine_name` stayed unset on all 95, and it was wrong in
places: one machine with a panel of 1620 × 2560 px on 192 × 120 mm, one written to `.goo`
for a firmware that reads only `.prz`.

The vendor's own slicer ships a profile for 84 of our machines, with the panel, the travel,
the mirroring, the output container and the string it writes into a file. Carrying those
facts into a generated file would be undone by the next run.

## Decision

Every file under `assets/profiles/printers/` is kept by hand; none carries a generated
marker. A machine the vendor's own slicer has a profile for takes its `machine_name` from
it, and its panel, travel and mirroring where nothing better is known. A machine whose only
known container is one we do not write is not shipped. The generator is deleted, and the
files carry no `UNVERIFIED` header: none of the catalogue is a measurement, and saying so in
every file said nothing a reader of `docs/design/profiles.md` does not already know.

## Consequences

`machine_name` is now set on every machine with a vendor profile, so the firmware and SDCP
see the string a vendor file carries. A correction is one edit to one file and survives.
Adding a machine is writing a file by hand, with the slip per file ADR 0165 named; the
vendor profile is what it is checked against. Where a file still disagrees with that
profile is not written into the file.

The signal to revisit is a source that covers the machines better than the vendor profiles
and is cheaper to regenerate from than to edit by hand.

## Alternatives considered

### Correct the generator's source and keep generating

It keeps one transcription path. It lost because the source has no machine string and its
panels are the ones found wrong; every fix would be a table of overrides in the generator,
which is a hand-kept catalogue in a worse place.

### The option that won, and what it costs

Hand-kept files drift one by one: nothing holds the 101 files to one shape but review, and a
new machine is typed in rather than read off a source.
