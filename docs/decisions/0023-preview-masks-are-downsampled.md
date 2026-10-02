# 0023. Preview masks are max-pooled down to at most 2048 pixels

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

A rasterised layer covers the whole panel: 8520 × 4320 pixels on a Mars 4 Ultra. egui
uploads textures as RGBA8, so handing it a layer whole is 147 MB of conversion and
transfer for a picture that lands in a panel a few hundred points across. The preview
changes texture every time the slider moves, so that cost would be paid during a drag.

Shrinking is therefore not optional. How it shrinks is the decision. A printed part is
full of features one or two pixels wide — a wall, a hole's edge, the tip of a support in
step 7 — and those features are the reason to look at a layer at all.

## Decision

`core_raster::downsample` turns a `LayerRuns` into a `LayerMask` of at most
`MAX_PREVIEW_PX` = 2048 pixels on each side. Both axes shrink by the same whole factor, so
the mask keeps the panel's pixel aspect, and the leftover row and column of a size that
does not divide become a block of their own.

A block of source pixels becomes the **brightest** pixel it holds, not their average. A
wall one pixel wide inside a 16-pixel block averages to 16, which is black on screen; the
maximum keeps it at full exposure. The preview answers "is anything exposed here", and the
maximum is the operator that answers it.

The function takes runs rather than a mask, so a blank surround costs one run and no
memory proportional to the panel, and it is drawn with nearest-neighbour filtering: the
picture is magnified back up, and a linear filter would blur away exactly the features the
maximum was chosen to keep.

## Consequences

- A Mars 4 Ultra layer shrinks by 5 to 1704 × 864: 5.9 MB as RGBA against 147 MB, and it
  is uploaded only when the layer changes.
- Cost follows the runs, not the panel. A blank layer is one run and one pass over the
  output.
- Area read off the preview is wrong, upward: the maximum spreads a thin feature over its
  whole block. The panel therefore reports the exposed area from the layer's contours,
  which is exact, and never from the mask.
- The preview is not pixel-accurate on a panel larger than 2048 pixels. Judging the exact
  shape of a one-pixel feature needs the exported file. A zoom that rasterises only the
  visible part of the panel at full resolution is the way out if that becomes a problem.

## Alternatives considered

### Average the block

The correct answer for a photograph, and it would make the reported area right. It fades
out every thin feature on a large panel, which is the opposite of what a slicer's preview
is for.

### Upload the whole panel and let the GPU shrink it

One texture, hardware filtering, no CPU pass. It is the 147 MB upload per slider step that
this decision exists to avoid, and the hardware filter would be linear.

### Decode the runs in a shader

No CPU-side mask at all: upload the runs and expand them on the GPU, which is where the
picture ends up anyway. It needs a storage buffer, a compute or fragment pass and a place
in the viewport's wgpu plumbing, for a step whose job is a slider and a picture. Worth
revisiting if the preview ever needs to zoom.

### The option that won, and what it costs

The preview shows a mask no printer will ever expose: every block is as bright as its
brightest pixel, so a part looks slightly fatter than it is, and anti-aliased edges look
harder than they are. The shrink factor is a whole number, so a 3000-pixel panel shrinks
by 2 to 1500 rather than to the 2048 it could have used — up to half the allowed detail is
given away to keep the arithmetic exact.
