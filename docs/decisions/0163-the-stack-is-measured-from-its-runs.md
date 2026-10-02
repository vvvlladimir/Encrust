# 0163. The stack is measured from its runs, in a crate of its own

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

The resin volume was the area of the contours, so grey edges, blur, the grey floor and
the edge of the panel never reached the file header, the weight or the price. Step 17
wants the stack read as it is written — area, pieces, the pull on the film — and later the
layer a print fails on. `core-supports` reads contours on its own coarse grid, which is
not what the printer is sent.

## Decision

A new crate, `core-analysis`, depending only on `core-raster`, reads a layer's
`LayerRuns`: lit runs merged into row spans, joined into pieces by union-find over
neighbouring rows, and each piece's area and second moments summed. The pull is Stefan's
law, whose suction over a shape is its torsion constant, taken as Saint-Venant's
`A^4 / (4 pi^2 I_p)`; see `docs/design/analysis.md`. Each layer is measured in the same
parallel map that rasterises it; `Measured` folds the layers in order and holds none.
The file, the CLI, the batch JSON and Preview take the volume from it. Preview measures the
stack on its own thread without writing a file.

## Consequences

`core-analysis` sits beside `core-supports` above `core-raster`; step 17b's lever arms and
islands go here too. The volume now matches what cures, and an unpriced resin still says
so. Measuring a dense 8520 x 4320 layer costs 7.8 ms against 26.7 ms to rasterise it
(`benches/measure.rs`). Preview rasterises the whole stack once per plate and panel.

## Alternatives considered

### A module of `core-supports`

It already walks the stack, but on contours and a grid of its own, not on the pixels the
printer is sent, and a crate about supports would own the price of a print.

### The jump in area alone

What the roadmap first named, but suction goes as the fourth power of size: a strip pulls
far less than a square of its area, and an area jump flags both alike.

### The option that won, and what it costs

A new crate on the graph, and a second full rasterisation pass in Preview that the file
write repeats. The torsion constant is an approximation — within ten per cent on a square
and a strip — and a proxy only: no peel force is calibrated in newtons.
