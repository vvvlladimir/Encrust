# 0032. Read layer areas as spans on a raster grid, not as polygons

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

`core-supports` first answered the area questions placement asks with polygon booleans from
`i_overlay`, wrapped in one module: take one layer away from another,
grow a layer by how far it can reach, split what is left into pieces, shrink a piece to
find its rim.

It worked, and it was slow. On an 80 mm ball cut at 0.05 mm — 1600 layers, the shape every
support generator is judged on — a run took over seven hundred milliseconds, and it grew
with the triangle count: the same ball at a million faces cost nearly twice what it cost at
sixteen thousand, because every contour point became a vertex the boolean engine had to
sweep. A slicer is expected to answer in the time it takes to release a button.

The costs were structural rather than incidental:

- **Scale.** Coordinates were quantised to a tenth of a micrometre so that the result was
  repeatable. That is 180 times finer than a 9K panel's pixel, and every operation carried
  the vertex count that precision implies, for answers that are then rounded to a support
  every five millimetres.
- **Offsets.** Growing a layer by a millimetre with a round join is a full offsetting pass
  producing hundreds of new vertices per contour, and the result was thrown away after one
  subtraction.
- **Vertices.** A layer of a scanned model carries thousands of points a tenth of a
  millimetre apart. None of them can move the answer.

The rest of the workspace already had the machinery for exactly this shape of question.
`core-raster` turns contours into runs — `docs/decisions/0020` — sweeping each row once
and emitting a span wherever the layer is solid, at a cost that follows the outline rather
than the area.

## Decision

`core-supports` reads every layer onto a grid of 0.1 mm square cells with
`ScanlineRasterizer` in binary shading, and does all of its area work on the spans that
come back. `i_overlay` is dropped from the workspace.

A `Field` is one layer's cells: the spans of every row end to end in one buffer, with a
`starts` index saying where each row begins, and a row number for the first row. Not a
vector per row — a run reads thousands of layers and asks four or five questions of each,
and a vector per row of each of them is more time in the allocator than in the geometry.

Every operation is a merge walk over sorted rows: difference, intersection, "does this
touch that", "does this lie next to that", connected pieces by union-find over spans, and
`shrunk`, which pulls a field back from its own edge. Growing a field is not one of them.
Placement never needs the grown field itself, only what a grown field would leave behind,
so `uncarried_by` subtracts the other field's rows as it reads them, widened on the way,
and stops on a row as soon as nothing is left of it.

The grid is anchored to the plate's origin, not to the model's bounding box, so a model
moved a whole number of cells reads exactly the same.

Contours are thinned before they are read, to half a cell, by dropping points that stay
inside a corridor around the line the run started along. The walk begins at the ring's
lowest point rather than at whichever vertex the slicer stitched from, because two layers
of the same wall must thin to the same outline — a ring thinned from a different vertex
lands a fraction of a cell elsewhere, and the layer above would then read as material the
layer below does not carry.

## Consequences

- A run is between five and thirteen times faster. On the benchmark: four tables of
  1200 layers 22.5 ms to 4.3 ms, a 30 mm ball of 600 layers 53.5 ms to 8.4 ms. The 80 mm
  ball above went from 706 ms to 52 ms.
- The cost no longer follows the triangle count. The same ball at 16 k, 262 k and 1 M faces
  now costs within a fifth of the same, because thinning removes everything the grid cannot
  show. `generate_supports/ball, 1 M faces` is in the benchmark to keep it that way.
- One less dependency, and `i_shape`/`i_float` with it. The workspace has no polygon
  boolean engine at all.
- 0.1 mm is the finest thing placement can tell apart. That is four times finer than the
  thinnest support head and two and a half times finer than the narrowest island worth
  holding, and coarser than a printed pixel: a feature under a tenth of a millimetre is not
  something a column can be stood under anyway.
- Areas are cell counts, so an area is out by the cells along its own outline. A 5 mm
  square reads within 1% and a 0.5 mm speck within 20%; the filters that use areas are
  thresholds with an order of magnitude of headroom either side.
- The signal to reopen this: an operation placement needs that a span list cannot do
  cheaply — a true offset with a mitred join, or a boolean between regions that are not
  both on this grid. Branching supports in step 7c work on columns rather than on areas, so
  this is not expected.

## Alternatives considered

### Keep `i_overlay` and make it cheaper

Thin the contours before handing them over, drop the quantisation scale, cache the grown
layers. The first two are most of the win and were tried; what is left is a sweep-line over
exact predicates being asked a question that is already answered by the rows the rasteriser
walks anyway. Keeping a dependency for that is paying for generality nothing uses.

### Dense bitmaps, one bit per cell

The obvious raster answer: a layer becomes a bit per cell and every operation is a word-wise
AND or ANDNOT. Simple, and fast on a small model. It loses on the same argument
`docs/decisions/0020` made about layers: a 200 mm plate at 0.1 mm is four million cells a
layer whether the model fills it or not, and a difference costs the plate rather than the
part. Spans cost the outline.

### A distance field per layer

Store, per cell, how far it is to material on the layer below; the overhang test becomes a
lookup. Elegant, and it answers "how far past" with a number rather than a yes or no. A
distance transform is a two-pass sweep over every cell of the bounding box on every layer,
which is the dense bitmap's cost with more arithmetic.

### The option that won, and what it costs

Spans are quantised, and quantisation has an error. A field's edge is where the layer covers
a cell's centre, so an outline can land half a cell — 0.05 mm — from where the contour
really is, and thinning may move it another half. An overhang detected exactly at the
profile's angle may be detected a layer late or not at all, and two shapes 0.05 mm apart
read as touching. Spans also cannot answer anything off the grid: a question about a
distance finer than a cell has to be asked somewhere else. Both were accepted because every
number placement produces is rounded to a support head 0.4 mm across, eight cells wide.
