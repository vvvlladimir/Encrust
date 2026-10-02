# 0008. Fill by the non-zero winding rule

- **Status:** Superseded by 0071
- **Date:** 2026-09-17

## Context

A scanline rasterizer has to decide which side of a contour is solid. The two candidate
rules are even-odd, which flips at every crossing, and non-zero winding, which counts the
direction of each crossing and calls solid everything the contour wraps at least once.

For a single contour with holes wound the other way they agree. They disagree exactly
where two solid regions overlap.

That case is not hypothetical here. Supports are geometry merged into the mesh before
slicing, so a support touching the model produces two counter-clockwise contours that
overlap. Two objects positioned into one another on the plate do the same. Under even-odd,
the overlap goes dark: a hole is printed straight through both bodies at every layer where
they touch.

`core-slicer` already gives the contours a direction taken from the mesh's outward
winding, so the information non-zero needs is there for free. The stub in `core-raster`
was written for even-odd before that was true.

## Decision

Pixels are solid where the winding number is not zero. Crossings count `+1` for a contour
running down the image and `-1` for one running up; a span runs from where the total
leaves zero to where it returns.

`Rasterizer::rasterize` takes a `RasterSettings` and returns `Rastered`, which carries the
mask plus how far the layer reached past the panel. A layer that does not fit is clipped
and reported rather than refused.

## Consequences

Overlapping bodies expose their union, which is what a print needs. Holes still work,
because the slicer winds them the other way and the winding number returns to zero inside
them.

The rule is immune to the coordinate flips in `PixelSpace`. The row flip from model Y to
image rows, and the panel mirroring on top of it, each negate every winding number at
once, and `!= 0` does not care. Nothing has to be compensated for, which removes a whole
class of "the mask came out inside-out on this printer" bugs.

The cost is a dependency on contour direction being meaningful. A contour built by hand
with the wrong winding, or a mesh sliced after `--no-validate` skipped the orientation
fix, will expose a hole where it meant to expose material. Even-odd would have tolerated
that, at the price of the overlap bug.

Reopen this if a contour source appears that cannot state a direction.

## Alternatives considered

### Even-odd

Simpler to implement — no counter, just a flip — and it is what the stub was written for.
It ignores contour direction entirely, which is its appeal and its defect: it cannot tell
a hole from an overlap, and the overlap case corrupts real prints.

### Fill where the winding number is positive

Used by some renderers to make orientation errors visible rather than silent. Rejected
because it is not immune to the coordinate flips: mirroring the panel would invert the
whole mask, and the correction would have to be threaded through every mapping.

### The option that won, and what it costs

Non-zero winding is correct for the geometry this project produces, but it moves a silent
failure mode upstream. If `core-slicer` ever emits a contour with the wrong winding, the
rasterizer will not notice and will not complain; it will print the inverse of that region
and the first sign will be a ruined part. There is no check in `core-raster` that could
catch it, because a wrongly wound contour is indistinguishable from a deliberate hole.
