# The `.fdg`

What the Voxelab Ceres 8.9, Polaris 5.5 and Proxima 6 read, Chitu boards all: the `.ctb`
v4 preview records and per-layer records under a header of its own, and a layer codec of
one bit a pixel. Written by `format-chitu`'s `fdg` module.

Read off the vendor's own slicer, statically: its writer for this container, its encoder
and its cipher. No file has been printed from yet.

## Shape of a file

```
header            0x134 bytes
large preview     record, 400 x 300
small preview     record, 200 x 125
machine name      PrinterProfile::machine_name, not nul-terminated
layer table       36 bytes a layer
layer 0           84-byte record, then its bytes
...
```

The preview records and both per-layer records are the `.ctb` ones, byte for byte
(`chitu.md`): the 36-byte row, and the 84-byte block of the row again plus the layer's lift,
speeds, rests and light power.

## Header

Offsets hexadecimal. Words are little-endian.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | magic, `u32` | `0xBD3C7AC8` |
| 0x04 | version, `u32` | 4 |
| 0x08 | layer count, `u32` | |
| 0x0C | bottom layers, `u32` | |
| 0x10 | projector type, `u32` | 1 when either mirror flag is set, as in `.ctb` |
| 0x14 | bottom layers again, `u32` | |
| 0x18 | resolution X, Y, `u32` each | |
| 0x20 | layer height, exposure, bottom exposure, `f32` each | mm, s, s |
| 0x2C | large preview, small preview, layer table addresses, `u32` each | |
| 0x38 | print time, `u32` | seconds |
| 0x3C | `u32` | 1, a constant of the vendor writer |
| 0x40 | bottom light PWM, light PWM, `u16` each | the reverse of `.ctb`'s order |
| 0x44 | two words | zero, as `.ctb`'s two at 0x14 |
| 0x4C | total height, `f32` | 0, as the vendor writes it |
| 0x50 | bed X, Y, Z, `f32` each | `PrinterProfile::build_volume` |
| 0x5C | encryption key, `u32` | never zero, as for `.ctb` (ADR 0365) |
| 0x60 | anti-alias level, `u32` | the level the edges were drawn with, at most 16; the layers stay lit or unlit |
| 0x64 | `u32` | `0x44`, a constant of the vendor writer |
| 0x68 | volume ml, weight g, cost, `f32` each | cost zero without a resin price |
| 0x74 | machine name address, length, `u32` each | |
| 0x7C | bottom light-off delay, light-off delay, `f32` each | |
| 0x84 | `u32` | zero |
| 0x88 | bottom lift, bottom lift speed, lift, lift speed, retract speed, `f32` each | mm, mm/min; a lift is both stages summed |
| 0x9C | second stage: bottom lift, its speed, lift, its speed, retract, its speed, `f32` each | as `.ctb`'s slicer info +0x00 |
| 0xB4 | `u32` | zero; the vendor copies a setting we have not found |
| 0xB8 | modified timestamp, `u32` | minutes since the Unix epoch |
| 0xBC | software version, `u32` | `0x01090000`, as `.ctb` v4 |
| 0xC0 | rest after retract, rest after lift, `f32` each | |
| 0xC8 | transition layers, transition type, `u16` each | the count, then 0 |
| 0xCC | address and length of the block at 0xD8, `u32` each | `0xD8`, `0x50` |
| 0xD4 | `u32` | zero |
| 0xD8 | the first 40 bytes of `.ctb`'s v4 parameters (`chitu.md`) | both bottom retract speeds, the two 4.0 words, rest after retract, after lift, before lift, the second bottom retract |
| 0x100 | exposure, light-off delay and rests over every layer, `f32` | seconds |
| 0x104 | largest layer's byte count, `u32` | |
| 0x108 | index of the last layer below the bottom height, `u32` | |
| 0x10C | layers at a height of their own, `u32` | two heights within 0.001 mm are one |
| 0x110 | nine words | zero |

## Layer bytes

One bit a pixel: a grey from 128 up is lit, anything less is not. The stream runs over the
whole layer, row after row:

- a byte with `0x80` set names a colour and is one pixel of it: `0x80` unlit, `0xFC` lit
  (seven bits of grey, of which the vendor writes only 0 and `0x7C`);
- any other byte is that many more pixels of the current colour, at most `0x7D`.

The first byte stands for the first pixel, and is `0x80` when it is unlit and zero when it
is lit — the vendor writer's own behaviour, kept. A colour byte opens every row and every
row's middle (column `width / 2`), even where the colour runs on across it.

## Cipher

With a key other than zero, each layer's bytes are covered word by word: with
`step = (key + 0xE234893D) ^ 0x257E2431` and
`word = step · ((layer index ^ 0x110BDACD) · 0x82391EFD)`, each little-endian four bytes are
XORed with `word`, then `word += step`; all arithmetic wraps at 32 bits, and a short last
chunk takes the low bytes. We write the key `.ctb` does and read any.

## What we do not write

Grey: the container has room for seven bits a pixel but the vendor writer cuts at 128, and
so do we.
