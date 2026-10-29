# The Elegoo `.goo` format

The container Elegoo's resin printers read. We write it; we do not parse whole files, only
the run-length data of a layer. Phrozen's newer machines read the same bytes as `.prz`.

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| [`Goo Format Spec V1.2`](https://github.com/elegooofficial/GOO) | Field order, names, types and units of the header and the layer definition |
| Open-source readers of the container | Run-length bit layout, checksum, the ending string, the values other slicers write |
| Files written for a real machine, read byte by byte | Cross-check of the same, and the offsets below |

Three things the official specification does not state and that a file is rejected over:

1. **Every multi-byte field is big-endian.** The specification never says so.
2. **Each layer carries a checksum**: the negated sum of its run bytes. Not in the
   specification at all.
3. **The ending string is eleven bytes**, not the twelve the diagram on page 1 suggests:
   three pad bytes followed by the eight-byte magic tag, which is what real files contain.

## Shape of a file

```
header        0x2FB95 bytes, fixed
layer 1       definition, then run-length data
...
layer N
ending        11 bytes
```

The header's `offset of layer content` repeats `0x2FB95`. Because the header is a fixed
size and the layer count is known before the first layer is written, the whole file goes
out front to back in one pass with no seeking back. See
`docs/decisions/0012-streaming-sliced-file-writer.md`.

## Header

Offsets are decimal, from the start of the file. The full field list is in the
specification; this table names the ones whose value is a decision rather than a copy of a
profile.

| Offset | Field | What we write |
|---|---|---|
| 0 | version, 4 bytes | `V3.0`; `V5.1` for a machine whose profile names it, see *Revision 5.1* |
| 4 | magic tag, 8 bytes | `07 00 00 00 44 4C 50 00` |
| 12 | software info, 32 bytes | `Encrust` |
| 44 | software version, 24 bytes | the crate version |
| 68 | file time, 24 bytes | `YYYY-MM-DD HH:MM:SS` in UTC |
| 92 | printer name and type, 32 bytes each | `PrinterProfile::machine_name()` in both, the string the firmware matches (ADR 0140) |
| 156 | profile name, 32 bytes | `MaterialProfile::name` |
| 188 | anti-aliasing level, `u16` | 8 for coverage shading, 1 for binary |
| 190 | grey level, `u16` | how many greys the masks were rounded to, 0 when unrounded |
| 192 | blur level, `u16` | radius in pixels the edges were faded over, 0 for binary |
| 194 | small preview | 116 x 116 RGB565, big-endian |
| 27106 | delimiter | `0D 0A` |
| 27108 | big preview | 290 x 290 RGB565, big-endian |
| 195308 | delimiter | `0D 0A` |
| 195310 | total layers, `u32` | |
| 195314 | X and Y resolution, `u16` each | from `RasterSettings` |
| 195318 | X and Y mirror, `bool` each | see *Mirroring* below |
| 195320 | X, Y, Z size, `f32` each | illuminated area and the machine's Z travel, mm |
| 195332 | layer thickness, `f32` | the height actually sliced at, not the profile's |
| 195340 | exposure delay mode, `bool` | always `true`, as in every known file; see below |
| 195341 | turn off time, then six static waits, `f32` each | the light-off delay or zero; before lift, after lift, after retract, for the bottom layers and then the rest |
| 195445 | advance mode, `bool` | see below |
| 195458 | total price, `f32`, then its unit as 8 bytes | what the job costs in the resin's own currency, `€/L` or `$/kg`; zero for a resin with no price |
| 195470 | offset of layer content, `u32` | `0x2FB95` |
| 195474 | grey scale level, `bool` | `true`: masks use the whole `0x00..0xFF` range |
| 195475 | transition layers, `u16` | |

Both previews hold the job's thumbnail, cut to their square from the picture
`core-thumbnail` renders of the plate; a job without one leaves them black. Their size is
part of the fixed header, so the bytes cannot be left out either way. A pixel is
`rrrrrggg gggbbbbb`, most significant byte first like every other field of the container.

*Advance mode* tells the printer to read exposure and motion from each
layer rather than from the header. We set it whenever a layer's exposure differs from the
header's — transition layers, or a band of height (ADR 0090) — and only on a machine whose
profile says it reads the per-layer tables (ADR 0141). A known file leaves it clear for an
Elegoo Saturn 4 Ultra and lets the machine ramp the bottom block off the header. It also
states 0.05 in every lift and retract field, distance and speed, which the machine's
profile locks (ADR 0301): a file stating 6 mm made the plate rise again after the tilt.

*Exposure delay mode* is the constant 1 in every known `.goo`, V3 and V5 alike,
whether its resin waits by a light-off delay or by rests: the wait not taken is written as
zeros instead. We write the same, and a reader tells the mode from the values — any rest
above zero is resting, else the light-off delay — never from the byte.

### Mirroring

Row zero of a layer is the near edge of the plate on an X-mirrored machine and its far edge
on any other, which is how known files print the right way round (ADR 0134, 0366).

The mirror flags say how the panel is mounted, and come from `PrinterProfile::mirror_x` and
`mirror_y` rather than from the mask, which carries no mirroring. Readers take them to
show the layer the way the plate stood.

## Layer

Each layer is a 66-byte definition, then its data:

| Offset in the layer | Field |
|---|---|
| 0 | pause flag, `u16` |
| 2 | pause position Z, `f32`, the layer's own Z so a pause leaves the plate where it stands |
| 6 | layer position Z, `f32`, the **top** of the layer |
| 10 | layer exposure time, `f32` |
| 14 | layer off time, `f32`: the bottom light-off on a bottom layer |
| 18 | before lift, after lift, after retract, `f32` each |
| 30 | lift distance and speed, `f32` each |
| 38 | second-stage lift distance and speed, `f32` each |
| 46 | retract distance and speed, `f32` each |
| 54 | second-stage retract distance and speed, `f32` each |
| 62 | light PWM, `u16` |
| 64 | delimiter `0D 0A` |
| 66 | data size, `u32` |
| 70 | magic `0x55` |
| 71 | run-length data |
| | checksum, `u8` |
| | delimiter `0D 0A` |

`data size` counts the magic byte, the runs and the checksum: `runs.len() + 2`. The
checksum covers **only the runs** — neither the magic byte nor itself.

## Run-length data

Runs are taken across the whole image in one pass, row by row from the top left. They do
not restart at a row boundary.

**Every pixel must be covered.** The printer decodes a layer into an uninitialised buffer,
so a run that stops short leaves whatever the previous layer put there, and the part comes
out with garbage on it.

Each chunk starts with one byte, `aabbcccc`:

| `aa` | Meaning |
|---|---|
| `00` | a run of `0x00` |
| `01` | a run of one grey value, which follows in the next byte |
| `10` | a step from the previous value |
| `11` | a run of `0xFF` |

For `00`, `01` and `11`, `bb` says how wide the run length is and `cccc` holds its low
four bits. The remaining bytes follow the value byte, most significant first:

| `bb` | Run length | Extra bytes |
|---|---|---|
| `00` | up to `0xF` | none |
| `01` | up to `0xFFF` | `len >> 4` |
| `10` | up to `0xFFFFF` | `len >> 12`, `len >> 4` |
| `11` | up to `0xFFFFFFF` | `len >> 20`, `len >> 12`, `len >> 4` |

A run longer than `0xFFFFFFF` is split into several chunks.

For `10` the layout is `10abcccc`: `a` is the sign of the step, `b` says whether the run
is one pixel (`0`) or as long as the next byte says (`1`), and `cccc` is the magnitude,
at most 15. A step chunk therefore covers at most 255 pixels, so we only choose it for
runs that short; a longer run of the same grey is cheaper as an `01` chunk.

The first chunk of a layer is never a step chunk: the value it would step from is the
value the previous layer left behind.

## Revision 5.1

What the Elegoo Jupiter 2 reads, and so what we write when a
profile's `output` is `goo5` (ADR 0376). Read from known files.

**Every multi-byte field is little-endian**, the previews included. The header up to the
transition layers is the version 3.0 header at the same offsets, but for:

- the version, `V5.1`;
- the first-stage retract, header and layers alike: the resin's retract less the layer's
  thickness, the travel down that leaves the plate a layer higher. A known file with a
  2 mm lift at 0.05 mm states 1.95.

Behind the header, at 195477, come fifteen bytes and three tables, every offset absolute:

| Offset | Field |
|---|---|
| 195477 | `u8` 2 |
| 195478 | `u32` each: where the definition, mask and block tables start |
| 195490 | `u16` `0x0300` |
| A1 | `0xA1`, then per layer `u32` offset and `u32` 66 of its definition |
| A2 | `0xA2`, then per layer offset and size of its left mask, then of its right |
| A3 | `0xA3`, `u32` 1, then offset and size, 142, of the name block |

The definitions follow A3, 66 bytes each, laid out as in version 3.0 with their delimiter
and nothing behind it. Then the masks, a layer's left half and then its right. Then the
name block: `0x66`, the resin preset's name in 139 bytes, `0D 0A`. The file closes with the
MD5 of every byte before it, 32 lower-case hex characters, in place of the ending string.

### A layer's masks

The panel is split down the middle and each half is its own mask, `width / 2` wide: the
Jupiter 2's 15120 columns are two of 7560. A mask is `0x55`, the runs, the checksum of
version 3.0 over the runs, `0D 0A`; its size counts all four.

A pixel is stored as one of eight levels, its grey shifted right by five; a reader spreads
the level back over the byte, `level * 255 / 7`. Runs cross row ends. Each starts with one byte:

| Byte | Meaning |
|---|---|
| `00lllxxx` | `lll` pixels, 1 to 6, of the current level `xxx`; `111` means 7 or more |
| `1sssssss` | the level changes by `sssssss - 0x20`, -7 to +7, and stays for 1 pixel or more |
| `01bbcccc` | a length field, only ever behind one of the two above |

A length field stores a number `m` in `cccc` and `bb` more bytes, least significant first:
`m & 0xF`, then `m >> 4`, `m >> 12`, `m >> 20`. Behind `00111xxx` the run is `m + 8`
pixels, behind a level change `m + 2`; without one, 7 and 1. The level starts at 0.
