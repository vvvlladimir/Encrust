# The Anycubic Photon Workshop containers

What Anycubic machines read, and what Photon Workshop exports. One container under many
names: the extension is what a machine's firmware looks for, and the bytes behind it are the
same table container at some revision. We write **version 1**, **516** and **517**.

| Extension | Machine | Revision |
|---|---|---|
| `.pw0` | Photon Zero | 1 |
| `.pwx` | Photon X | 1 |
| `.pwmx` | Photon Mono X | 516 |
| `.pwmo` | Photon Mono | 516 |
| `.pwms` | Photon Mono SE | 516 |
| `.pmsq` | Photon Mono SQ | 516 |
| `.dlp` | Photon Ultra | 516 |
| `.pwma` | Photon Mono 4K | 516 |
| `.pm3` | Photon M3 | 516 |
| `.pm3m` | Photon M3 Max | 516 |
| `.pwmb` | Photon Mono X 6K | 516 |
| `.pwmb` | Photon M3 Plus | 517 |
| `.dl2p` | Photon D2 | 517 |
| `.px6s` | Photon Mono X 6Ks | 517 |
| `.pmx2` | Photon Mono X2 | 517 |
| `.pm3n` | Photon Mono 2 | 517 |
| `.pm3r` | Photon M3 Premium | 517 |
| `.pm5` | Photon Mono M5 | 517 |

`.pwmb` is read at two revisions by two machines, which is why the revision is a field of
the printer profile and not a property of the extension (ADR 0166).

We do not parse whole files; we decode the run-length data of a layer, to check what we
wrote.

## Where the knowledge comes from

There is no published specification.

| Source | What it settles |
|---|---|
| Open-source readers of the container | Table names and order, field types, which revision adds which table, the run-length codec |
| A reference `.pwmx`, read byte by byte | Every offset and unit below, and the three things the source above leaves ambiguous |

The reference file settles three things the code alone does not:

1. **A table's stated length counts different things in different tables.** The preview
   counts its own name and length word in; the header and the layer table do not. Getting
   this wrong moves every table behind it by sixteen bytes.
2. **Two units are not what the rest of the project uses.** A lift or retract speed is
   millimetres a **second**, not a minute. A layer's height in the layer table is that
   layer's own **thickness**, not its height above the plate.
3. **The currency is one UTF-16 code unit in a four-byte field**, not a string. A `$` is
   `24 00 00 00`.

## Shape of a file

```
file mark             48 / 52 / 56 bytes: the container, its revision, and every address
HEADER                16-byte table head, then 80 / 84 / 92 bytes of fields
PREVIEW               16-byte table head, 12 bytes of shape, then raw 565 colour
grey table            516, 517: 28 bytes, no table head
LAYERDEF              16-byte table head, the layer count, then 32 bytes per layer
EXTRA                 516, 517: 16-byte table head, then 56 bytes of two-stage motion
MACHINE               516, 517: 156 bytes, head counted in
software              517: 164 bytes, no table head
MODEL                 517: 48 bytes, head counted in
layer 1               four-bit run-length data
...
layer N
```

The order is the same at every revision; a later one inserts its blocks and nothing moves
relative to the mark's own slots. The grey table goes in front of the layer table, and
everything else between the layer table and the first layer's runs.

Every table begins with its own name, nul-padded to twelve bytes, and a `u32` length. A
reader that followed an address checks the name before trusting it. Everything is
little-endian except one run length; see *Run-length data*.

Layers are written back to back with no padding, no per-layer block in front of them and no
checksum behind them: each layer's data ends exactly where the next begins.

## File mark

| Offset | Field | What we write |
|---|---|---|
| 0x00 | mark, 12 bytes | `ANYCUBIC`, nul-padded |
| 0x0C | version, `u32` | 1, 516 or 517 |
| 0x10 | table count, `u32` | 4 at version 1, 8 at 516, 9 at 517 |
| 0x14 | header address, `u32` | the mark's own end |
| 0x18 | software address, `u32` | 517 only, zero below it |
| 0x1C | preview address, `u32` | |
| 0x20 | grey table address, `u32` | 516 and 517, zero at version 1 |
| 0x24 | layer table address, `u32` | |
| 0x28 | extra address, `u32` | 516 and 517, zero at version 1 |
| 0x2C | machine address, `u32` | **516 and 517 only** |
| 0x2C / 0x30 | layer data address, `u32` | where the first layer's runs begin |
| 0x30 / 0x34 | model address, `u32` | **517 only** |

An address a revision has but a file does not use is written as zero rather than left out:
the mark is a fixed width at a given revision. The machine address is the one that moves
everything behind it, because 516 inserts a slot rather than appending one. The table count
is not the number of addresses written, and the counts above are what the reference writer
states.

Every address is known before anything is written, because the preview is a fixed size and
the layer table is a fixed width once the layer count is known. Only the layer table's rows
and the resin volume are filled in afterwards.

## Header, 80 bytes of fields

Offsets are from the end of the table's 16-byte head.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | pixel size, `f32` | micrometres, from `Display::pixel_pitch_mm` along X |
| 0x04 | layer height, `f32` | mm, the height actually sliced at |
| 0x08 | exposure, `f32` | seconds |
| 0x0C | wait before cure, `f32` | seconds |
| 0x10 | bottom exposure, `f32` | seconds |
| 0x14 | bottom layers, `f32` | a count, in a float field |
| 0x18 | lift distance, `f32` | mm |
| 0x1C | lift speed, `f32` | **mm/s** |
| 0x20 | retract speed, `f32` | **mm/s** |
| 0x24 | resin volume, `f32` | ml |
| 0x28 | anti-alias level, `u32` | 1, see *Grey* |
| 0x2C | resolution X, Y, `u32` each | from `RasterSettings` |
| 0x34 | weight, `f32` | grams |
| 0x38 | price, `f32` | zero for a resin with no price |
| 0x3C | currency, one UTF-16 code unit then padding | the first character of the resin's |
| 0x40 | per-layer settings, `u32` | 1 when exposure or thickness varies by layer |
| 0x44 | print time, `u32` | seconds |
| 0x48 | transition layers, `u32` | |
| 0x4C | transition layer type, `u32` | 0, the linear ramp |

There is no build volume and no machine name here: the machine block revisions 516 and 517
add carries the name, and below 516 the container records nothing about what it was sliced
for. There is no mirroring field either, and the masks go in as they stand — a
reference file converted from a `.sl1` matches the source layer for layer with no flip, so
the panel's mounting stays a property of the profile (ADR 0134).

The stated length is what says which of the later fields are there: 84 at revision 516 adds
`advanced mode`, and 92 at 517 adds `grey`, `blur level` and `resin type`. We write zero in
all four — the slicer plans one lift stage, so basic mode is what the motion block beside it
describes, and nothing here can state a grey level, a blur or a resin type.

## Preview

One preview at version 1, 224 by 168 pixels.

| Offset | Field |
|---|---|
| 0x00 | resolution X, `u32` |
| 0x04 | a mark of its own, 4 bytes: `x`, nul-padded |
| 0x08 | resolution Y, `u32` |
| 0x0C | the pixels |

The pixels are raw five-six-five colour, `rrrrrggg gggbbbbb` in a **little-endian** word,
row by row from the top left, with no run-length coding: the size is always
`width * height * 2`. The job's thumbnail is cut to the record's shape, padded with black
where a square image does not reach it.

This table's stated length counts its own head, so it is `16 + 12 + width * height * 2` and
the next table begins exactly that far on.

## Layer table

The 16-byte head, then a `u32` layer count, then one 32-byte row per layer in print order:

| Offset | Field |
|---|---|
| 0x00 | data address, `u32` |
| 0x04 | data size, `u32` |
| 0x08 | lift distance, `f32`, mm |
| 0x0C | lift speed, `f32`, **mm/s** |
| 0x10 | exposure, `f32`, seconds |
| 0x14 | layer height, `f32`: this layer's **own thickness** |
| 0x18 | lit pixels, `u32` |
| 0x1C | padding, `u32` |

A bottom layer carries the bottom lift distance and speed; every other layer the ordinary
ones. This is what lets exposure and motion change over the height of a print, and the
header's per-layer settings field is what allows a machine to obey it.

*Lit pixels* is a count the machine displays. We count it after the grey is quantised, so it
is what the file cures; the reference writer counts it before, and its own files disagree
with themselves by about half a per cent.

## Run-length data

Runs are taken across the whole image in one pass, row by row from the top left. They do not
restart at a row boundary, and they must cover every pixel of the panel.

The format carries **four bits of grey**: a value loses its low nibble on the way in and,
on the way out, the nibble is repeated into both halves of the byte. The sixteen greys are
therefore 0, 17, 34, … 255, and a value comes back within one step of where it went in —
above it as often as below, since 0x64 becomes 0x66. Anything under 16 falls to nibble zero
and lights nothing. Two eight-bit values sharing a nibble have to be merged into one run, or
a reader that counts runs disagrees with the writer.

There are two chunk forms, and the colour decides which:

| Colour | Chunk | Longest run |
|---|---|---|
| `0x0` or `0xF` | two bytes, `cccc` then twelve bits of length, **big-endian** | 4095 |
| anything between | one byte, `cccc` then four bits of length | 15 |

Black and white get the long form because a one-byte chunk of either could not be told from
the first byte of a two-byte one. It is also where it pays: those two are what a layer is
mostly made of. A longer run is split into several chunks.

The word of the long form is the one big-endian field in the container.

## Grey, and the anti-alias level

The header's anti-alias level asks a machine to read that many one-bit passes of every
layer and sum them, which is how the `.pws` container carries grey. This one does not need
it: the runs carry four bits of their own. We write **1**, because a level above one asks a
machine to look for passes it will not find. See `docs/decisions/0147`.

## The blocks revisions 516 and 517 add

The layer codec does not change, and neither does anything in front of the grey table.

**Grey table**, 28 bytes with no name or length of its own: a `u32` saying whether the full
greyscale is in use, a `u32` count of levels, that many bytes of eight-bit grey, and a `u32`
nobody has identified. The count is sixteen, and at one anti-alias level the reference
writer fills every entry with 255 — the nibble in the layer data is the grey either way.

**`EXTRA`**, the two-stage motion block: a stage count and three floats for the bottom
block, then the same for the rest. The second stage is zero and both counts are one, because
the slicer plans one lift. The block **states a length of 24 and writes 56 bytes of
fields**; the reference writer does the same, and a reader that believed the number would
stop in the middle of it.

**`MACHINE`**, 156 bytes with its own head counted in: the machine name in 96 bytes, the
layer codec's name (`pw0Img`) in 16, the highest anti-alias level, a property count — 1 at
516 and 7 at 517 — the panel in millimetres, the travel, the newest revision the extension
takes, and the colour the machine paints behind a preview.

**Software**, 164 bytes with no name: the slicer's name in 32 bytes, its own length as a
`u32`, the version in 32, the host system in 64, and an OpenGL version in 32 that is left
blank because nothing here renders through OpenGL.

**`MODEL`**, 48 bytes: the box the print stands in, measured from the panel's centre, then
whether supports were generated and how dense. The stack's extent in X and Y does not reach
the writer, so the box is the whole panel — an over-estimate the machine only draws with.

## What we do not write

- **Revisions 515 and 518, and the `.pws` codec.** No machine's published profile asks for
  515. 518 adds a second preview, a sub-layer table and a machine block of 224 bytes, and
  two machines read it. `.pws` is the same container with a different layer codec.
- **A checksum.** The reference writer computes one over a layer's runs and does not write
  it; the field it would go in does not exist.
- **The two-stage lift.** The motion block is there from 516 on and the header's advanced
  mode is what switches it on; one stage is what the slicer plans, so it stays off.
