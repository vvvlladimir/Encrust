# 0375. Write `.prz` as `.goo` and `.crmaslice` as the gcode archive

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

Phrozen's newer machines read a `.prz` whose bytes are a `.goo` (`docs/formats/goo.md`), and
twelve Carima machines read a `.crmaslice` that is the gcode-and-PNG archive
(`docs/formats/gcode-zip.md`). Nothing in either file but the extension tells them apart.

## Decision

`SlicedFormat::Prz` and `SlicedFormat::Crmaslice` are formats of their own, so a profile and
an output name can choose them, and `core-pipeline` writes them through `GooWriter` and
`GcodeZipWriter` under their own extension; reading either extension opens the same reader.
No crate is added.

## Consequences

The Phrozen and Carima profiles ship with the container their machines read, and a file of
theirs reads back here. A change to `.goo` or the archive reaches both at once. Reopen if
either vendor's file diverges from the container it copies.

## Alternatives considered

### A flavour on the existing writers, as `.cbddlp` has

It would spread an extension choice into two format crates for what is only a name.

### The option that won, and what it costs

Two more variants in `SlicedFormat` whose writer is another's, which a reader of the enum has
to know.
