# 0362. Write `.cxline`, `.phz` and `.photons` in the crates of their families, and trace masks in `core-raster`

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

Four machines in the vendor's own catalogue read a container we did not write: the PioNext
D158 and D160 a `.cxline`, the Phrozen Sonic Mini a `.phz`, the Anycubic Photon S a
`.photons`. Their writers, read statically out of the vendor's slicer, settle what each is:

- `.cxline` is the `.cxdlp` version 3 header, previews, settings, footer and checksum, with a
  layer held as polygons in micrometres instead of vertical lines.
- `.phz` runs the `.fdg` writer's layer loop and encoder instruction for instruction —
  previews, machine name, table, per-layer records, one-bit codec, the header from 0xC0 on —
  under another header and another cipher.
- `.photons` shares nothing with any container we write: big-endian doubles, a raw preview
  and a bit-reversed one-bit codec.

ADR 0148 makes a format a `format-*` crate of its own; ADR 0146 and 0168 keep a family in
one crate when its members share codecs. A vector layer has to be traced out of the mask, as
ADR 0169 does for `.svgx` inside `format-svgx`, and an edge between two format crates is
forbidden. Reading a vector layer back needs the rasteriser, which takes `core-slicer`'s
contours.

## Decision

- `.cxline` is a `cxline` module of `format-creality`, on the version 3 blocks and the
  family's checksum. `format-creality` takes `core-slicer` and `glam`, as `format-svgx`
  does, to fill a read layer's rings.
- The tracer moves from `format-svgx` into `core-raster` as `rings_of` and `Ring`, which
  `.svgx` and `.cxline` both call: it reads `LayerRuns` and nothing else.
- `.fdg`, the Chitu layer records and previews under a header of its own and a one-bit
  codec, is an `fdg` module of `format-chitu` rather than a crate on its own (ADR 0148): it
  shares that crate's records and cipher.
- `.phz` is a `phz` module of `format-chitu`, writing through the `.fdg` sink, which takes the
  function that writes its header, and read through the `.fdg` reader with its own cipher.
- `.photons` is a `photons` module of `format-anycubic`, the Photon S's maker.

Each writes key zero where it has a cipher (ADR 0046), and each has a reader, so every file
is read back. Layouts are in `docs/formats/cxline.md`, `phz.md` and `photons.md`.

## Consequences

Four machines are in the catalogue. `core-raster` now turns a mask back into rings as well
as contours into masks. A fix to the `.fdg` sink lands in `.phz` with it, and a break of it
breaks both. None of the four has printed a file from here; the signal to revisit is a file
sliced for one of them by the vendor, compared byte for byte, or a machine that refuses ours.

## Alternatives considered

### A crate for each container

What ADR 0148 asks by default. It lost because `.cxline` would reach the `.cxdlp` blocks
and checksum and `.phz` the `.fdg` sink only across a forbidden edge, or by moving one
vendor's blocks into `core-format`.

### A copy of the `.fdg` sink for `.phz`

Fewer moving parts in `fdg.rs`. It lost because the two would be the same 150 lines.

### The option that won, and what it costs

`format-creality` gains two dependencies for one reader, and `format-chitu`'s `.fdg` sink
carries a function pointer it did not need alone. The vendor's own oddities are kept where a
machine might read them — the `.photons` lit count missing a layer's last stretch, the
D158's model code — and dropped where they lose data: the `.cxline` preview's green bits.
