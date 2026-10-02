# 0109. The preview is read against the plate, not against the panel

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

Almost every masked-SLA machine wants its exposure images reflected, because of how the
LCD is mounted and wired under the vat: every printer profile we ship carries
`mirror_x = true`. The rasteriser applies that reflection in `PixelSpace`, so the masks
that reach `.goo` and `.ctb` are already mirrored and the format's mirror field only
records the fact (`format-goo/src/header.rs`).

The preview pane asked the rasteriser for the same settings the writer uses, so the
picture beside the model was the file's bytes. A part standing on the right of the plate
appeared on the left of the mask. Other slicers show the mask the way round the plate is,
and mirror only on export.

## Decision

`Preview::mask` rasterises with `mirror_x` and `mirror_y` forced off. Mirroring belongs to
the written file; the preview is a reading of the plate from above, in the same handedness
as the viewport beside it.

Nothing about the written file changes: `job::pipeline::raster_settings` still takes both
flags straight from the profile.

## Consequences

- The mask and the model agree: a part on the right of the plate is on the right of the
  picture, which is the whole reason the two are drawn side by side.
- The preview no longer proves the file is mirrored the right way. A machine whose profile
  has the wrong flag now shows nothing wrong on screen and prints a mirrored part. The
  check for that is the written file, in a reader or on the printer's own preview.
- If anyone asks to see the bytes as written, that is a toggle over `as_seen`, not a
  redesign.

## Alternatives considered

### Leave the preview mirrored and mirror the viewport instead

Makes the two halves agree the other way round. Rejected: the viewport is the plate, and
turning the plate round to suit a panel flag would break picking, the gizmo and every
coordinate a user reads.

### Flip the texture at draw time in `mask_pane`

One `uv` rectangle. Rejected because it leaves `Preview::mask` returning a mirrored mask
that only one caller knows to undo, and the layer area and the downsample factor are
already computed off the unmirrored side.

### The option that won, and what it costs

Losing the mirroring from the preview loses the only place a wrong `mirror_x` was visible
before a print. We accept that: the flag is a property of the machine, set once in the
profile, and an on-screen picture nobody can compare against a real panel was never good
evidence for it.
