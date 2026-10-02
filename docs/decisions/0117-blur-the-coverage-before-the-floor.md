# 0117. Blur the coverage on the runs, before the floor

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

The vendor slicers pair their grey levels with an image blur of level 2 or 4, which fades an edge over
more than the one pixel exact coverage greys. Published tests on a Mars 3 found grey level 3
with blur 2 the best surface, and that a wider blur turns the white just inside a wall grey
as well — dimmer than a panel cures. The kernel itself is documented nowhere.

Two things are fixed here already. A layer is runs and is never expanded to pixels
(ADR 0020). And a grey under the profile's floor is written black (ADR 0113), so whatever
the blur produces must pass through the floor after it, not before: a floor applied first
has already turned the outer half of every edge black, and the blur would only dim the
inside.

## Decision

`RasterSettings::blur_px` is the radius of a box filter `2r + 1` pixels wide, applied in both
directions to the linear coverage of the layer, after its planes are united and before
`Grey` rounds and floors it. `LayerRuns::blurred` works on the steps between runs: a row's
sum only changes within `r` of a step, and a column's sum only where one of its `2r + 1` rows
steps, so the cost follows the edges. The radius goes into the `.goo` blur field; binary
shading ignores it.

## Consequences

An edge fades symmetrically about where it was: half the fade lies outside the model and
half inside, and the pixel half covered stays at half. With the default floor of 128 the
outer half is cut black, so what prints is the inner fade — the same dimming inward other
slicers' users report, which is why the window hints past two pixels.

Every grey pixel is a chunk in `.goo`, so the file grows with the edge: a 1632-layer model at
the default floor goes from 45 MB to 73 MB at radius 1 and 101 MB at radius 2. A grey
ladder claws much of it back (8 levels with radius 1: 59 MB; 4 levels: 47 MB). A blurred
layer costs about 2 ms on a full Mars 4 Ultra panel against 0.25 ms for coverage alone.

## Alternatives considered

### A ramp along each row only

What the step was first written as: fade each run's ends horizontally. Cheaper, but an edge
running along a row would stay sharp while one running down a column fades, so a square
would come out with two soft sides and two hard ones.

### A Gaussian kernel

What "blur" means in an image editor. Its tails are below any floor a panel cures at, so
the floor cuts them and what is left is close to a box; it would cost more per pixel for
a difference no print shows.

### Blur the written greys, after the floor

Simplest to add, as a pass over the finished runs. It lost because the floor has already
removed the outer half of the edge by then, so the blur could only move light inward.

### The option that won, and what it costs

A bigger file, and a layer several times slower to rasterise, for an effect whose worth
rests on other people's resin tests, not ours. The box also fades a diagonal edge a little
wider than a straight one, which a distance-based ramp would not.
