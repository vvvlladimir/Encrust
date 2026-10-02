# 0051. Let a printer profile name the format its firmware reads

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

Since ADR 0047 the output file's extension picks the writer: a `.goo` name writes the
Elegoo container, a `.ctb` name the Chitu one. That is the right rule for a name the user
typed, but it says nothing about which name they should type. A Mars 3 Pro reads `.ctb`
and a Saturn 4 Ultra reads `.goo`, and the user is not supposed to know which firmware
their machine runs.

With a catalogue of machines, the slicer knows.

## Decision

`PrinterProfile` carries `output`, one of `goo`, `ctb4` or `ctb5`, defaulting to `goo`
for a profile that does not say. Picking a printer in the window sets the format pill
under the layer height, so the save dialog opens on a name the machine can read.

ADR 0047 is untouched: the extension still decides. The profile decides the default name,
not the writer. The CLI keeps writing whatever the `-o` name says and prints a line when
that is not what the machine reads, rather than overriding it.

## Consequences

Choosing a printer chooses a file format without the user knowing what a format is, which
is the point of the step. The enum lives in `printer-profiles`, a leaf crate, so both
binaries map from it and no cycle appears; `format-*` crates stay unaware of profiles.

Adding a third container means adding a variant here as well as a crate, so the leaf now
knows the names of formats it cannot write. That is the cost of letting data name a
behaviour.

## Alternatives considered

### Derive the format from the manufacturer

No new field: Elegoo means `.goo`, everything else `.ctb`. Wrong on the facts — Elegoo's
own Mars 3 Pro reads `.ctb` — and it hides a real per-machine property behind a string
that exists for display.

### Let the profile pick the writer outright, over the extension

Fewer surprises for a beginner, but it contradicts ADR 0047 and makes the file name lie: a
plate saved as `plate.ctb` for a Saturn would be a `.goo` file. Rejected.

### The option that won, and what it costs

A field in the profile that only the two binaries read, and a default that is right for
Elegoo and wrong for everyone else who hand-writes a profile without it. A user whose
machine reads `.ctb` and whose profile omits `output` gets a `.goo` default and has to
notice the warning line.
