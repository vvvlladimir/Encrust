# The encrypted `.ctb`

The container the newer Chitu boards take and no other: the plain `.ctb`'s fields
(`chitu.md`) gathered into one settings block that is enciphered and signed. Encrust reads it
whoever wrote it and writes it for a printer whose profile says `output = "ctb-encrypted"`,
so that a user's own file prints on the machine they own (ADR 0298).

## Where the knowledge comes from

There is no published specification. The layout follows the open-source readers and writers
of the container, checked against two known files for a Saturn 4 Ultra (a
trimmed copy of one is the crate's fixture) and, for what the writer lays down, against known
files for the GK3 Ultra, Sonic Mega 8K S, Whale3 Super 14K and
Mars 4 Ultra with one resin whose two-stage peel and bottom light-off were unique (ADR 0365).
A file we write is opened by an open-source reader with its signature accepted.

## Shape of a file

```
header                48 bytes, in the clear
settings              288 bytes, enciphered
large preview         32-byte record, then RGB15 run-length data   (as chitu.md)
small preview         32-byte record, then RGB15 run-length data
machine name          not nul-terminated
disclaimer            320 bytes of text                            (as chitu.md)
resin parameters      40 bytes, then three strings                 (version 5 only)
layer pointers        16 bytes per layer
layer 1               88-byte definition, run-length data, then the lit area f32 mm² and 0
...
layer N
signature             32 bytes
CRC32                 zlib's, of every byte before it, little-endian
```

Every address is a file offset except those split into a page and an offset within it,
where a page is 2^32 bytes: the layer pointers and the data address in a definition.

## Header, 48 bytes

| Offset | Field | What we write |
|---|---|---|
| 0x00 | magic, `u32` | `0x12FD0107` |
| 0x04 | settings size, `u32` | 288 |
| 0x08 | settings address, `u32` | 48 |
| 0x0C | unknown, `u32` | 0 |
| 0x10 | version, `u32` | 5 |
| 0x14 | signature size, address, `u32` each | 32, and where it went |
| 0x1C | unknown, `u32` | 0 |
| 0x20 | two `u16` | 1, 1 |
| 0x24 | three `u32` | 0, 42, 0 |

## Cipher and signature

The settings are AES-256 in CBC mode with no padding, under one key and initialisation
vector every file shares; they are in `format-ctb-encrypted/src/cipher.rs`. The signature is
the SHA-256 of the settings' first eight bytes, the checksum, enciphered the same way. A
reader that finds them differ refuses the file. We write a checksum of zero, as known
version 5 files do.

A layer's run-length data is the plain `.ctb`'s, covered with its layer cipher under the key
at 0x80. A definition may also name a span of the data enciphered with AES, which the known
files never use and we never write.

## Settings, 288 bytes

The plain `.ctb`'s header, print parameters, slicer info and version 4 block, run together.
Offsets are into the deciphered block.

| Offset | Field |
|---|---|
| 0x00 | checksum, `u64` |
| 0x08 | layer pointers address, `u32` |
| 0x0C | display width, height, machine Z, `f32` each, mm |
| 0x18 | two unknown words |
| 0x20 | total height (0, as known files carry it), layer height, exposure, bottom exposure, light off delay, `f32` each |
| 0x34 | bottom layers, resolution X, Y, layer count, `u32` each |
| 0x44 | large and small preview address, print time in s, projector type, `u32` each |
| 0x54 | bottom lift, bottom lift speed, lift, lift speed, retract speed, `f32` each, mm and mm/min; each lift is both stages' whole climb |
| 0x68 | volume in ml, weight in g, cost, the bottom layers' own light off delay, `f32` each |
| 0x78 | 1, `u32` |
| 0x7C | light PWM, bottom light PWM, `u16` each |
| 0x80 | layer cipher key, `u32` |
| 0x84 | second stage: bottom lift, its speed, lift, its speed, retract, its speed, `f32` each |
| 0x9C | rest before lift, `f32`, s |
| 0xA0 | machine name address, length, `u32` each |
| 0xA8 | the plain `.ctb`'s slicer flags, `u32`: 7 in the low byte, with grey or without; `0x50` in the top byte only where a layer differs from these settings, a transition ramp not counted |
| 0xAC | modified time in minutes since the epoch, anti-alias level (4 with it off), `u32` each |
| 0xB4 | rest after retract, rest after lift, `f32` each, s |
| 0xBC | transition layers, `u32` |
| 0xC0 | bottom retract speed, its second stage, `f32` each |
| 0xC8 | edge band width in mm and its exposure in s, then the bottom layers' pair, `f32` each: `0`, `4.0`, `0`, `4.0`, the band off |
| 0xD8 | rest after retract, after lift, before lift, `f32` each, s: the ones we read |
| 0xE4 | second-stage bottom retract, `f32` |
| 0xE8 | the plain `.ctb`'s version 4 totals: every layer's exposure, light off and rests added up `f32` s, the largest layer's data bytes, 0, the layer count, `u32` each |
| 0xF8 | four padding words |
| 0x108 | disclaimer address, length, then 1, `u32` each |
| 0x114 | resin parameters address, `u32`, then two padding words |

## Layer pointer, 16 bytes

The definition's offset within its page, the page, 88, and 0.

## Layer definition, 88 bytes

| Offset | Field |
|---|---|
| 0x00 | 88, `u32` |
| 0x04 | Z, exposure, light off delay (the bottom one on a bottom layer), `f32` each |
| 0x10 | data offset within its page, the page, data length, unknown, `u32` each |
| 0x20 | AES span offset and length within the data, `u32` each; zero for none |
| 0x28 | lift (both stages' whole climb), lift speed, second-stage lift and speed, retract speed, second-stage retract and speed, `f32` each; a bottom layer's own on a bottom layer |
| 0x44 | rest before lift, after lift, after retract, `f32` each |
| 0x50 | light PWM, `f32` |
| 0x54 | unknown, `u32` |

## Resin parameters, 40 bytes

Padding, the colour (zero, as known files carry it), then the address and length of the resin
type, the resin name and the preset's name, the density in g/ml and padding; the three strings
follow, as in the plain `.ctb`. Version 4 has no such block and its address is zero.

## What we write differently from known files

The layer cipher key is fixed and non-zero; the previews are our usual sizes, where known
files carry one picture twice.
