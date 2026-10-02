# 0066. Slice a stack a window of layers at a time

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

ADR 0063, 0064 and 0065 fixed what a hollowing run costs, and a measurement of what was
left said the memory had moved rather than gone: on a 60 mm ball with a 1 mm honeycomb at
50%, hollowing came to 149 MB of the 1.90 GB peak, the slicer's Z index to 541 MB and the
stack of contours to the rest.

Two causes. `slice` cut every layer and returned all of them, so the whole stack was
resident before a mask was written — only the rasteriser worked a window at a time. And
`ZBins` listed a face in every bucket of the model's height it spans, which is fine for a
surface and ruinous for a lattice: 4.17 million faces became 135 million entries.

Neither is necessary. Layers are independent, they are written in order, and a layer only
needs the faces that cross it.

## Decision

`SliceEngine` gains `slice_at(mesh, heights)`, which cuts exactly the planes it is given
and builds its index over just those planes' Z range. `slice` is that, over every plane.
`layer_heights(mesh, settings)` gives a caller the planes without cutting any of them.
`ZBins` leaves out a face the range does not reach, so a window's index costs the window's
geometry.

The CLI drives it through a `Plan`, which holds the heights and hands out one window of
layers at a time, 64 by default and settable with `--slice-window`. A window's contours are
counted or written and then dropped.

The one pass that writes also counts: it folds each window into the `SliceReport`, which
is now counters rather than layers, and hands the volume it came to over at `finish`
(ADR 0067). The report is printed after the file rather than before it.

Extraction also sorts a layer's tiles before merging them, because they come out of a hash
map. Without that a cavity's vertices are numbered differently every run, and on a mesh
with a hole in it that moves which contours get papered over: the same model gave 5 282
contours and then 5 285.

## Consequences

On the 60 mm ball with the 1 mm honeycomb at 50%, peak memory went from 1.90 GB to
0.68-0.73 GB and the CPU time from 205 s to 220 s — wall clock on this machine is too
noisy to quote, so these are `user` times. The 7% is the index rebuilds. On a real
406 000-triangle model nothing moves, because that model's stack was never the expensive
part.

A shorter window holds less and rebuilds the index more often, each rebuild walking every
face. Sixty-four is where the walks stop showing.

The window only applies to the CLI. The window's Preview tab keeps the whole stack on
purpose, because the user scrubs it, and export reuses that stack. A plate whose stack does
not fit is therefore still a problem in the application, and that is the signal to carry
this into `encrust-app` — taken up by ADR 0068.



## Alternatives considered

### A sweep instead of a bucket index

Sort faces by their lowest Z and keep an active set as the plane rises: memory follows the
faces rather than faces times buckets, with no per-window rebuild. Rejected for now
because parallel-per-layer slicing would have to become parallel-within-a-block, and
per-window buckets already take the 541 MB down to tens. It is next if rebuilds show.

### The decision above, and what it costs

Every window rebuilds the index, which walks every face of the mesh, so a dense stack
costs a few per cent more CPU than cutting it in one go. The window size is also a number
with no right answer: it is a memory-against-rebuilds curve whose shape depends on the
model, and 64 is a measured default rather than a derived one.
