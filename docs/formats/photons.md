# The `.photons`

What the Anycubic Photon S reads: a big-endian header of doubles, one raw preview and a
layer codec of one bit a pixel. Written and read by `format-anycubic`'s `photons` module.
It shares nothing with the Photon Workshop container of `anycubic.md` but the maker.

No file has been printed from this layout yet.

## Shape of a file

```
tag               6 bytes
header            0x5C bytes
preview           224 x 168, two bytes a pixel
layer count       u32
layer 0           0x1C-byte record, then its bytes
...
```

Nothing points at anything: a reader walks the records, each stating the length of its data.

## Header

Big-endian. Offsets from the start of the file.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | `u32`, `u16` | 2, then `0x31`: the tag |
| 0x06 | pixel size, `f64` | mm, the panel's width over its pixels |
| 0x0E | layer height, exposure, light-off delay, bottom exposure, `f64` each | mm, s |
| 0x2E | bottom layers, `u32` | |
| 0x32 | lift distance, lift speed, retract speed, `f64` each | mm, mm/s |
| 0x4A | volume, `f64` | ml |
| 0x52 | preview width, `u32` | 224 |
| 0x56 | `u32` | 42, a constant |
| 0x5A | preview height, `u32` | 168 |
| 0x5E | `u32` | 10, a constant |

The volume is only known once the last layer is written, so the header is written again then.

## Preview

224 × 168 little-endian words, row after row: five bits each of red at bit 11, green at bit 6
and blue at bit 0, bit 5 clear — the `.ctb` preview's colour (`chitu.md`) without its runs.

## A layer

The record, big-endian:

| Offset | Field |
|---|---|
| 0x00 | lit pixels, `u32`: a stretch is counted once the next begins, so a layer's last stretch is never counted |
| 0x04 | two words, zero |
| 0x0C | panel width, height, `u32` each |
| 0x14 | length of the data in bits, plus 32, `u32` |
| 0x18 | panel width, height, `u16` each, each byte's bits reversed |

The data runs over the whole layer, row after row, as stretches of one colour: a grey from 128
up is lit. A stretch of `n` pixels is bytes of `(lit << 7) | (count − 1)` for counts of at most
128, each byte's bits then reversed, so bit 0 of the stored byte is the colour.

## What we do not write

Grey, which the container has no room for; per-layer settings, likewise. The preview is
fitted into the 224 × 168 without stretching.
