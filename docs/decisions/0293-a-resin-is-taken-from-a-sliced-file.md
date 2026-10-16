# 0293. A resin is taken from a sliced file

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

No resin ships (ADR 0196), so a user moving from another slicer starts by typing in numbers
that program already holds. Every file it wrote for their machine carries them: a `.goo`
states the exposure, the transition, the three rests, every peel and the light power; the
Chitu and Anycubic headers and the archives' settings most of these. Our readers kept only
the four exposure fields of `SlicedFile`. Files for a tilting vat state placeholders where a
peel would be (the layer height, 0.05 mm/min, or zero), and one container states a speed in
the other unit.

## Decision

`SlicedFile` carries `stated: PrinterTuning`, the resin's own per-printer table, holding
what the container records beyond the exposure fields; `SlicedFile::tuning` adds those, and
`SlicedFile::resin` lays it over the default resin under the file's resin name or its stem.
`core-format` re-exports `PrinterTuning`, `Waits` and `WaitMode`, so a `format-*` crate
still depends on `core-format` alone (ADR 0148).

A reader keeps a value only where a printer could take it: a distance of 0.5 to 100 mm, a
speed of 1 to 1000 mm/min, a wait of 0 to 600 s, a PWM of 1 to 255. Anything else is `None`
and the resin keeps its default. A container with separate rests for the bottom layers gives
the common ones, the only set a resin holds.

`Catalogue::printer_for_panel` picks the machine a file was cut for: the one machine of its
panel, or among several the one whose firmware name or label the file names, case and
spacing aside. `Catalogue::import_resin` takes the resin in as an import does (ADR 0235).
`encrust profiles import FILE` with no `--kind` reads `FILE` as a sliced file, and the
window's resin imports take a sliced file wherever they take a resin's TOML, setting it up
on the printer in view or in hand.

## Consequences

A user's first resin is the one their old slicer printed with, on the right machine, with
no typing. The readers of `.goo`, `.ctb` and `.cbddlp`/`.photon`, the Anycubic family,
`.cws` and gcode-zip state what they hold; `.cxdlp`, `.sl1` and `.svgx` state only the
exposure. An encrypted `.ctb` is not read at all, so its resin waits on that reader. A file
whose panel several machines share and which names none of them needs `--printer`. Reopen
the bounds if a real machine is found peeling outside them.

## Alternatives considered

### A separate importer per container, outside the readers

It would leave `SlicedFile` as it was, but read every header twice, in two places that
must agree on each offset; the reader already walks the header.

### Keeping placeholders as the file states them

Faithful to the bytes, but a resin with a 0.05 mm/min lift taken to a peel machine would
stall it. Dropping them costs a user whose machine really peels at under 1 mm/min the
default instead of their number; no such machine is known.
