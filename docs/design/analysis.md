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

Separating a layer from the film draws resin into the gap from the layer's edge. Measured
on solid sections of one area, the force follows the area and, more weakly, how far resin
has to reach in, the area over the perimeter: 6.16 N for a 314 mm² disc, 4.9 N for a star
of the same area (Pan et al., *Rapid Prototyping Journal* 23(2), 2017). Stefan's squeeze
film alone, `F ∝ R⁴`, puts the disc several times past the star and a thin strip at next to
nothing; a film peeling off its edge does neither. So a piece pulls

`F = A (PULL_N_PER_MM2 + PULL_N_PER_MM3 r)`, with reach `r = sqrt(A³ / (8 π² I_p))`

which is `A / L` exactly for a disc and within a fifth of it for a square or a strip, read
off the moments with no perimeter traced. The two constants' ratio, 0.146 per mm, is the
line through Pan's disc and star; their scale puts a 100 cm² disc at about 100 N, the order
users report. Each piece is taken apart from the rest, since resin reaches every piece from
its own edge, and the layer's pull is the sum. `core-plate` reads a cut section with the
same `pull_n`, so the search and the proof read one law (ADR 0281).

The newtons are known to about a factor of three: Pan pulled straight up off PDMS, a
tilting vat peels a PFA film. So `PEEL_LIMIT_N`, 30 N — a solid disc some 73 mm across — is
a warning level, and what the number is sure of is which of two poses pulls harder. A hole
lets resin in only where it is open; one sealed from below is a suction cup, read apart
below, and Pan measured more pull, not less, on holes the plate sealed.

A pull is stated as the diameter of the solid disc that pulls as hard,
`equivalent_disc_mm`.

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
- The layer's pull on a piece is its own `F`. On a neck of area `A` and
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

## What each tip carries

`TipLoads` reads the model alone up the stack, joining pieces that touch into parts, so
a part is held only by the tips under it so far. A tip seats on the piece within 0.15 mm of
it, on its own layer or within a millimetre above. A part on the plate is the plate's.

Each layer's pull on a part goes through its tips read as **one section of separate discs**,
area `A = Σ a_i`, second moments `I = Σ (a_i d_i d_iᵀ + a_i r_i² / 4)` about their centre. The
highest stress in tip `i` is

`F / A + κ·d_i + |κ| r_i + |P I⁻¹ d_i| + r_i σ_max(I⁻¹ P)`, with `κ = I⁻¹ M`

`M` the pull's moment about that centre and `P = Σ F_p (J_p / 2)^½` the peel lever of each
piece `p`, `J_p` its second moments about its own centre over its area: the film's last grip
lies on that ellipse, far along a strip and near across it, at half the polar gyration on a
disc or a square, as for a neck. The peel is taken the way worst for that tip. One tip is
exactly a neck's `F / A + F L / (π r³ / 4)`. Margin is the strength over this stress. It is
rigid-body sharing: a thin arm is not softer than the body, and the film is not followed as
it peels. Margins are read against `NECK_LIMIT_MPA` as calibrated below.

Placement mends what it reads (ADR 0422): a tip under ×1.5 is built one size heavier where
its plan has a wider tip; at the widest it gets a neighbour, which helps only if it seats
before the tip's worst layer. Three rounds at most, read on 0.05 mm pixels.

## Calibration

One print of a `support-lab --coupon` set the numbers: flat 8 × 8 × 1.2 mm plates on 1, 2, 4,
9 or 16 tips of 0.30 to 0.80 mm, pillars 1.6 mm flaring to 3.2 mm so only tips tear, in
Anycubic ABS-Like Pro 2 at 0.05 mm and 2.5 s on a tilting vat. Every plate read past the
old 5 MPa that held stood at 13.0 MPa or under, every one that tore at 17.1 or over; one at
27.7 held loose. `NECK_LIMIT_MPA` is the most that held, 13 MPa. The pull law is unchanged:
with one resin only the limit over the pull is measured, so the limit carries it.

The bench's area rule agrees once a tip holds 120 mm² of flat plate per mm² of its section
(`--flat-ratio`), and only from two tips up: a plate on one tip tears sooner, which the
lever reads. A 0.30 mm tip prints but breaks in the wash, so `--min-tip-mm` is 0.4.
Reopen with a second resin, another layer height or a straight-pulling vat.

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
the panel's edge or a row past the lit ones. That reads one layer alone, so it is done as
the layer is cured, in parallel; only the linking below runs in print order, and on twenty
parts spread over a plate the order-bound fold fell from 672 ms to 35 ms
(`cargo bench -p core-engine --bench plate`). A hole is *vented* when any of its pixels lies
over open air in the layer under it, or over a hole that was; the first layer lies on the
plate, so its holes are sealed. A sealed hole carries its cup up the stack while it stays
sealed, adding its area times the layer's thickness; the cup is named once, at the layer it
is widest, when a full vacuum there pulls at least `CUP_LIMIT_N` (10 N, a third of the peel
limit: `MIN_CUP_MM2`, some 99 mm²) and it holds at least `MIN_CUP_MM3` (0.02 ml). Anything
less pulls too little beside the lift to matter (ADR 0417). A model standing on supports has open air
between them, so its cavities start vented; one lying on the plate or on its own floor does
not.

A **thin section** is a piece held from below whose area, as a disc, is under `THIN_MM`
(0.15 mm) across: narrower than the finest support tip, so a tip is never named. A run of
thin pieces standing on each other is named once, where it starts, and only once it is
`MIN_THIN_RUN_MM` (1 mm) tall: a shorter one is the point a curved surface starts on over a
tip, or the end of a spike, and neither has anything to carry.
