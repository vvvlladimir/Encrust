# Measuring the stack

`core-analysis` reads each layer as its runs, the form the printer is sent, and never as
contours. Why it is its own crate: ADR 0163.

## A layer

A run lights `length` pixels in reading order at one grey and may cross a row end, so the
runs are cut at row ends and merged into **spans**: the lit pixels of one row, whatever
their greys. Spans are joined into **pieces** by union-find: two spans are one piece when
they overlap on neighbouring rows, walked with two pointers down both rows. Pieces touching
only at a corner stay apart. Cost is linear in the runs, never in the panel.

Each span adds its pixels to its piece as rectangles of panel weighted by grey (`value /
255`): the area `A`, the first moments and the second moments about the corner, summed in
`f64` millimetres. The polar moment about the piece's centroid is
`I_p = Ixx + Iyy - (Sx^2 + Sy^2) / A`.

## The pull

Separating a flat layer from the film draws resin into a thin gap from its edge. Stefan's
law gives the force on a disc as `3 pi mu R^4 h' / (2 h^3)`; for any shape the pressure
solves the same Poisson problem as Saint-Venant torsion, so the force is proportional to
the section's torsion constant `J`. Each piece is taken apart from the rest, since resin
reaches every piece from its own edge, and the layer's pull is the sum. `J` is
approximated as `A^4 / (4 pi^2 I_p)`: exact for a disc (`pi R^4 / 2`), 0.152 `s^4` for a
square against 0.141, 0.304 `a b^3` for a thin strip against 0.333.

A pull is stated as the diameter of the disc that pulls as hard, `2 (2 J / pi)^(1/4)`.

## A stack

`Measured` folds layers in print order: the volume is each layer's area times its own
thickness, which is not one number on an adaptive plan. Above the bottom block — which the
plate holds, not the model — it keeps the layer that pulls hardest and the layer whose
area grows most over the one under it. It holds the layer below and one reading a layer.

## Islands and levers

Two layers **touch** where a span of one overlaps a span of the other on the same row;
one walk down both span lists finds every shared stretch, summed per pair of pieces. Why
the risks are read this way: ADR 0164.

- A piece of at least `MIN_ISLAND_MM2` that touches nothing below is an **island**. One
  that touches only floating pieces floats too, and is not named again.
- Each anchored piece carries its **neck**: the narrowest section between it and the
  plate. It is the piece's own contact where that is under 0.9 of the necks it stands on,
  and otherwise those necks together, their areas summed and their centres weighted.
- The layer's pull on a piece is `F = J * PEEL_N_PER_MM4`. On a neck of area `A` and
  radius `r`, off by the lever `L` from the piece's centre, the stress is
  `F / A + F L / (pi r^3 / 4)`: tension, and bending over a round section. Past
  `NECK_LIMIT_MPA` it is a **lever**, kept once per neck at its worst.
- A layer whose whole pull `F` passes `PEEL_LIMIT_N` is a **peel**; a run of them is
  named once, at the hardest.

The first island is where a print fails, since nothing cured over nothing comes up with
the plate; failing that, the most stressed neck, then the hardest peel.

## Removing islands

A fold that removes islands takes every piece touching nothing below out of its layer,
whatever its size, and hands back the stretches it covered. The writer erases them from
the runs before encoding, and the next layer is judged against the layer as written, so
whatever stood only on an island becomes one and goes too. A write therefore runs in four
stages a window at a time: rasterise and cure in parallel, fold in order, erase and encode
in parallel, push in order. `Measured` keeps the stretches per layer, which is what
Preview erases from its picture.
