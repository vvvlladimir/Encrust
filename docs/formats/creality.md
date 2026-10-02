# The Creality `.cxdlp`

What the Halot line reads: one extension, two containers. Version 3 holds a layer as a list
of vertical lines and is big-endian throughout; version 4 holds it as the seven-bit
run-length runs of the Chitu family and is little-endian behind its magic. Twelve machines
in the shipped catalogue carry `output = "cxdlp3"` and two `output = "cxdlp4"`.

Both are headed by `CXSW3DV2\0` and end with a checksum, which is the only thing they have
in common; the field behind the magic is what a reader picks on (ADR 0168).

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| An open-source reader and writer of both revisions | Every offset and field name below, and the packing of a line |
| The same reader's checksum routine | That it is a reflected CRC-32 with neither an inverted register nor a final xor |

Three things worth stating before the tables:

1. **The model code is matched by the firmware.** The header carries `CL-60`, `CL-89L`,
   `CT-005` and the like, picked out of the machine's name; a profile whose name holds no
   `CL` or `CT` code is refused rather than guessed at.
2. **An exposure is in tenths of a second in version 3** while the bottom exposure beside
   it is in whole seconds, and the wait before cure has a floor of one second: a zero there
   does not print.
3. **The area fields** state what a layer cures in square millimetres times a thousand. A
   vendor slicer writes the largest island; we write everything the layer lights, because a
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
| — | u32 + UTF-16BE | panel width in millimetres, as text |
| — | u32 + UTF-16BE | panel height in millimetres, as text |
| — | u32 + UTF-16BE | layer height in millimetres, as text |
| — | u16 | exposure, tenths of a second |
| — | u16 | wait before cure, seconds, at least 1 |
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
corrections in the version 3 block — distortion, XY and Z compensation, the grey range and
the firmware's own blur — are all written off: ours are already in the masks, see
`docs/design/compensation.md`. Those four fields are little-endian where the rest of the
file is big, which is the container's own inconsistency and not a mistake of ours.

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
`docs/formats/chitu.md` describes the two blocks and the layer codec; what differs:

| Offset | Type | Field |
|---|---|---|
| 0x00 | u32 BE, 9 bytes, u16 BE | the magic, its length and the revision, 4 |
| — | u32 BE + bytes | model code, counted with its nul |
| — | u16, u16 | panel width and height, pixels |
| — | f32 ×3 | build volume x, y, z, millimetres |
| — | f32, f32 | print height, layer height |
| — | u32 | bottom layer count |
| — | u32 ×6 | small preview, layer table, layer count, large preview, print time, projector type |
| — | u32, u32 | print parameters offset and its size, 68 |
| — | u32 | grey passes, always 1 |
| — | u16, u16 | light PWM, bottom light PWM |
| — | u32 | encryption key, always 0 |
| — | u32, u32 | slicer block offset and its size, 76 |

The previews are the Chitu family's run-length RGB15 records at 120×120 and 300×300,
smallest first. The print parameters and the slicer block are the `.ctb` blocks of the same
names at 68 and 76 bytes. A row of the layer table is 40 bytes — Z, exposure, light-off,
address, size, data type, centroid distance, largest area, two reserved words — and the
address points at a 44-byte motion block whose eleven floats carry this layer's own lift,
retract and rests, with the run-length data behind it. **The size field counts that block
and the data together**, which is the opposite of the `.ctb` convention.

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
