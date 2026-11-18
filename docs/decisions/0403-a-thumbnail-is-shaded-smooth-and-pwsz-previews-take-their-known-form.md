# 0403. Shade the thumbnail smooth with lines, and write `.pwsz` previews in their known form

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

ADR 0048 shades each face flat against one light. On a printer's 3.5 to 5 inch screen that
reads poorly: a curved part shows its facets, a face turned from the light sinks into the
dark background, and a limb in front of a body merges with it. Vendor software shades
smooth.

ADR 0402 left the `.pwsz` records on the colours `docs/formats/pwsz.md` stated, teal and
navy. The two known files, for the M7 Pro and the Mono 4 Ultra, say otherwise: both
previews are RGBA PNGs, the model on an opaque `#0A0A0B`, with transparent bars where their
picture does not reach. A model on teal sat on the printer's own teal screen.

## Decision

The renderer fills a depth and a normal per pixel and lights each covered pixel once
afterwards. Normals are smoothed with `core_geometry::smooth_normals` within 40°. The light
is an ambient, a key high over the camera's left shoulder, a fill low on its right, a key
highlight and a rim light towards white. Where the depth jumps, one surface in front of
another, a pixel is darkened to 40%. Numbers and reasons: `docs/design/thumbnail.md`.

A `.pwsz` preview is an RGBA PNG, every pixel opaque, on `#0A0A0B`, through a new
`core_format::encode_colour_alpha`.

## Consequences

Every format's preview and the window's recent-file pictures, which share the renderer,
show smooth curves, three tones on a box and the outline of a part in front of another. The
`.pwsz` preview looks as a known file's does on the printer.

A render costs 100 ms for a 978k-triangle sphere at 1024 pixels on an Apple M1, against
51 ms flat; smoothing the normals is most of it and runs on `rayon`. A placed part is copied
into plate coordinates for its normals, one mesh's worth of memory for the render's life.
ADR 0402 states 0.50 and 0.69 s for the old render: those were taken on a loaded machine,
and 32 and 51 ms are the figures.

A `.pwsz` writes no transparent pixel: our picture is cropped to fill the record, so there
are no bars to leave clear.

Reopen the lines if a part with deep, narrow grooves comes out striped: the threshold is a
depth, not a curvature.

## Alternatives considered

### Shade per face as before, from smoothed normals averaged per face

No change to the rasteriser. It lost because it cannot remove facets: a face is still one
colour, only a different one.

### Outline the silhouette in a dark colour

The usual cartoon outline. It lost on the background: on a black or near-black record a dark
line around the part is invisible, and it would eat the rim light that does part it from
the background.

### Leave the `.pwsz` bars transparent, as known files do

Matching their alpha byte for byte means rendering at another shape and padding, which ADR
0402 removed for every format. Transparent pixels around a cropped picture would show the
screen's teal behind the part again.

### The option that won, and what it costs

Twice the render time, a normal buffer of 12 bytes a pixel, 12 MB at 1024, and a set of
lighting constants tuned by eye on a few models rather than measured against a printer's
screen.
