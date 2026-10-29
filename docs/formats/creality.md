# The Creality `.cxdlp`

What the Halot line reads: two containers, `.cxdlp` and `.cxdlpv4`. Version 3 holds a layer as
a list of vertical lines and is big-endian throughout; version 4 holds it as six-bit runs along
each row and is little-endian behind its magic. Twelve machines
in the shipped catalogue carry `output = "cxdlp3"` and two `output = "cxdlp4"`.

Both are headed by `CXSW3DV2\0` and end with a checksum, which is the only thing they have
in common; the field behind the magic is what a reader picks on (ADR 0168).

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| An open-source reader and writer of both revisions | Every offset and field name below, and the packing of a line |
| The same reader's checksum routine | That it is a reflected CRC-32 with neither an inverted register nor a final xor |

Three things worth stating before the tables:

1. **The model code is matched by the firmware.** The header carries `CL-60`, `R012` and the
   like: the maker's own code for a machine name it lists (`HALOT-X1 Max` is `R012`),
   else a `CL` or `CT` code picked out of the name; a profile with neither is refused rather
   than guessed at. A shipped machine with no listed code (`HALOT-PLAY`, `HALOT-R6`,
   `PIOCREAT C02`) carries its name, as known files do for any unlisted name.
2. **An exposure is in tenths of a second in version 3** while the bottom exposure beside
   it is in whole seconds, and the wait before cure is whole seconds with the fraction
   dropped, as known files carry it.
3. **The area fields** state what a layer cures in square millimetres times a thousand. Known
   files state the largest island; we write everything the layer lights, because a
   writer of ours is handed runs and not contours. Nothing in the firmware is known to read
   it beyond showing it.

## Version 3

Everything is big-endian unless a field says otherwise.

| Offset | Type | Field |
|---|---|---|
| 0x00 | u32 | magic length, 9 |
| 0x04 | 9 bytes | `CXSW3DV2\0` |
| 0x0D | u16 | revision, 3 |
| 0x0F | u32 + bytes | model code, counted with its nul |
| — | u16 | layer count |
| — | u16, u16 | panel width and height, pixels |
| — | 64 bytes | zero |
| — | — | three previews: 116×116, 290×290, 290×290, raw big-endian RGB565, each closed by `0D 0A` |
| — | u32 + UTF-16BE | panel width in millimetres, as text to six decimals |
| — | u32 + UTF-16BE | panel height in millimetres, the same |
| — | u32 + UTF-16BE | layer height in millimetres, the same |
| — | u16 | exposure, tenths of a second |
| — | u16 | wait before cure, whole seconds |
| — | u16 | bottom exposure, seconds |
| — | u16 | bottom layer count |
| — | u16 ×5 | bottom lift mm, bottom lift mm/s, lift mm, lift mm/s, retract mm/s |
| — | u16, u16 | bottom light PWM, light PWM |
| — | u32 × layers | each layer's area, then `0D 0A` |
| — | — | the version 3 block: slicer name and resin name, both counted with their nul, then the corrections, then `0D 0A` |
| — | — | the layers |
| — | u32 + 9 bytes | the magic again, as a footer |
| — | u32 | checksum |

The speeds are millimetres a second where our profiles hold millimetres a minute. The
corrections in the version 3 block — distortion, XY and Z compensation, each a flag and a
little-endian value — are written off, ours being in the masks
(`docs/design/compensation.md`). Five bytes follow: the anti-alias level (4 with it off),
0, 0, 1 and the blur (2 with none), the bytes known files carry.

### A layer

A layer is `u32` area, `u32` line count, that many six-byte lines, then `0D 0A`. One line is
a run down a column:

```
byte 0: startY >> 5
byte 1: (startY << 3) + (endY >> 10)
byte 2: endY >> 2
byte 3: (endY << 6) + (startX >> 8)
byte 4: startX
byte 5: grey
```

`startY` and `endY` are thirteen bits each and `startX` is fourteen, so the panel can be at
most 16383×8191; `endY` is the last row the line covers, not the row after it. A line
carries all eight bits of its grey, and the encoder walks X and runs along Y — across our
own runs, which is why this is the one codec that expands a mask to pixels first.

## Version 4

The same magic and revision field, then the tables of the `.ctb` family, little-endian.
`docs/formats/chitu.md` describes the two blocks; what differs, checked against a known
file for the HALOT X1 Max:

| Offset | Type | Field |
|---|---|---|
| 0x00 | u32 BE, 9 bytes, u16 BE | the magic, its length and the revision, 4 |
| — | u32 BE + bytes | model code, counted with its nul |
| — | u16, u16 | panel width and height, pixels |
| — | f32 ×3 | build volume x, y, z, millimetres |
| — | f32, f32 | print height, layer height |
| — | u32 | bottom layer count |
| — | u32 ×6 | small preview, layer table, layer count, large preview, print time, projector type (0, as known files carry it on a mirrored panel too) |
| — | u32, u32 | print parameters offset and its size, 68 |
| — | u32 | anti-alias level, 4 with it off, as known files carry it |
| — | u16, u16 | light PWM, bottom light PWM |
| — | u32 | encryption key, 0, as known files carry it here |
| — | u32, u32 | slicer block offset and its size, 76 |

The previews are the Chitu family's run-length RGB15 records at 120×120 and 300×300,
smallest first. The print parameters and the slicer block are the `.ctb` blocks of the same
names at 68 and 76 bytes. The print parameters end at 0x34 with the HALOT buffer: height in mm,
speed in mm/min, the wait after it in s, `f32` each, then `u32` 1 when it is on (ADR 0370). A row of the layer table is 40 bytes — Z, exposure, light-off,
address, size, data type 0, centroid distance, the area the layer cures in whole mm², two
reserved words — and the address points at a 44-byte motion block whose eleven floats carry this layer's own lift,
retract and rests, with the run-length data behind it. **The size field counts that block
and the data together**, which is the opposite of the `.ctb` convention.

### A version 4 layer

Runs along each row, restarting at every row. A run opens with `0x80 | g`, `g` its grey in six
bits (`0x3F` fully lit, read back as `g << 2 | g >> 4`), then its length less one in seven-bit
groups, most significant first; a run of one pixel has no group. A dark row of the X1 Max's
15120 pixels is `80 76 0F`.

A file ends with the same four-byte checksum.

## The checksum

A reflected CRC-32 over every byte of the file before the four the checksum occupies, with
the register starting at zero and no final inversion — so `123456789` sums to `0x2DFD2D88`
rather than the standard `0xCBF43926`. It is written big-endian.

A writer of ours never reads the file back, and both revisions leave a table in the middle
of it that is only known once the last layer has landed. The sum is therefore taken over
each region as its bytes become final and the regions are joined afterwards: a CRC is linear
over GF(2), so running a known number of zero bytes through the register is a matrix, and
squaring it repeatedly reaches any length in a few dozen steps.
