# Orientation and arranging

Which way up a model prints best, and where several of them go: what `core-plate` decides.

## What is being minimised

An MSLA print fails on the peel before it fails on anything else: every layer is pulled
off the film, and that force follows the cross-section being pulled. So the score is not
the support-volume score an FDM tool uses. Four terms, each made dimensionless so that a
miniature and a bust are scored on the same scale:

| Term | What it is | Weight |
|---|---|---|
| overhang | downward area past the overhang angle, weighted by how far past, over the surface area | +1.0 |
| peel | the largest cross-section, over the model's own square length | +0.6 |
| height | how tall the model stands, over the model's own length | +0.2 |
| bottom | area flat on the plate, over the surface area | −0.3 |

The model's length is the cube root of its bounding box, which does not turn with it, so
dividing by it changes no ranking — it only stops the weights meaning different things at
different sizes. Lower total wins.

A downward face lying on the plate is held by the plate, so it counts as footprint and not
as overhang. "Lying on the plate" is within 0.5 mm of the lowest point.

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
— and only the best few are cut at 24 heights to measure the section for real.

Without the shadow the first pass would rank on footprint and height alone, and would send
a slab into the second pass lying flat, which is the one orientation the peel cannot take.

The cost is a bias: a candidate whose shadow badly overstates its section can be cut from
the shortlist before it is ever measured. See `docs/decisions/0087`.

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
