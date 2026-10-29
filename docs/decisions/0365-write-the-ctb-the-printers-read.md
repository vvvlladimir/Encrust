# 0365. Write the `.ctb` the printers read, and pick its revision by the panel

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

A Creality LD-002H prints a known `.ctb` for it and refuses ours. Byte for
byte the two differed in revision (ours 4, theirs 5), block order, the slicer info flags and
software version, the count and totals in the version 4 block, the layer key and a tail after
every layer and after the file. The panel settles the revision: a panel
with a side past 4095 pixels gets the encrypted container (`0x12FD0107`), every other one
version 5. It never writes version 4. ADR 0046 kept the layer key zero and ADR 0165 sent every
`.ctb` machine to version 4; both rest on the claim that firmware does not care.

The encrypted writer had copied its unknown words from a single file. Against
known files for the GK3 Ultra, Sonic Mega 8K S, Whale3 Super 14K and Mars 4
Ultra it wrote one peel stage where they carry two, the normal light-off delay for the
bottom layers, the per-layer flag on every file, and constants where they carry the
stack's totals, each layer's area and a CRC.

## Decision

`format-chitu` writes the `.ctb` as the printers read it
(`docs/formats/chitu.md`): print parameters, slicer info and the version 4 block before the
previews; a per-layer flag only when a layer differs from the header; a software version and
total height of zero; the summed layer time, largest layer and layer count in the version 4
block; a layer key that is never zero; each layer's lit area behind its data; a CRC32 of
everything as the last four bytes.

`format-ctb-encrypted` writes the same numbers into its settings block and definitions
(`docs/formats/ctb-encrypted.md`): both stages and their whole climb, the bottom light-off,
the slicer flags, anti-alias level, cost and version 4 totals, a lit-area tail behind every
layer and a CRC32 after the signature. The per-layer flag stays clear for a transition ramp,
as in a known file with one. What both containers compute alike — a layer's peel, its
area, the default resin type — lives once in `core-format`.

A profile's `output` is `ctb5`, or `ctb-encrypted` when a side of its panel is past 4095
pixels. Version 4 stays writable for a machine that asks for it on the command line, where
`--ctb-version` now defaults to the profile's revision.

## Consequences

Our file and the known one for LD-002H, and the encrypted ones for the four machines above,
agree in every field but the addresses, previews, layer key, time estimate and the edge rule
of a pixel. A layer costs one more pass over its bytes for the key.
The tests decode through the key, as the reader already did for known files.

Reopen if a machine is found that prints version 4 and refuses version 5. The plain `.ctb`
still sets its per-layer flag for a transition ramp; no known file of that container with
one has been seen.

## Alternatives considered

### Keep version 4 and the zero key, and find the one field LD-002H rejects

Cheaper per field, but each guess costs a print on a machine we do not hold, and no machine
was ever seen to print a file laid out otherwise.

### The option that won, and what it costs

The layer cipher and the CRC make a broken byte harder to spot by eye in a dump, and a hand edit
of a file now needs the CRC recomputed. This supersedes ADR 0046 and the `.ctb` revision of
ADR 0165.
