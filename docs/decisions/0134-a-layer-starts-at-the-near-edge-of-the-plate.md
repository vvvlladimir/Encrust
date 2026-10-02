# 0134. Write the plate as it stands, and leave mirroring to the header

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

`PixelSpace` mapped model Y onto rows the way a screen draws an image — row `0` was the far
edge of the plate — and `core-pipeline` baked `PrinterProfile::mirror_x` into every mask it
wrote. A plate arranged with a part at the right and far corner therefore reached the file
mirrored in X and with its rows the other way up from what the vendor slicer writes for the
same machine.

The difference is not academic: a vendor file for the ELEGOO Saturn 4 Ultra prints
where the part was put, and ours printed somewhere else. Opened side by side, its layer
sits in one corner and ours in another.

That file's header still states `mirror_x = 1, mirror_y = 0` for that machine, and so does
our profile. The flag is therefore not a description of the pixels: it records how the panel
is mounted, which is what a viewer needs in order to show the layer the way the plate stood.

## Decision

**A written mask is the plate seen from above, row `0` at its near edge.** `PixelSpace::map`
is `y_px = y_mm / pitch.y`, with no inversion, and `core_pipeline::raster_settings` asks for
no mirroring at all. `reverses_winding` is the parity of the two mirrors rather than of three
reflections.

**The mirror flags come from the printer profile, not from the mask.** `.goo` writes
`PrinterProfile::mirror_x` and `mirror_y`, and `.ctb`'s projector type is set from the same
pair, whatever the masks were drawn with.

`RasterSettings::mirror_x` and `mirror_y` stay: they are how a caller asks for a layout. The
preview asks for `mirror_y`, because a picture starts at the far edge of the plate where a
file starts at the near one — `Preview::as_seen`, which already cleared the panel mirroring
(ADR 0109), now sets it.

## Consequences

A plate carried across from a vendor slicer prints in the same corner of the vat, and both files
look the same in a reader.

`core-supports` no longer turns the rasteriser's runs the right way up, so the row order is
restated in one fewer place. Every test that named a corner had to be reread, and the mask
pane draws the plate the way the viewport beside it does.

The profile's mirroring now affects only two header fields. If a machine turns up that wants
the pixels mirrored as well, that is a property of the file it reads, and belongs to the
writer rather than to the map — reopen this then.

## Alternatives considered

### Keep baking the mirror into the mask

What we had, and what the flag's old comment claimed. Rejected because the print came out in
the wrong corner: the machines this project ships profiles for take the plate as it stands.

### Mirror in the writers instead of the map

Each format would state the rule, and a streaming writer would have to hold a whole layer to
reverse its rows.

### The option that won, and what it costs

Two fields of `RasterSettings` now mean "how to lay this mask out" while two fields of
`PrinterProfile` mean "how the panel is mounted", and the names do not say so on their own.
The pair is pinned by a test in `encrust-app` and by `docs/formats/goo.md`.
