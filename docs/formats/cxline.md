# The `.cxline`

What the PioNext D158 and D160 read: the `.cxdlp` version 3 header, previews and settings
(`creality.md`) with a layer held as polygons rather than lines, and the family's footer and
checksum. Written and read by `format-creality`'s `cxline` module.

Read off the vendor's own slicer, statically: its writer for this container, the lambdas that
place a point, and the table that names the model. No file has been printed from yet.

## Shape of a file

Big-endian throughout.

| Field | What we write |
|---|---|
| `u32` 9, then `CXLINEV2\0` | the magic |
| `u16` | revision, 1 |
| `u32` + bytes | model code, counted with its nul: `CD60` for the D158, any other name as it stands, the D160's included |
| `u16` ×3 | layer count, panel width and height in pixels |
| 64 bytes | the machine block, below |
| three previews | 116×116, 290×290, 290×290, raw RGB565, each closed by `0D 0A`, as `.cxdlp` |
| `u32` + UTF-16 ×3 | panel width and height in mm, layer height in mm, as `%f` text: `158.699997` |
| `u16` ×11 | zero, light-off delay s, bottom exposure s, bottom layers, bottom lift mm, bottom lift mm/s, lift mm, lift mm/s, retract mm/s, bottom light PWM, light PWM |
| `u32` × layers, `0D 0A` | each layer's area field |
| 8 bytes, `0D 0A` | the time compensation block: zero, as with it off |
| the layers | below |
| `u32` 7, then `CXSW3D\0` | the footer |
| `u32` | checksum, as `.cxdlp`'s |

The settings are whole numbers, rounded; the vendor writer truncates them.

### The machine block

| Offset | Field | What we write |
|---|---|---|
| 0x00 | build height, `u32` | µm |
| 0x04 | shrinkage compensation on, `u32` | 0: ours is in the masks |
| 0x08 | support time offset, `f32` | 0 |
| 0x0C | firmware blur, `u32` | 0 |
| 0x10 | exposure, `u16` | tenths of a second |
| 0x12 | volume, `f32` | ml, written again once the stack is measured |
| 0x16 | the rest | zero |

## A layer

`u32` area field, `u32` ring count, then each ring as a `u32` point count and that many
points of two `i32`, x then y, in micrometres from the panel's corner; then `0D 0A`. A ring
ends on its first point again. The area field is what the layer lights in mm² times a
thousand.

The vendor writer places a point at `x + width / 2`, `y + height / 2` of its plate-centred
contour, `height / 2 − y` when the projector mirrors; ours are traced out of the mask
already mirrored, so the corner coordinates go as they are. Rings are traced with material on
one side and holes the other way round, as for `.svgx`; which rule the firmware fills by is
not known.

## What we do not write

Grey, which a polygon has none of; per-layer settings, likewise. The vendor writer's preview
words drop some of green's bits; ours are the plain RGB565 of `.cxdlp`.
