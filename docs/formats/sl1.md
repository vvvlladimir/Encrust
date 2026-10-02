# The Prusa `.sl1` container

What a Prusa SL1 and SL1S Speed read, and what the vendor's own slicer exports for them. Alone among
the formats we write it is not a binary container at all: a zip of two settings files, two
previews and one PNG per layer.

| We write | Machine |
|---|---|
| `.sl1` | Original SL1 |
| `.sl1s` | SL1S Speed |

The archive is the same either way; each names its own machine in `printerModel` and its own
`sla_archive_format`, which is what a reader takes the machine's tilt times from.

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| Open-source readers of the archive | The keys of both settings files, how a reader finds the layers, and how it derives the stack height |
| A reference `.sl1s` and the `.sl1` it converts to | The entry names, the PNG shape, and which keys a real file actually carries |

Three things the reference file settles:

1. **The stack's height is not stated.** A reader takes it as `numSlow + numFast`, and
   the vendor's own slicer writes the whole count into `numFast` with `numSlow` at zero.
2. **A layer is found by the shape of its name**, not by the name itself: the pattern is
   five digits then `.png`, with any prefix. The vendor's slicer uses the job's own name,
   and so do we, but a reader does not require it — and the two previews are safe from the pattern
   because `thumbnail400x400.png` does not end in five digits.
3. **Layers are eight-bit greyscale PNGs**, one channel, no palette and no alpha.

## Shape of the archive

```
config.ini                       what the machine's own firmware reads
prusaslicer.ini                  what a slicer reads back
thumbnail/thumbnail400x400.png   preview, colour
thumbnail/thumbnail800x480.png   preview, colour
<job>00000.png                   layer 1, eight-bit greyscale
...
<job>NNNNN.png                   layer N
```

We write the entries in the opposite order — previews, then layers, then the two settings
files. One of the settings is the resin the stack came to, which is only known once the last
layer is cut, and an archive entry cannot be rewritten once it is closed. A reader looks
entries up by name, so the order does not reach it.

Entries are stored rather than deflated: a PNG is already compressed, and deflating it a
second time costs time for nothing.

## `config.ini`

The file the firmware reads. `key = value`, one per line.

| Key | What we write |
|---|---|
| `action` | `print` |
| `jobDir` | the output file's own stem, which the layer entries are named after |
| `expTime` | seconds, the exposure of the whole stack |
| `expTimeFirst` | seconds, the bottom block's |
| `expUserProfile` | 0 |
| `fileCreationTimestamp` | UTC |
| `hollow` | 0 |
| `layerHeight` | mm, the height actually sliced at |
| `materialName` | `MaterialProfile::name` |
| `numFade` | layers the bottom block fades to the rest over, which is our transition layers |
| `numFast` | the stack's whole layer count |
| `numSlow` | 0 |
| `printProfile`, `prusaSlicerVersion` | what names us |
| `printTime` | seconds |
| `printerModel` | `SL1` or `SL1S` |
| `printerProfile` | `PrinterProfile::machine_name` |
| `printerVariant` | `default` |
| `usedMaterial` | ml |

## `prusaslicer.ini`

What a slicer reads the machine back out of. The vendor's own writes about 136 keys here,
most of them its support and pad settings; we write the ones we have a real value for. An unknown
key is one a reader skips, but a key we invented a value for is one it believes.

The machine: `printer_technology` (`SLA`), `printer_model`, `printer_variant`,
`printer_vendor`, `printer_settings_id`, `sla_archive_format`, `display_width`,
`display_height`, `display_pixels_x`, `display_pixels_y`, `display_orientation`,
`display_mirror_x`, `display_mirror_y`, `max_print_height`, `bed_shape`, `thumbnails`.

The print: `layer_height`, `initial_layer_height`, `exposure_time`,
`initial_exposure_time`, `faded_layers`, `material_name`, `material_density`.

`display_mirror_x` and `display_mirror_y` come from the printer profile and the masks go in
as they stand, as everywhere else (ADR 0134).

## What the container cannot carry

There is **no per-layer table of any kind**. One exposure and one layer height hold for the
whole stack, and the only thing that varies is the bottom block, named by a count and faded
by a ramp the firmware computes. So a job whose exposure is banded by height, or whose stack
is of mixed thicknesses, is refused rather than written flat: see
`FormatError::FixedForWholeStack` and `docs/decisions/0148`.

There is also no lift, retract or light-off anywhere in the archive. These are tilting-vat
machines and the firmware owns the motion; some readers smuggle their own values through
`material_notes`, which is a reader's convention rather than Prusa's, and we do not write
them.
