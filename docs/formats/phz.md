# The `.phz`

What the Phrozen Sonic Mini reads: the `.fdg` layout (`fdg.md`) — its previews, machine name,
layer table, per-layer records and one-bit codec, in the same order — under a header of its
own and another cipher. Written by `format-chitu`'s `phz` module, through the `.fdg` sink.

Read off the vendor's own slicer, statically: its writer for this container runs the `.fdg`
writer's layer loop and encoder instruction for instruction. No file has been printed from yet.

## Header

0x134 bytes, little-endian. Offsets hexadecimal.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | magic, version, `u32` each | `0x9FDA83AE`, 4 |
| 0x08 | layer height, exposure, bottom exposure, `f32` each | mm, s, s |
| 0x14 | bottom layers, resolution X, Y, `u32` each | |
| 0x20 | large preview, layer table addresses, `u32` each | |
| 0x28 | layer count, `u32` | |
| 0x2C | small preview address, `u32` | |
| 0x30 | print time, `u32` | seconds |
| 0x34 | projector type, `u32` | 1 when either mirror flag is set, as in `.ctb` |
| 0x38 | anti-alias level, `u32` | as `.fdg`'s at 0x60 |
| 0x3C | light PWM, bottom light PWM, `u16` each | |
| 0x40 | two words | zero, as `.ctb`'s two at 0x14 |
| 0x48 | total height, `f32` | mm |
| 0x4C | bed X, Y, Z, `f32` each | `PrinterProfile::build_volume` |
| 0x58 | encryption key, `u32` | 0: the layers in the clear (ADR 0046) |
| 0x5C | bottom light-off delay, light-off delay, `f32` each | |
| 0x64 | bottom layers again, `u32` | |
| 0x68 | `u32` | zero |
| 0x6C | bottom lift, bottom lift speed, lift, lift speed, retract speed, `f32` each | mm, mm/min; a lift is both stages summed |
| 0x80 | volume ml, weight g, cost, `f32` each | cost zero |
| 0x8C | `u32` | zero, a word of the second stage |
| 0x90 | machine name address, length, `u32` each | |
| 0x98 | the second lift and retract stage, six words | zero |
| 0xB0 | `u32` | `0x14`, a constant of the vendor writer |
| 0xB4 | modified timestamp, `u32` | minutes since the Unix epoch |
| 0xB8 | anti-alias level, `u32` | the same again |
| 0xBC | software version, `u32` | `0x01090000`, as `.ctb` v4 |
| 0xC0 | the rest | as `.fdg` from the same offset to its end |

## Cipher

As `.fdg`'s, word by word over each layer's bytes, with other seeds:
`step = (key mod 0x4324) · 0x34A32231` and
`word = step · ((layer index ^ 0x3FAD2212) · 0x1E1530CD)`, then `word += step` for each next
four bytes. We write key zero and read either.

## What we do not write

Grey and the second lift stage, as for `.fdg`. The vendor writer fits each preview into
500 × 500 from whatever picture it is handed; ours are the `.fdg` sizes, which fit.
