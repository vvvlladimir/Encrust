# The `.cws`

What the Nova3D Bene and Elfin read: a plain zip holding one settings entry, one eight-bit
greyscale PNG per layer and one gcode program. Five machines in the shipped catalogue carry
`output = "cws"`.

It is the third archive container we write, beside `.sl1` and the `.zip` of greyscale PNGs,
and the one whose settings and program are separate: `slice.conf` is what the firmware reads
the stack's shape from, and the program is what it executes.

Two further variants of the extension exist — one holding 24-bit images and one holding a
`manifest.xml` — and are a different firmware's file; a source profile naming either waits
for a later step (ADR 0170).

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| An open-source reader and writer of the archive | The entry names, every key of `slice.conf`, and the keywords of the program |
| The same reader's gcode builder | That a wait is milliseconds, a feed rate millimetres a minute, and the moves relative |

Three things worth stating before the tables:

1. **The moves are relative**, after `G91`. A layer lifts by its lift distance and comes
   back down by that distance less one layer height, so the difference of the two is the
   only thing that raises the plate.
2. **A wait is milliseconds**, in both the settings and the program: `;<Delay> 30000` is a
   thirty-second exposure.
3. **The images are found by their numbers, not their names.** The firmware shows them in
   the order their numbers put them, so the stem a writer chose does not reach a reader. Ours
   is `encrust`, and a stem may not end in a digit or it would run into the number.

## Shape of the archive

```
slice.conf          the settings, `key = value` a line
encrust0000.png     one eight-bit greyscale PNG per layer, numbered from zero
encrust0001.png
...
encrust.gcode       the program the board executes
```

Entries are stored rather than deflated: a PNG is already compressed and the two text
entries are small.

## `slice.conf`

Two comment lines — the first names what wrote the file — a blank line, and then the keys,
each padded to twenty-four characters before its `=`.

| Key | Unit | What it is |
|---|---|---|
| `xppm`, `yppm` | pixels/mm | the reciprocal of the pixel pitch |
| `xres`, `yres` | pixels | the panel |
| `thickness` | mm | the layer height the header states |
| `layers_num` | — | layers in the stack |
| `head_layers_num` | — | the bottom block's count |
| `layers_expo_ms` | ms | normal exposure |
| `head_layers_expo_ms` | ms | bottom exposure |
| `wait_before_expo_ms` | ms | the light-off delay |
| `lift_distance` | mm | lift |
| `lift_up_speed`, `lift_down_speed` | mm/min | lift and retract |
| `lift_when_finished` | mm | how far the plate rises when the print is done |

## The program

A settings header in comments, `;(Key = value)` a line, in three sections — the stack, the
machine, and what wrote the file. The firmware ignores all of it and a slicer reads it back,
so the keys are spelt as the container spells them, spaces included. The last section is ours:
it carries the slicer's name, the resin and the two light PWM values.

Then the opening moves, one block per layer, and the closing moves:

```gcode
G28 ;Auto Home
G21 ;Set units to be mm
G91 ;Relative Positioning
M17 ;Enable motors
;<Slice> Blank
M106 S0

;<Slice> 0
M106 S255 ;UV on
;<Delay> 30000
M106 S0 ;UV off
;<Slice> Blank
G1 Z5.000 F120
G1 Z-4.950 F120
;<Delay> 1000

M106 S0 ;UV off
G1 Z5.000 F120
M18 ;Disable Motors
;<Completed>
```

`;<Slice> N` shows layer `N`'s image and `;<Slice> Blank` blanks the panel. Everything in a
block is that layer's own, so a banded exposure and a stack of mixed thicknesses are written
as they stand, as in the other gcode archive (ADR 0167).

A reader of ours takes a layer's exposure from the wait between the `M106` that lights the
panel and the one that puts it out, and its Z from the relative moves added up; a block runs
from the keyword that shows its image to the wait after its peel, so the closing move falls
outside every layer.
