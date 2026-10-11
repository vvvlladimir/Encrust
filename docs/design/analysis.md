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
  radius `r` the stress is `F / A + F L / (pi r^3 / 4)`: tension, and bending over a round
  section. The lever `L` is how far off the neck the pull acts: the piece's centre off the
  neck's, plus half the piece's own radius of gyration `sqrt(I_p / A)`, because the film
  lets go from one edge and the pull is then carried by the half still stuck. Without that
  term a plate centred over a wire broke nothing. Past `NECK_LIMIT_MPA` it is a **lever**,
  kept once per neck at its worst.
- A layer whose whole pull `F` passes `PEEL_LIMIT_N` is a **peel**; a run of them is
  named once, at the hardest.

The first island is where a print fails, since nothing cured over nothing comes up with
the plate; failing that, the most stressed neck, then the hardest peel.

## Removing islands

A fold that removes islands takes every piece touching nothing below out of its layer,
whatever its size, and hands back the stretches it covered. The layer below is judged as
cured, islands and all, so a piece standing on an island is floating rather than an island
of its own: it stays in the file, a part floating over sixty layers loses its first layer
alone, and the piece over the island is named once as **floating**, failing the verdict
until it is supported (ADR 0228). Each further pass the user asks for takes one layer more:
a piece standing only on what was taken out goes too while its depth over the island is
within the passes, and the first one past them is named. The writer erases what was taken
from the runs before encoding. A write therefore runs in four stages a window at a time:
rasterise and cure in parallel, fold in order, erase and encode in parallel, push in
order. `Measured` keeps the stretches per layer, which is what Preview erases from its
picture.

## Suction cups and thin sections

A **suction cup** is a hole: dark pixels a layer closes in on every side, found per layer by
joining the dark runs between two lit ones across rows and dropping every group that reaches
the panel's edge or a row past the lit ones. A hole is *vented* when any of its pixels lies
over open air in the layer under it, or over a hole that was; the first layer lies on the
plate, so its holes are sealed. A sealed hole carries its cup up the stack while it stays
sealed, adding its area times the layer's thickness; the cup is named once, at the layer it
is widest, when that is at least `MIN_CUP_MM2` (25 mm², some 2.5 N at a full vacuum) and it
holds at least `MIN_CUP_MM3` (0.02 ml). Anything less is a letter's counter or a pin's socket,
which pulls too little to matter. A model standing on supports has open air
between them, so its cavities start vented; one lying on the plate or on its own floor does
not.

A **thin section** is a piece held from below whose area, as a disc, is under `THIN_MM`
(0.15 mm) across: narrower than the finest support tip, so a tip is never named. A run of
thin pieces standing on each other is named once, where it starts, and only once it is
`MIN_THIN_RUN_MM` (1 mm) tall: a shorter one is the point a curved surface starts on over a
tip, or the end of a spike, and neither has anything to carry.
