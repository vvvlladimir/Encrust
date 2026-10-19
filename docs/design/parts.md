# Parts

How `core_volume::parts` splits a model into the parts a user would name. Why it is done
this way is in ADR 0307.

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

## A marked part

`part_off` takes a point on the model, on a lattice that puts 2²⁴ points over its box,
never under 0.05 mm. Which points are inside is read off the faces, not off a distance
field. Each face is projected down onto the columns of points it covers, giving where it
crosses each and whether it faces up or down. Going up a column, a face turned down is a
way in and one turned up a way out, so a point is inside where the count below it is
positive. That is the winding number along a vertical ray, which overlapping shells do
not fool. A column through an edge two faces share goes to exactly one of them, by which
way the edge runs, in `f64`, where the products of `f32` differences are exact. It costs a
pass over the faces and a sort of the crossings, tens of milliseconds on a scan.

The wave starts at the inside lattice point nearest the mark and steps to the six
neighbours of each point, so a level is every point that many steps from it.

The front of a level is its points; its area is their count, averaged over five levels.
A level is a neck when its front is at most half both the widest front before it and the
widest after it, and the neck is the level where that ratio is greatest. Comparing with
the widest before, not with the start, is what keeps the small fronts round the mark from
reading as a neck.

A level's front can be in pieces, since the wave is still filling corners behind it. Each
piece is followed onward through the levels after it, and the piece most of the wave came
through is the neck's. The plane goes through its centre. Its normal runs from the centre
of the three levels behind that piece to the centre of the three in front, which point
the wave came in. The square cut is that piece's widest in-plane reach plus three voxels,
either way.

A second wave starts from the mark and may not step across that square. If it reaches the
neck's points three levels on, it got round the square, which is widened by half, twice at
most, before the answer is `NoNeck`. A box with one face on the square and as deep as its
half-width, on the mark's side, cuts the model into what is inside and what is outside it.
Each body of each is voted for by its vertices more than a voxel off the plane. Each
vertex counts for the side its nearest inside lattice point is on, by whether the second
wave reached it. A body with no votes is the mark's where it came from inside the box. The
mark's bodies inside and outside the box are joined by exact union, and so are the rest's.
The box's other faces leave no seam that union does not take back out. Reading a side as a
solid costs about what joining it does, so where a side's union would go through most of
the model, the other side is joined instead and taken out of the model's solid, read once
beside the wave.

A wave that dies out with no neck, having filled under half the model's volume, was held
in by something finer than its lattice. The lattice is narrowed once onto the box round
what it filled, grown by half its size either way, with as many points over that, and the
inside read again over it alone. A
partition the coarse lattice missed is then found as a neck. A wave still held in is a
piece no material the finer lattice sees joins to the rest, as on a scan whose surface
folds round it. That piece is grown two voxels out into the empty space round it, never
onto a voxel beside the rest's inside, and the closed surface round those voxels cuts the
model by exact intersection and difference. The surfaces of both stay the model's own,
except where the cutter crosses whatever thin fold joined them.

## Scraps

A body under the smallest size is joined to the part nearest it if any of its vertices
comes within 0.2 mm of it, measured through each part's `Bvh`. Otherwise it is dropped.
Parts are returned largest first.
