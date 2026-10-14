# Orientation and arranging

Which way up a model prints best, and where several of them go: what `core-plate` decides.

## What is being minimised

An MSLA print fails on the peel before it fails on anything else: every layer is pulled
off the film, and that force follows the cross-section being pulled. So the score is not
the support-volume score an FDM tool uses. Each term is made dimensionless, so that a
miniature and a bust are scored on the same scale:

| Term | What it is |
|---|---|
| support | overhang, tips and support height, over the surface area (below) |
| peel | the area of the disc pulling as hard as the hardest section, over the model's own square length |
| height | how tall the model stands, over the model's own length |
| bottom | area flat on the plate, over the surface area; zero on supports |
| top | the overhang on faces that look up in the file, over the surface area |

The support term adds three things a support is put under:

- the downward area past the overhang angle, weighted by how far past;
- a tip, lower than every vertex it shares an edge with: each one is charged as three
  flat patches one contact holds, `contact_spacing_mm` squared, since the model can hang
  off it;
- half again the overhang for support standing the model's own length tall, so a high
  ceiling costs more than a low one.

A section's pull is Stefan's: each piece pulls with `A⁴ / (4π² J)`, its torsion constant,
the measure `core-analysis` reads off a cured layer (`docs/design/analysis.md`). Scoring
the area of the disc that pulls as hard keeps the term in the units of the shadow it
replaces. A disc pulls the most an area can, so a thin strip or two pieces apart score
less than their area.

| Goal | support | peel | height | bottom | top |
|---|---|---|---|---|---|
| Fewest supports | 1.0 | 0.6 | 0.05 | 0.3 | — |
| Least peel force | 0.5 | 2.0 | 0.05 | 0.3 | — |
| Shortest print | 0.3 | 0.3 | 1.5 | 0.3 | — |
| Clean top surface | 1.0 | 0.6 | 0.05 | 0.3 | 3.0 |

The model's length is the cube root of its bounding box, which does not turn with it, so
dividing by it changes no ranking — it only stops the weights meaning different things at
different sizes. Lower total wins. `orient-lab` (`docs/cli.md`) is what checks the weights.

A model on supports is lifted off the plate, so its underside hangs like any other
surface. Only a model printed straight on the plate rests on it: there a downward face
within 0.5 mm of the lowest point is footprint and not overhang, and a tip there is held.

## The candidates

The model's own flat faces first: normals are binned on a 16-per-axis lattice, the bins
are summed by area, and the largest 24 are kept. A model rests on its own faces, and a bin
is a hash lookup a face where clustering properly would be a comparison a cluster.

Then 200 directions from a Fibonacci lattice, which needs no triangulation and clumps at
no pole. That is what covers a model with no flat face at all.

Anything within 4 degrees of a direction already kept is dropped, and the flats are added
first, so a sphere sample that lands on a face loses to the face.

## Two passes

The peel term needs the model cut; the other three are read off the faces without turning
the mesh at all, since area and normals do not turn with it. So every candidate is scored
on the three cheap terms plus a **shadow** — half the area the faces project onto the
plate, which is the cross-section exactly for a convex model and an upper bound otherwise
— and only the best few are cut at 24 heights to measure the section for real: the area its
contours cover, by the rule the rasteriser fills by, so bodies overlapping at that height
are counted once (ADR 0206).

Without the shadow the first pass would rank on footprint and height alone, and would send
a slab into the second pass lying flat, which is the one orientation the peel cannot take.

The cost is a bias: a candidate whose shadow badly overstates its section can be cut from
the shortlist before it is ever measured. See `docs/decisions/0087`.

The same cut finds **suction cups**: a hole in a section whose inside point stands, in
the section under it, on material or on a hole already sealed. On the plate the first
section's holes are sealed by the plate; on supports air reaches them from below. It is
the rule `core-analysis` follows on the cured stack, read at 24 sections instead of
every layer, so a vent thinner than the spacing between two of them is missed.

## The poses a person tries

Three kinds of candidate are cut whatever the first pass makes of them (ADR 0277):

- the model leaned 15°, 30° and 45° from the way it stands, eight ways round. A leaning
  model's shadow is far larger than any of its sections, so the first pass ranks the very
  pose that cuts the peel last;
- the two best on each reading alone: overhang, shadow, height, top overhang;
- each drain hole pointed at the plate (below).

## Resting on the plate

A model drawn to print as it stands should not be lifted. Where the settings allow it,
the way the model stands and its six largest flat faces are scored set on the plate. A
pose rests when a face lies flat on it, under a hundredth of the surface hangs, nothing is
cupped against the plate and no section tears. A resting pose comes ahead of every
supported one, and is printed with no supports.

A pose that hangs more than that but has a quarter of its shadow flat on the plate may
still print bare: a test piece is drawn to. The score cannot tell, so such a pose comes
last among the finalists and is only ever taken by the proof below.

Set on the plate with no face flat on it, a model is balanced on an edge or a point: that
counts as a failure, under a cup and a tearing pull.

## A model as it prints

What the search is handed is the model the way it will cure, not the way its file drew it
(ADR 0276). The window bakes a scale or a stretch into the mesh first, so every
millimetre above is one on the plate.

A hollowed model keeps two readings apart. Supports stand under the outer surface, so the
first pass reads that alone. The second pass cuts the shell with its drain holes and
channels appended, wound inward: a ring pulls a fraction of the disc it is the rim of, and
the cavity is a cup until a hole lets the air in. Two things keep that reading honest:

- a shell is cut every millimetre, up to 128 sections, because a cavity's floor is one
  wall thick and a section that misses it leaves the cavity looking open from below;
- every run of cuts gets its own planes half a millimetre apart, up to 16, so that one of
  them meets the hole inside the wall it goes through.

A hole vents a cavity from its first layer only when it points almost straight at the
plate, far finer than the sphere is sampled. So each hole pointed down is a candidate of
its own, and is always among those cut.

## Choosing

The shortlisted candidates, and always the way the model stands now, are picked by
priority rather than by the total (ADR 0275):

1. what fails the print: a suction cup, then a pull past the 30 N `core-analysis` names;
2. the goal's own term, unweighted, within a tenth of the best;
3. the total.

The model then stays as it stands unless the turn fails less, puts the goal's own term
back in the band, or wins 5 % of the total.

A wider shortlist and a pattern search around the winner were measured and dropped:
the better the search found low scores, the more of what it found the support generator
left hanging (ADR 0275).

## Proving

The score is a prediction, and what it predicts is measured cheaply once a printer is
known. `core-engine` takes the search's finalists — best first, no two within 10° — and
prints the first four and the way the model stands, with supports stood under each, and
every pose that may rest on the plate bare: the stack cut and cured, and read the way a
finished plate is.

The trials are cut coarse: the tallest pose into about 160 layers, cured at a 0.2 mm
pixel, and all of them at once. A trial ranks poses against each other, and the shape of
a stack settles that long before its last layer does; at the print's own layer and pixel
a trial costs a whole slice, 35 s for five poses of a 230 ml model against 2 s. What a
pose prints at the real resolution is `print_out`'s to say, which is what `orient-lab`
judges the choice on.

A pose set bare on the plate that leaves more than sixteen places hanging is a model
that needs supports, and is dropped. The poses with the fewest suction cups and tearing pulls are kept first: those lose the
whole print, and no goal is worth one. A pose's failures are then its islands and its
contacts with no room, and ten for each cup and tear; set on the plate, everything
placement would have held counts as hanging. A pose is printable within a quarter and two
of the fewest failures: an island more or less is the grain of the support grid, not a
difference between poses. Among the printable, the goal's own reading decides — contacts,
newtons, layers, contacts on up-facing faces — and the model stays as it stands unless a
turn wins 5 % of it.

## Arranging

Footprints are bitmaps, not polygons: one bit a cell of `cell_mm` (1 mm), packed into
64-bit words a row, so testing a part against the plate is an `and` of a few words a row.
A triangle marks the cells its centre-test covers and the cells its edges cross, so a
sliver thinner than a cell is still covered.

A clearance is a dilation of the part before it is painted onto the plate, which is why
`Mask::grown` returns a mask bigger on every side rather than one of the same size.

Packing is first-fit by decreasing area, each part as near the near left corner as it
fits, and the packed block is moved to the middle of the plate once at the end. Packing
from the middle instead splits the free space in two and loses a plate of three models to
one. Why bitmaps rather than no-fit polygons: `docs/decisions/0088`.
