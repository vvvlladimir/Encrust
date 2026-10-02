# 0021. Anti-alias by exact pixel area, not by sub-scanlines

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

ADR 0009 chose sub-scanline sampling: 16 sample lines per pixel row, each contributing
`1/16` of the row's coverage, with exact horizontal coverage along each line. It named two
signals to reopen the decision. One of them was *profiling on a real stack shows the
sample loop dominating a job*.

ADR 0020 made that happen. With the panel-sized bitmap gone, the benchmark disc costs
2.22 ms per layer at 16 samples and 0.62 ms at 4 — a factor of 3.6 where the two had
previously been indistinguishable. Everything that is left is the sample loop.

The other reason ADR 0009 gave for not taking exact area was that it is "substantially
more code to write and to get right". That is less true after ADR 0020 than it was before
it: exact area needs a per-row list of coverage differences and a running sum over it,
which is exactly the machinery ADR 0020 already built for the sparse rows. What was a
second rasteriser is now a second way of filling a structure that exists.

## Decision

`Shading::Coverage` computes the exact area of each pixel the layer covers. It carries no
sample count, because there is no sampling.

Each contour edge deposits, into every pixel of every row it crosses, the signed area that
edge cuts from that pixel. The running sum along the row is then the winding number
weighted by coverage, and the pixel's grey is its magnitude clamped to one. Solid pixels
between two edges receive nothing and are carried by the running sum, as they already were.

This is the exact-area method every serious font rasteriser uses — `libart`, FreeType's
smooth renderer, `stb_truetype` v2 and `font-rs`. The formula is theirs; the code is ours,
written against the sparse rows of ADR 0020 rather than against a dense accumulation
buffer, so a row's working set follows the edges crossing it rather than the panel width.

`Shading::Binary` is unchanged: one sweep at the centre of each pixel row, for panels that
do not honour grey.

`RasterError::ZeroSamples` is gone with the setting that could trigger it, as is the CLI's
`--samples`.

## Consequences

- The benchmark disc went from 2.22 ms per layer to 225 µs, and a full slice of a 30 mm
  cylinder 400 layers deep from 0.35 s of CPU against 0.89 s. Coverage shading is now
  2.2 times the cost of binary rather than 21 times, which makes `--no-anti-alias` a
  setting for panels that need it rather than one people reach for to save time.
- Accuracy improved by more than two orders of magnitude, not by a little: the 720-gon
  that ADR 0009's test held to 0.1 % now comes out at 0.00045 %. The test bound moved to
  0.01 % to pin that down.
- The asymmetry ADR 0009 admitted to is gone. Vertical and horizontal directions are
  treated alike, so near-horizontal edges — the ones a shallow overhang produces — are as
  accurate as vertical ones. A 12 mm wedge 0.37 mm deep now rasterises to its exact area.
- Nobody has a quality dial any more. A user who wanted a faster preview could previously
  drop `--samples`; the answer now is `--no-anti-alias`, which is a real quality loss
  rather than a smaller one. That trade is acceptable because exact area is already faster
  than 4 samples used to be.
- The layer's grey now comes from a winding number that may run either way round, because
  panel mirroring reverses every contour. `grey` takes its magnitude. That is the standard
  non-zero rule of the method and it keeps ADR 0008's union behaviour: two overlapping
  bodies accumulate to 2 and clamp to fully lit.
- Signed-area accumulation conflates winding with coverage inside a single pixel. Where
  two unrelated boundaries cross the same pixel, the result is their signed sum rather
  than the area of their union. The difference is bounded by one pixel and by the 8 bits
  the mask has, and it is the same approximation every font renderer ships.

## Alternatives considered

### Keep sub-scanlines and raise the count only where it matters

Adaptive sampling: more sample lines on rows with near-horizontal edges. It keeps the
existing code and targets the cost at the error. Rejected because deciding where it
matters costs a pass over the edges that exact area does not need at all, and because it
leaves an approximation with a tuning knob in place of an answer with none.

### Supersample and downsample

ADR 0009 rejected this on memory, at 590 MB per layer for a 4× buffer. Runs make that
argument weaker — a supersampled layer in run form is not 590 MB — and it is how some
slicers anti-alias, in both XY and Z. Still rejected: it is 16 times the sweep work for an
answer that is quantised to 1/16 of a pixel, where exact area does one sweep and is exact.
Z supersampling is a different feature and a different decision; it is about what the
layer *is*, not how it is drawn.

### The option that won, and what it costs

Exact area is more delicate code than a sampling loop. The area a segment puts into each
pixel it crosses is a closed-form expression with four branches — inside one pixel, across
two, across three, across many — and every one of them has to conserve the total. A sign
error in any branch produces a layer that looks almost right, which is the worst kind of
bug in a slicer. The unit tests in `area.rs` pin each branch down individually for that
reason, and the analytic area tests hold the whole to a closed-form answer.

It is also harder to explain. Sub-scanline sampling is obvious from its name; exact area
needs the reader to understand why a running sum of signed areas is a winding number. That
is what `docs/design/rasterisation.md` is for, and it is a real cost of the decision.
