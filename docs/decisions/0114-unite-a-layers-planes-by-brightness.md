# 0114. Sample a layer at several planes, and unite them by brightness

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

A layer has been one plane through the middle of its band since step 2, which was a known
shortcut from the start: a feature thinner than a layer —
a fin, a rib, the tip of a cone — is fully printed or fully lost depending on which side of
that plane it lands. The fix is to sample several planes inside the band and take their
union.

Taking that union on the contours needs a polygon boolean, which the workspace has twice
declined to take on (ADR 0088, ADR 0089). So the union has to happen after the contours
become pixels, which puts it in `core-raster`.

Two other things constrain the shape. `core-supports`, the drainage scan and Preview all
read `Sliced` and mean "the cross-section of this layer"; multiplying the stack's layer
count would change what they see, what the exposure bands index and what the written file
says. And the sweep in `core-raster` already sums windings across contours, which makes
adding the planes together look free.

## Decision

`SliceSettings::samples` splits each layer's band into that many equal parts and samples the
middle of each. `Windows` cuts them — it is the one place that has both the plan's bands and
the streaming window — hands the whole group to `slice_at` in one call, and folds each group
into one `Layer`: the plane nearest the middle of the band becomes `contours`, the rest
`extra`. `Layer::z` stays the middle of the band, because that is where the plate stands.

Only a rasteriser reads `extra`. `ScanlineRasterizer` fills each plane on its own and
combines them with `LayerRuns::brightest_of`, a two-cursor walk over both run lists keeping
the brighter pixel.

Brightness, not the sum of the windings. A wall covering half of a pixel on three planes
sums to 1.5, clamps to 1 and comes out white, which throws away the coverage anti-aliasing
of ADR 0039 and fattens every edge by up to a pixel. The maximum keeps a wall's edge exactly
as one plane saw it, and still lets a feature only one plane found light its own pixels at
its own coverage.

## Consequences

Everything that reads a cross-section is untouched: with one sample the code path is the
one that was there before, and with more, `contours` is still the plane through the middle
of the band. Supports are placed, resin traps found and the preview drawn exactly as before.

The work grows with the plane count but the wall time need not: every plane of a window goes
into one `slice_at`, so `rayon` spreads planes the way it spread layers. A 14 400-triangle
sphere cut into 400 layers on eight cores took 0.43 s at one plane and 0.45 s at five. On a
model that already saturates the cores it will cost its full multiple.

Two figures now under-report. `Sliced::resin_volume_mm3` adds contour areas, and it cannot
add the extra planes without the polygon boolean this decision exists to avoid, so the
estimate misses what only an extra plane found. The `.goo` header's volume does not, because
it is measured from the written masks (ADR 0067). The CLI's contour count *does* include
every plane, since the point of that figure is the work done.

`LayerRuns::brightest_of` returns `self` unchanged when the two layers are different sizes.
Nothing can produce that today — both come from one `RasterSettings` — and the alternative
was a fallible signature on a hot path for a case the types nearly rule out.

## Alternatives considered

### Multiply the stack: one `Layer` per sampled plane

Two lines in the slicer and nothing new in the rasteriser. It lost because it changes what
a layer *is* for four other consumers: island detection would find islands at sub-layer
spacing, exposure bands would index the wrong layers, and the written file would claim three
times the layers at a third of the height.

### Add the planes' windings

Free: the sweep already sums contours, so an extra plane's rings could simply join the list.
Rejected on the number above — a half-covered edge pixel present on every plane comes out
white. There is a test for exactly this, `two_planes_over_the_same_wall_do_not_brighten_its_edge`.

### Union the contours with a polygon boolean

The textbook answer, and it would let the volume estimate stay honest. It lost because it
means `clipper2` or its like in the workspace, which ADR 0088 and ADR 0089 each refused for
a smaller gain than this one.

### The option that won, and what it costs

`Layer` grew a third field, and it is a field most of its readers must know to ignore. A
consumer that reasonably assumes `contours` is the whole layer — anything counting area,
which `resin_volume_mm3` already does — is quietly wrong when sampling is on, and nothing but
this document stops the next one. A newtype that made the two cases impossible to confuse
would have been better and would have touched every one of the fourteen places that build a
`Layer`.
