# Parts

How `core_volume::parts` splits a model into the parts a user would name. Why it is done
this way is in ADR 0306.

## Shells

The model is welded at `DEFAULT_WELD_TOLERANCE` first, so a seam the indices left open
does not split it. Faces are then joined by union-find across every edge exactly two faces
use. An edge with one face is the rim of a hole and joins nothing. An edge with three or
four faces is where two bodies meet, and joins nothing either. A corner shared by two
bodies joins nothing, since only edges join.

## Nesting

Shells are taken largest bounding box first. Each one is tested against the earlier shells
from the smallest up, and it goes with the first whose box holds its box and whose winding
number is over a half at most of its sampled vertices (sixteen, evenly through its
vertices). It goes with "most" rather than "all", since a shell resting in another often
shares a face with it. The winding of the outer shell alone is used, so a lattice sitting
in a cavity is inside the body's outer shell, and goes with it. A shell inside a shell that
is itself inside another goes with the outermost.

## Size

A closed body's size is its volume. An open one has no volume, so its size is that of the
cube whose six faces have its area, `(area / 6)^1.5`. A torn but large surface then stays a
part, and a few loose triangles do not.

## Narrow joins

With `half` = neck / 2, the body's field is built about the surface `half` inside it
(`iso_mm = -half`). The lattice is `half / 2.5`, or coarser where the model's area asks
for it, and coarser again where the field would pass its budget. The field is extracted
into cores, one per piece the erosion left. A join narrower than the neck vanishes, so the
bodies either side of it become separate cores.

Cores holding less than the smallest part are noise and are not counted. With two or more
left, every core but the largest gets a cutter: its own field built about the surface
`half + 2 voxels` outside it. The cutter reaches past the body the core came from
everywhere but at the join, where it ends in the join. Each cutter is intersected out of
what is left of the body, and the largest core's part is what remains. So the pieces always
add up to the body exactly.

A piece can come out in several shells: a fin of the neighbouring part caught in the
cutter, or a thin feature outside it. Each piece is nested again. Bodies under the smallest
size go back to the nearest piece however far it is, since they are the body's own
material, not scraps of the model's.

## Scraps

A body under the smallest size is joined to the part nearest it if any of its vertices
comes within 0.2 mm of it, measured through each part's `Bvh`. Otherwise it is dropped.
Parts are returned largest first.
