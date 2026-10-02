# 0169. Trace the `.svgx` polygons out of the mask rather than carry contours to the writer

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

`.svgx` is the first container we write whose layers are vectors: each is a group of filled
SVG paths in millimetres from the middle of the panel, with no grey anywhere in the file.

Our pipeline hands a writer rasterised layers. `LayerSink::encode` is given a `LayerRuns`,
one window at a time, and the contours the slicer found are gone by then — `fold_group`
rasterises them and the pipeline keeps the runs, which is what every other container needs
and what the stack is measured from (ADR 0127, 0163).

The contours are therefore either carried through the pipeline to the writer, or recovered
from the mask inside the crate that wants them. Reading the container back needs the inverse
either way: polygons in millimetres have to become runs again.

## Decision

`format-svgx` traces its own polygons. Every boundary between a lit pixel and a dark one is a
unit step on the pixel grid; the steps are oriented so that material lies to one side,
stitched into closed rings, and a step continuing the one before it is dropped. A mask shaded
by coverage is cut at a grey of 128.

Every ring of a layer goes into **one** path with `fill-rule="evenodd"`, so a ring inside
another is a hole whatever order the two are written in and no nesting has to be worked out.

The reader goes the other way through code we already have: a group's subpaths become
`core_slicer::Contour`s, which `core_raster::ScanlineRasterizer` fills at the panel the
document states. `format-svgx` therefore depends on `core-slicer` as well as `core-raster` —
a downward edge, drawn in `docs/architecture.md`.

The resin volume is the one field the document states in front of its layers and only knows
behind them, so it is written as a fixed-width zero-padded decimal and filled in place at the
end. The document stays a stream.

## Consequences

The pipeline and every other writer are untouched, and the tracing is parallel with the rest
of the encoding because it happens in `encode`. A layer round-trips exactly: the mask written
is the mask read back, holes and all, which the crate's integration test pins down.

An outline is a stair-step of the pixel grid, as a vendor slicer's is, so a file is larger
than the same stack as runs — one point per step, collinear runs merged. That is the shape of
the container and not a choice of ours.

Grey is lost, which this container has nowhere to put. The threshold is stated once, in the
crate and in `docs/formats/svgx.md`; if a machine of this family turns out to honour partial
coverage somehow, that is the signal to reopen this.

## Alternatives considered

### Carry the slicer's contours to the writer

Honest about the data: the contours exist upstream and are exact. It would mean widening
`SlicedFileWriter` and `LayerSink` so every container takes a kind of layer it has no use
for, and the pipeline holding both forms of a window. One container does not get to reshape
the interface the other eight use.

### One path per ring

Simpler to emit. It is wrong: even-odd filling is per path element, so a hole in its own path
is painted rather than cut, and the layer comes back solid.

### The option that won, and what it costs

Tracing recovers the outline from a quantised mask, so a wall that fell on a pixel boundary
moves by at most half a pixel and a feature thinner than one pixel is gone before the writer
sees it. For a container that cannot carry grey at all this is the same loss the firmware
would take anyway, and it buys an unchanged pipeline.
