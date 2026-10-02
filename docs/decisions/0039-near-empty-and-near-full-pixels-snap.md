# 0039. Snap near-empty and near-full pixels to black and white

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

A layer leaves `core-raster` as runs, and `.goo` stores runs: one chunk per run, with a
shorter encoding for full black and full white than for any grey (`docs/formats/goo.md`).
The file's size is therefore set by how many runs a layer breaks into, far more than by
how many pixels it has.

Exact-area coverage (ADR 0021) gives every pixel the edge crosses a value of its own. Where
a contour runs nearly along a pixel boundary, that value is 1, 2 or 3 — a pixel the edge
clips by about a percent — or 252, 253, 254 on the inside of the same edge. Each one of
those splits an otherwise long black or white run into three chunks, and there are tens of
thousands of them on a 8520 × 4320 panel.

Comparing one of our files against a vendor file of a similar part showed 15 198 chunks
per layer against 13 365, at a *smaller* panel: 19% more bytes per layer for the same job.
The histogram said where they came from.

A value of 1 out of 255 is 0.4% of a pixel. On a 0.018 mm panel that is 0.07 µm of edge
position, and no resin cures from 0.4% of an exposure.

## Decision

`grey` snaps: coverage at or below 0.02 becomes `0x00`, coverage at or above 0.98 becomes
`0xFF`, and everything between keeps the rounded value it had. The threshold is a constant,
`SNAP`, in `core-raster`.

This is a rasteriser decision, not a format one, so the PNG stack, the layer preview and
every sliced file get the same pixels.

## Consequences

Measured on a 70 mm sphere sliced at 0.05 mm for a Mars 4 Ultra — 1400 layers, 8520 × 4320
— the `.goo` file goes from 51 514 777 to 48 541 283 bytes, 5.8% smaller, with no visible
change to any mask. The earlier estimate of 15–20% is not what a smooth convex body gives;
a part with long near-tangent edges has more of these pixels to lose, and a support-heavy
plate more again.

The edge of a layer now steps at 2% of a pixel rather than at 0.4%. A gradient across two
adjacent pixels that used to read 3 and 252 reads 0 and 255, so the antialiasing is very
slightly coarser at the extremes and exactly as before everywhere else.

If a panel ever turns out to cure detectably from a 2% pixel, or an exposure test shows the
threshold shifting a dimension, `SNAP` is one constant to lower. The signal to revisit this
is a measured dimensional difference between a snapped and an unsnapped print, not a
histogram.

## Alternatives considered

### Quantise every grey to fewer levels

Rounding all coverage to, say, 16 levels would merge far more runs and shrink the file
much more. It also changes pixels in the middle of the range, which are the ones doing the
antialiasing work that ADR 0009 and ADR 0021 exist for. The extremes are free to lose; the
middle is not.

### Encode smarter instead

The encoder already emits a difference chunk where one fits, which is the cheap form for a
near-neighbour grey. The chunk count is what costs, and no encoding choice removes a run
boundary that the pixel values themselves create.

### Leave it, and accept the size

Correct in the sense that the values are what the geometry says. It costs about 6% of every
file for detail no printer can reproduce, which is megabytes per print and seconds per
transfer to a machine over USB.

### The option that won, and what it costs

The rasteriser now has a tolerance in it that is not derived from geometry. It is a claim
about panels and resin — that 2% of a pixel does nothing — and it is untested against a
real machine, like the printer profile it sits beside. Two masks that differ only at the
extremes are no longer distinguishable in the output, which would matter to anyone using
this rasteriser to measure coverage rather than to print.
