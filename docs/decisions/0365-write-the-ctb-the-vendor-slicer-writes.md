# 0365. Write the `.ctb` the vendor slicer writes, and pick its revision by the panel

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

A Creality LD-002H prints the `.ctb` the vendor slicer makes for it and refuses ours. Byte for
byte the two differed in revision (ours 4, theirs 5), block order, the slicer info flags and
software version, the count and totals in the version 4 block, the layer key and a tail after
every layer and after the file. The vendor slicer's own serialiser settles the revision: a panel
with a side past 4095 pixels gets the encrypted container (`0x12FD0107`), every other one
version 5. It never writes version 4. ADR 0046 kept the layer key zero and ADR 0165 sent every
`.ctb` machine to version 4; both rest on the claim that firmware does not care.

## Decision

`format-chitu` writes the `.ctb` as the vendor's slicer lays it down
(`docs/formats/chitu.md`): print parameters, slicer info and the version 4 block before the
previews; a per-layer flag only when a layer differs from the header; a software version and
total height of zero; the summed layer time, largest layer and layer count in the version 4
block; a layer key that is never zero; each layer's lit area behind its data; a CRC32 of
everything as the last four bytes.

A profile's `output` is `ctb5`, or `ctb-encrypted` when a side of its panel is past 4095
pixels. Version 4 stays writable for a machine that asks for it on the command line, where
`--ctb-version` now defaults to the profile's revision.

## Consequences

Our file and the vendor's for LD-002H agree in every field but the previews, the time
estimate and the edge rule of a pixel. A layer costs one more pass over its bytes for the key.
The tests decode through the key, as the reader already did for vendor files.

Reopen if a machine is found that prints version 4 and refuses version 5.

## Alternatives considered

### Keep version 4 and the zero key, and find the one field LD-002H rejects

Cheaper per field, but each guess costs a print on a machine we do not hold, and no machine
was ever seen to print a file the vendor slicer would not write.

### The option that won, and what it costs

The layer cipher and the CRC make a broken byte harder to spot by eye in a dump, and a hand edit
of a file now needs the CRC recomputed. This supersedes ADR 0046 and the `.ctb` revision of
ADR 0165.
