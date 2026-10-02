# The Chitu container family

What machines running Chitu Systems firmware read, and what the vendor slicer exports by
default.
Two containers, one lineage:

| We write | Magic | Version | Grey |
|---|---|---|---|
| `.ctb` | `0x12FD0106` | 4 or 5 | seven bits per pixel, one pass |
| `.cbddlp`, `.photon` | `0x12FD0019` | 2 | one bit per pixel, eight passes |

They share the header, both previews and the print parameters byte for byte. What the older
one lacks is the slicer info block, the version 4 block and the extended block in front of
each layer — and with them per-layer motion, which is why it carries exposure and nothing
else per layer. The two extensions of the older container are the same bytes; only the name
differs, and it is the name a machine's firmware looks for.

We do not parse whole files; we decode the run-length data of a layer, to check what we
wrote.

## Where the knowledge comes from

There is no published specification. Both sources below were read for this implementation
and they agree except where noted.

| Source | What it settles |
|---|---|
| Open-source readers of the container | Field order, types, block order, the addresses each block holds, the run-length codec, the values a vendor file carries |
| [`cbiffle/catibo`, `doc/cbddlp-ctb.adoc`](https://github.com/cbiffle/catibo/blob/master/doc/cbddlp-ctb.adoc) | The same for version 2, and the meaning of the fields both versions share |

Where they disagree, the maintained reader wins: it is the one that still writes files real
machines print. The header is the one place it matters — `catibo` lists one padding word
where there are two, so every offset from the previews onwards sits four bytes further on
than that document says. It reads as a difference between versions, and it is not: a
version 2 `.cbddlp` written today has the two words at `0x14` and the total height at
`0x1C`, exactly where version 4 has them.

Three things neither source spells out and that cost a layer table each:

1. **Everything is little-endian, except a `.ctb` run length.** The container is
   little-endian throughout; the bytes of a `.ctb` run length are big-endian. See
   *Run-length data, `.ctb`*.
2. **The header cannot be written first.** Half of its fields are offsets into blocks that
   follow it, and the layer table is a fixed-width table whose rows are only known once
   the layers have been compressed. Space is left for both and they are filled in
   afterwards, which is why every sliced-file sink is seekable; see
   `docs/decisions/0045-sliced-files-are-written-to-a-seekable-sink.md`.
3. **A block's address is stated by the block in front of it**, not by the header. The
   header points at the slicer info block; that points at the version 4 block; that points
   at the resin block. A reader that loses one of those addresses cannot find what follows.
   The older container has none of those three, so its header addresses everything it has.

## Shape of a file

A `.ctb`:

```
header                112 bytes, fixed
large preview         32-byte record, then RGB15 run-length data
small preview         32-byte record, then RGB15 run-length data
print parameters      60 bytes
slicer info           76 bytes, then the machine name
disclaimer            320 bytes of text
print parameters v4   464 bytes
resin parameters      40 bytes, then three strings   (version 5 only)
layer table           36 bytes per layer
layer 1               84-byte extended block, then run-length data
...
layer N
```

The blocks between the header and the layer table are written in that order because each
one's address is derived from the end of the one before it.

A `.cbddlp` or `.photon` stops after the print parameters:

```
header                112 bytes, fixed
large preview         32-byte record, then RGB15 run-length data
small preview         32-byte record, then RGB15 run-length data
print parameters      60 bytes
layer table           36 bytes per pass per layer, pass-major
layer 1               eight passes of one-bit run-length data, back to back
...
layer N
```

## Header

Offsets are hexadecimal, from the start of the file.

| Offset | Field | What we write |
|---|---|---|
| 0x00 | magic, `u32` | `0x12FD0106` for `.ctb`, `0x12FD0019` for `.cbddlp` and `.photon` |
| 0x04 | version, `u32` | 4 or 5 for `.ctb`, 2 for the older container |
| 0x08 | bed X, Y, Z, `f32` each | `PrinterProfile::build_volume`, mm |
| 0x14 | two unknown words | zero |
| 0x1C | total height, `f32` | layer count times layer height, mm |
| 0x20 | layer height, `f32` | the height actually sliced at, not the profile's |
| 0x24 | exposure, `f32` | seconds |
| 0x28 | bottom exposure, `f32` | seconds |
| 0x2C | light off delay, `f32` | seconds |
| 0x30 | bottom layers, `u32` | |
| 0x34 | resolution X, Y, `u32` each | from `RasterSettings` |
| 0x3C | large preview address, `u32` | 112: the preview follows the header |
| 0x40 | layer table address, `u32` | |
| 0x44 | layer count, `u32` | |
| 0x48 | small preview address, `u32` | |
| 0x4C | print time, `u32` | seconds, from `PrintJob::print_time_s` |
| 0x50 | projector type, `u32` | 1 when either mirror flag is set, else 0 |
| 0x54 | print parameters address, `u32` | |
| 0x58 | print parameters size, `u32` | 60 |
| 0x5C | anti-alias level, `u32` | 1 for `.ctb`, 8 for the older container, see *Anti-aliasing* |
| 0x60 | light PWM, bottom light PWM, `u16` each | |
| 0x64 | encryption key, `u32` | 0: the layer data is written in the clear, see `docs/decisions/0046` |
| 0x68 | slicer info address, `u32` | zero when there is no such block |
| 0x6C | slicer info size, `u32` | 76, the block **without** the machine name; zero for the older container |

### Anti-aliasing

An anti-alias level above one means the file carries that many one-bit passes of every
layer, which the printer sums into a grey value.

`.ctb` needs none of that: its runs carry seven bits of grey, so we write level 1 and one
pass, and stacking passes would cost several times the bytes for a worse result. The older
container has no grey of its own and the passes are the only way to carry any, so it is
written at level 8; see `docs/decisions/0146`.

`slicer info` carries an anti-alias *flag* as well, which is `0x0F` when grey is in use.
That is the one the firmware looks at; the level is what a reader displays. The older
container has no slicer info block, so the header's level is all it states.

### Mirroring

`projector type` says how the panel is mounted: it is set when `PrinterProfile::mirror_x`
or `mirror_y` is, exactly as for `.goo`. The masks themselves carry the plate as it stands
(ADR 0134).

## Preview records

Two, 400 x 300 and 200 x 125 pixels, in that order. Each is a 32-byte record:

| Offset | Field |
|---|---|
| 0x00 | width, height, `u32` each |
| 0x08 | data address, `u32`: the record's own end |
| 0x0C | data length, `u32` |
| 0x10 | four unknown words, zero |

The pixels are RGB15 with a repeat flag, not RGB565: the colour word is
`rrrrrggg gggbbbbb`, and green's lowest bit is stolen as the flag, which leaves five usable
bits in each channel. A word with `0x0020` set is
followed by a count word `0x3000 | (run - 1)`, covering at most `0xFFF` pixels. One or two
pixels of a colour are written out with the flag cleared, because clearing it changes the
colour by one step of green and setting it on a single pixel would be read as a run.

Both previews hold the job's thumbnail, cut to the record's shape from the picture
`core-thumbnail` renders of the plate; a square image in a 4:3 record is padded either side
with black rather than stretched. A job without a thumbnail leaves the pixels black, and
the records are there either way: the header's addresses are not optional.

## Print parameters, 60 bytes

| Offset | Field |
|---|---|
| 0x00 | bottom lift distance, bottom lift speed, `f32` each |
| 0x08 | lift distance, lift speed, retract speed, `f32` each |
| 0x14 | resin volume in ml, weight in g, cost, `f32` each |
| 0x20 | bottom light off delay, light off delay, `f32` each |
| 0x28 | bottom layers, `u32` |
| 0x2C | four padding words |

Cost is written as zero: the material profile carries no price.

## Slicer info, 76 bytes

| Offset | Field | What we write |
|---|---|---|
| 0x00 | second-stage lift and retract, seven `f32` | zero: one stage is enough |
| 0x1C | machine name address, `u32` | the end of this block |
| 0x20 | machine name length, `u32` | |
| 0x24 | anti-alias flag, `u8` | `0x0F` |
| 0x25 | padding, `u16` | |
| 0x27 | per-layer settings, `u8` | `0x40` for version 4, `0x50` for version 5 |
| 0x28 | modified timestamp, `u32` | **minutes** since the Unix epoch |
| 0x2C | anti-alias level, `u32` | 1 |
| 0x30 | software version, `u32` | `0x01090000` for version 4, `0x02000000` for version 5 |
| 0x34 | rest time after retract, after lift, `f32` each | the resin's rests, zero unless it rests |
| 0x3C | transition layers, `u32` | |
| 0x40 | print parameters v4 address, `u32` | past the machine name and the disclaimer |
| 0x44 | two padding words | |
| 0x4C | machine name, not nul-terminated | `PrinterProfile::name` |

The size in the header is 76, the block without the name. The name is part of the block as
far as the file is concerned and is addressed separately, which is why it is not padded to
a fixed width.

*Per-layer settings* is what allows the machine to obey the extended block in front of each
layer rather than the header's own values. Chitu firmware reads it from 4.3.9 on; a
machine known not to is marked `per_layer_settings = false` in its profile (ADR 0090). *Software version* is read as a capability
stamp, so each version names a vendor slicer release that shipped with it.

## Disclaimer, 320 bytes

CBD Technology's copyright notice, verbatim, written between the machine name and the
version 4 block. It is exactly 320 bytes and is not nul-padded. It is a field of the
container, not a claim of ours; the slicers that made the format write it, and the version 4
block addresses it, so leaving it out would move every address behind it.

## Print parameters v4, 464 bytes

| Offset | Field | What we write |
|---|---|---|
| 0x00 | bottom retract speed, second-stage speed, `f32` each | the profile's, then zero |
| 0x08 | padding, `u32` | |
| 0x0C | `4.0`, `f32` | the literal the firmware expects |
| 0x10 | padding, `u32` | |
| 0x14 | `4.0`, `f32` | the same literal again |
| 0x18 | rest after retract, after lift, before lift, second-stage retract height, a word | the rests, then zero |
| 0x2C | unknown, `u32` | zero |
| 0x30 | unknown, `u32` | 5 |
| 0x34 | last layer index, `u32` | layer count minus one |
| 0x38 | four padding words | |
| 0x48 | disclaimer address, `u32` | |
| 0x4C | disclaimer length, `u32` | 320 |
| 0x50 | resin parameters address, `u32` | zero in version 4 |
| 0x54 | reserved, 380 bytes | zero |

## Resin parameters, 40 bytes, version 5 only

| Offset | Field |
|---|---|
| 0x00 | padding, `u32` |
| 0x04 | resin colour B, G, R, A, `u8` each |
| 0x08 | machine name address, `u32` |
| 0x0C | resin type length, address, `u32` each |
| 0x14 | resin name length, address, `u32` each |
| 0x1C | machine name length, `u32` |
| 0x20 | resin density, `f32`, g/cm³ |
| 0x24 | padding, `u32` |

The three strings follow the block in the order resin type, resin name, machine name, each
addressed from the block. We write `UV Resin` as the type and `MaterialProfile::name` as
the name.

## Layer table

One 36-byte record per layer, in print order:

| Offset | Field |
|---|---|
| 0x00 | Z, `f32`: the **top** of the layer |
| 0x04 | exposure, `f32` |
| 0x08 | light off delay, `f32` |
| 0x0C | data address, `u32` |
| 0x10 | data size, `u32`, the runs alone |
| 0x14 | page number, `u32` |
| 0x18 | table size, `u32`: 84 in a `.ctb`, counting the extended block in front of the data; 36, the record alone, in the older container |
| 0x1C | two unknown words |

*Page number* splits addresses past 4 GB, where a `u32` no longer reaches. Nothing this
project writes gets there; a file that did would need its addresses rebased against the
page.

In the older container the table holds one record **per pass per layer**, laid out
pass-major: row `pass * layer_count + layer`. The header's layer count stays the number of
physical layers. Every pass of one layer is written together so that the stack still
streams, which means consecutive rows of the table address addresses far apart in the file.

### The extended block

In front of each layer's run-length data sits 84 bytes: the 36-byte record above, repeated
verbatim, then the motion for this layer alone.

| Offset | Field |
|---|---|
| 0x00 | the layer table record again, 36 bytes |
| 0x24 | total size, `u32`: 84 plus the data size |
| 0x28 | lift distance, lift speed, `f32` each |
| 0x30 | second-stage lift distance and speed, `f32` each |
| 0x38 | retract speed, second-stage retract height and speed, `f32` each |
| 0x44 | rest time before lift, after lift, after retract, `f32` each |
| 0x50 | light PWM, `f32` |

This is what lets exposure, lift and PWM change over the height of a print: a bottom layer
carries the bottom values, a transition layer its own exposure, without the header
changing. The record is repeated because a machine reading forward through the layers never
sees the table again.

## Run-length data, `.ctb`

Runs are taken across the whole image in one pass, row by row from the top left. They do
not restart at a row boundary, and they must cover every pixel of the panel.

The format carries **seven bits of grey**: a value is halved on the way in and, on the way
out, a non-zero value is doubled with its lowest bit set, so `0xFF` survives the round trip
and `0x00` stays black. Two eight-bit values one step apart therefore land on the same
seven-bit value and have to be merged into one run rather than written as two, or a reader
that counts runs disagrees with the writer.

Each chunk opens with the value in its low seven bits. Bit 7 says whether a length
follows; without one the chunk is a single pixel. The length's own first byte says how many
bytes carry it, in its leading ones, and the bytes that follow are **big-endian**:

| First length byte | Run length | Total length bytes |
|---|---|---|
| `0xxxxxxx` | up to `0x7F` | 1 |
| `10xxxxxx` | up to `0x3FFF` | 2 |
| `110xxxxx` | up to `0x1FFFFF` | 3 |
| `1110xxxx` | up to `0xFFFFFFF` | 4 |

`1111xxxx` is not a length and no encoder writes it. A run longer than `0xFFFFFFF` is split
into several chunks.

## Run-length data, `.cbddlp` and `.photon`

One bit per pixel, so a layer is written eight times over. Runs are taken the same way —
across the whole image, row by row, covering every pixel — and one byte carries one run:
bit 7 is the pixel, bits 6:0 the length. The longest run is `0x7D`, 125, not the 127 seven
bits would hold: that is where the reference writer stops and firmware is only known to be
tested against what it writes. A longer run is split into several bytes.

Pass *k* lights a pixel whose value is at least `256 / 8 * k - 1` **as a byte**, so pass
zero wraps to 255 and the thresholds are 255, 31, 63, 95, 127, 159, 191, 223. A machine
counts the passes that lit a pixel and reads `count * 32 - 1` as its value, which puts pass
zero's threshold where it has to be for a fully lit pixel to decode back to 255. Eight-bit
grey therefore lands on the step below it: 0, 31, 63, 95, 127, 159, 191, 223, 255.

The cost is the layer count times eight, and a run-length floor of one byte per 125 pixels
whether they are lit or not: a 4098 x 2560 panel cannot go below 671 kB a layer.

## The layer cipher

A `.ctb`'s run-length data is covered by a keyed exclusive or; a `.cbddlp`'s never is.
The vendor slicer sets the key, and other writers set a random one on every `.ctb`, so a reader
that ignored the header's key field would decode noise from almost any real file.

The keystream is a word derived from the key and the layer's own index, spent a byte at a
time and then advanced:

```
init = key * 0x2D83CDAC + 0xD8A83423
word = (layer_index * 0x1E1530CD + 0xEC3D47CD) * init
```

all as wrapping 32-bit arithmetic. Each byte of the data is exclusive-ored with the next
byte of `word`, least significant first; after four bytes `word += init` and it begins again.
It is symmetric, so one pass both covers and uncovers. Only the runs are covered — not the
extended block in front of them, whose floats are readable in any file.

A key of zero means the data is in the clear, which is what we write.

## What we do not write

- **Encryption.** The header's key is zero and the layer data goes out in the clear; see
  `docs/decisions/0046-ctb-layer-data-is-written-in-the-clear.md`. We read a file whose key
  is set.
- **Deduplicated layers.** A vendor file points two identical layers at one blob of data. We
  write each layer's own bytes, because the stack is streamed a window at a time and
  nothing holds the hashes of every layer written so far.
- **`.ctb` versions 2 and 3, `.cbddlp` version 1, and the encrypted and GKtwo variants.**
  Version 4 is the oldest `.ctb` worth writing: every machine that reads version 5 reads it
  too. A machine whose published profile asks for version 3 therefore ships pointing at
  version 4, which needs firmware from 2021 or later; an older board refuses the file
  rather than misprinting it (ADR 0165). Version 1 of the older container has no print
  parameters block and no anti-aliasing at all, so it cannot carry a grey edge.
