# The Anycubic Photon Workshop containers

What Anycubic machines read. One container under many names: the extension is what a
machine's firmware looks for, and the bytes behind it are the same table container stating
some revision. **one set of tables at every revision**, with the revision it writes for
the extension — **1**, **515**, **516**, **517** or **518** — and the machine block's
property count as the only things the number changes.

| Extension | Machine | Revision |
|---|---|---|
| `.pw0` | Photon Zero | 1 |
| `.pwx` | Photon X | 1 |
| `.pwmx` | Photon Mono X | 516 |
| `.pwmo` | Photon Mono | 515 |
| `.pwms` | Photon Mono SE | 515 |
| `.pmsq` | Photon Mono SQ | 515 |
| `.dlp` | Photon Ultra | 515 |
| `.pwma` | Photon Mono 4K | 516 |
| `.pm3` | Photon M3 | 516 |
| `.pm3m` | Photon M3 Max | 516 |
| `.pwmb` | Photon Mono X 6K | 517 |
| `.pwmb` | Photon M3 Plus | 517 |
| `.dl2p` | Photon D2 | 517 |
| `.px6s` | Photon Mono X 6Ks | 517 |
| `.pmx2` | Photon Mono X2 | 517 |
| `.pm3n` | Photon Mono 2 | 517 |
| `.pm3r` | Photon M3 Premium | 517 |
| `.pm5` | Photon Mono M5 | 517 |
| `.pm5s` | Photon Mono M5s | 518 |
| `.m5sp` | Photon Mono M5s Pro | 518 |
| `.pm4n` | Photon Mono 4 | 518 |

The M7 line, the Mono 4 Ultra and the P1 read another container with these layers inside:
`docs/formats/pwsz.md`.

The revision is a field of the printer profile, not a property of the extension (ADR 0166);
no two machines under one extension are written at different revisions.

We do not parse whole files; we decode the run-length data of a layer, to check what we
wrote.

## Where the knowledge comes from

There is no published specification.

| Source | What it settles |
|---|---|
| Open-source readers of the container | Table names and order, field types, which revision adds which table, the run-length codec |
| A reference `.pwmx`, read byte by byte | Every offset and unit below, and the three things the source above leaves ambiguous |
| The vendor's own slicer's writer, read statically | What we write, field by field: the revision each extension is written at (518 for one it does not list), that every revision gets the same tables, and every value below marked as the slicer's |

The reference file settles three things the code alone does not:

1. **A table's stated length counts different things in different tables.** The machine
   block counts its own name and length word in; the header, the preview and the layer table
   do not. Getting this wrong moves every table behind it by sixteen bytes.
2. **Two units are not what the rest of the project uses.** A lift or retract speed is
   millimetres a **second**, not a minute. A layer's height in the layer table is that
   layer's own **thickness**, not its height above the plate.
3. **The currency is one UTF-16 code unit in a four-byte field**, not a string. A `$` is
   `24 00 00 00`.

## Shape of a file

```
file mark             64 bytes: the container, its revision, and eleven addresses
HEADER                16-byte table head, then 96 bytes of fields
PREVIEW               16-byte table head, 12 bytes of shape, then raw 565 colour
grey table            8 bytes, no table head
LAYERDEF              16-byte table head, the layer count, then 32 bytes per layer
EXTRA                 16-byte table head, then 56 bytes of two-stage motion
MACHINE               224 bytes, head counted in
software              164 bytes, no table head
Model                 44 bytes: a name and no length
SUBIMGS               24-byte head, then 44 bytes per layer
layer 1               four-bit run-length data
...
layer N
```

Every revision writes all of these, in this order; the number on the file says which
revision a machine is to take it as. Revision 1 and 515 files written by older Photon
Workshop releases carry fewer tables, and the reader opens those as well: it finds the
machine block by the mark's table count, not by its revision.

Every table but the grey table, the software block and the model block begins with its own
name, nul-padded to twelve bytes, and a `u32` length. A reader that followed an address checks
the name before trusting it. Everything is little-endian except one run length; see
*Run-length data*.

Layers are written back to back with no padding, no per-layer block in front of them and no
checksum behind them: each layer's data ends exactly where the next begins.

## File mark

| Offset | Field | What we write |
|---|---|---|
| 0x00 | mark, 12 bytes | `ANYCUBIC`, nul-padded |
| 0x0C | version, `u32` | 1, 515, 516, 517 or 518 |
| 0x10 | table count, `u32` | 11 |
| 0x14 | header address, `u32` | the mark's own end |
| 0x18 | software address, `u32` | |
| 0x1C | preview address, `u32` | |
| 0x20 | grey table address, `u32` | |
| 0x24 | layer table address, `u32` | |
| 0x28 | extra address, `u32` | |
| 0x2C | machine address, `u32` | |
| 0x30 | layer data address, `u32` | where the first layer's runs begin |
| 0x34 | model address, `u32` | |
| 0x38 | second layer table address, `u32` | |
| 0x3C | second preview address, `u32` | zero: there is none |

Every address is known before anything is written, because the preview is a fixed size and
both layer tables are a fixed width once the layer count is known. Only their rows and the
resin volume are filled in afterwards.

## Header, 96 bytes of fields

Offsets are from the end of the table's 16-byte head.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | pixel size, `f32` | micrometres: the coarser axis, rounded to a whole micron |
| 0x04 | layer height, `f32` | mm, the height actually sliced at |
| 0x08 | exposure, `f32` | seconds |
| 0x0C | wait before cure, `f32` | seconds |
| 0x10 | bottom exposure, `f32` | seconds |
| 0x14 | bottom layers, `f32` | a count, in a float field |
| 0x18 | lift distance, `f32` | mm |
| 0x1C | lift speed, `f32` | **mm/s** |
| 0x20 | retract speed, `f32` | **mm/s** |
| 0x24 | resin volume, `f32` | ml |
| 0x28 | anti-alias level, `u32` | how many greys an edge is drawn in, see *Grey* |
| 0x2C | resolution X, Y, `u32` each | from `RasterSettings` |
| 0x34 | weight, `f32` | grams |
| 0x38 | price, `f32` | zero for a resin with no price |
| 0x3C | currency, one UTF-16 code unit then padding | the first character of the resin's |
| 0x40 | per-layer settings, `u32` | 0, as the slicer leaves it; the rows carry each layer's own |
| 0x44 | print time, `u32` | seconds |
| 0x48 | transition layers, `u32` | |
| 0x4C | transition layer type, `u32` | 0, the linear ramp |
| 0x50 | advanced mode, `u32` | 0: the slicer plans one lift stage |
| 0x54 | grey level, `u16` | the anti-alias level again |
| 0x56 | blur level, `u16` | the edge blur, pixels |
| 0x58 | resin type, `u32` | 10, which the slicer states for every resin |
| 0x5C | intelligent mode, `u32` | 0: the machine keeps the exposure it is given |

There is no build volume and no machine name here: the machine block carries the name.
There is no mirroring field either, and the masks go in as they stand — a reference file
converted from a `.sl1` matches the source layer for layer with no flip, so the panel's
mounting stays a property of the profile (ADR 0134).

## Preview

One preview, 224 by 168 pixels.

| Offset | Field |
|---|---|
| 0x00 | resolution X, `u32` |
| 0x04 | a `u32` the slicer always states as 42 |
| 0x08 | resolution Y, `u32` |
| 0x0C | the pixels |

The pixels are raw five-six-five colour, `rrrrrggg gggbbbbb` in a **little-endian** word,
row by row from the top left, with no run-length coding: the size is always
`width * height * 2`. The job's thumbnail is cut to the record's shape, padded with black
where a square image does not reach it; a job with none gets a black one, where the slicer
would state an empty preview.

The stated length is `12 + width * height * 2`: the shape and the pixels, not the head.

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
ones. This is what lets exposure and motion change over the height of a print.

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

The runs carry four bits of grey of their own, so the header's anti-alias level only says how
many greys an edge was drawn in, as the slicer states it: the panel's grey levels, at most the
sixteen a nibble holds, sixteen when every grey is kept, and 1 when the panel is drawn without
grey. The `.pws` codec is the one that reads the level as a count of passes; see below.

## The blocks behind the layer table

**Grey table**, 8 bytes with no name or length: two zero words, as the slicer writes it — no
levels listed, since the nibble in the layer data is the grey either way.

**`EXTRA`**, the two-stage motion block: for the bottom layers and then the rest, a stage
count of 2 and, per stage, a height in mm and the speeds up and down in mm/s. The slicer
plans one lift, so the second stage climbs nothing at no speed and retracts at the first's.
The block **states a length of 24 and writes 56 bytes of fields**, as the slicer does.

**`MACHINE`**, 224 bytes with its own head counted in: the machine name in 96 bytes, the
layer codec's name (`pw0Img`) in 16, the highest anti-alias level (16), a property count —
1, 7 at 517 and 15 at 518 — the panel in millimetres, the travel, the file's own revision,
the colour the machine paints behind a preview, the pixel along X and along Y in micrometres
as `f32`, eight zero words, a panel count of 1, a zero word, the resolution as two `u16`, and
four zero words.

**Software**, 164 bytes with no name: the slicer's name in 32 bytes, a length of 128 as a
`u32`, the version in 32, the host system in 64, and the OpenGL profile the slicer states,
`3.3-CoreProfile`, in 32.

**`Model`**, 44 bytes, named in mixed case and with no length: the box the print stands in,
measured from the panel's centre, then whether supports were generated and how dense. The
stack's extent in X and Y does not reach the writer, so the box is the whole panel — an
over-estimate the machine only draws with.

**`SUBIMGS`**, counting its head in: a `u32` layer count and a `u32` index of 1, then per
layer the address, the length and the lit pixel count of the same layer data the first table
points at, and eight floats nobody has identified, written as zero. It is filled in at the
end, like the first table.

## The `.pws` codec

The `.pws` of the Photon and Photon S is the version-1 container with another layer codec,
which only the extension names. A layer is as many one-bit passes of the panel as the
header's anti-alias level says, one after another, from 1 to 16. Each byte is a run: the high
bit lights it, the low seven bits count 0 to 127 pixels. A pass ends on the byte that fills
the panel exactly, and a run past it is corrupt. A pixel lit in `c` of `n` passes is grey
`256 c / n − 1`, so one pass is black and white. This is how the open-source readers
decode it, and a real file with four passes reads back as its model's volume.

## What we do not write

- **The second preview, and the `.pws` codec.** 518 has a slot for a second preview the
  vendor's own writer leaves empty, and so do we. A `.pws` is read, not written: see below.
- **A checksum.** The reference writer computes one over a layer's runs and does not write
  it; the field it would go in does not exist.
- **The two-stage lift.** The header's advanced mode is what switches the motion block's
  second stage on; one stage is what the slicer plans, so it stays off.
