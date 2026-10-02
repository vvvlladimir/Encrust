# 0009. Anti-alias by sub-scanline coverage, not by supersampling

- **Status:** Superseded by 0021
- **Date:** 2026-09-17

## Context

An MSLA panel honours intermediate grey as reduced exposure, so a partly covered pixel can
be printed partly. That is worth having: it puts an edge where the geometry actually is
rather than on the nearest pixel boundary, and it is what stops a curved wall printing as
a staircase.

The panel is large. A Mars 4 Ultra is 8520 × 4320, which is 36.8 MB for one 8-bit mask,
and a 165 mm model at 0.05 mm layers is 3300 of them. Any approach that multiplies the
working buffer is expensive at that size.

Three families were on the table: supersample the whole mask and downsample it, compute
exact area coverage with a cell-based rasterizer, or sample several scanlines per pixel
row while computing horizontal coverage exactly.

## Decision

`Shading::Coverage { samples }` walks `samples` sub-scanlines per pixel row, 16 by
default. Each contributes `1/samples` of the row's coverage. Along a sample line, the ends
of a span contribute the exact fraction of the pixel they cover.

Horizontal coverage is therefore exact and vertical coverage is quantised to `1/samples`.

`Shading::Binary` is the other setting: one sample line per row, and a pixel is lit
exactly when its centre falls inside a span. It exists for panels that do not honour grey.

The working buffer is one row of `f32`, 34 KB at full panel width, regardless of the
number of samples.

## Consequences

Accuracy is good enough that it can be asserted on numerically rather than looked at: a
720-gon of radius 5.03 mm rasterises to within 0.1 % of the area it encloses, and the
tests pin that down.

The error that remains follows the perimeter of a shape rather than its area, because it
comes from quantising horizontal edges to `1/samples`. Two shapes of different sizes
cannot be compared with a purely relative tolerance, which is stated in the one property
test that does compare them.

Cost scales with the sample count: 92 ms per full-panel layer at 16 samples, 34 ms at 4,
10 ms binary. `samples` is a setting rather than a constant precisely so that a user who
wants a faster preview can drop it.

Reopen this if a panel appears whose grey response makes 16 vertical levels visible, or if
profiling on a real stack shows the sample loop dominating a job.

## Alternatives considered

### Supersample the whole mask

Rasterise at 4× in both directions and box-filter down. The simplest correct approach and
the easiest to verify. Rejected on memory: a 4× buffer of a Mars 4 Ultra panel is 590 MB
per layer, which rules it out before the speed question is even asked.

### An exact cell-based rasterizer

What font rasterizers use: accumulate signed area and cover per cell, producing exact
coverage in one pass with no sampling at all. It is the right answer for accuracy and is
not much slower. Rejected for now because it is substantially more code to write and to
get right, and the accuracy it adds over 16 sub-scanlines is below what an 8-bit mask can
represent.

### The option that won, and what it costs

Sub-scanline sampling is an approximation dressed as a setting. Vertical and horizontal
directions are not treated alike, which is invisible on curves but shows up as a
systematic difference on near-horizontal edges — exactly the edges a low overhang angle
produces. Nothing in the test suite currently pins that asymmetry down, and a user who
raises `samples` to fix an artefact pays for it on every layer of the job rather than only
where it mattered.
