# 0041. Mesh a support as overlapping closed tubes rather than one welded solid

- **Status:** Accepted, with the strut extension superseded by 0044
- **Date:** 2026-09-19

## Context

A vertical column is a solid of revolution about one axis, and step 7a lathed it as one
closed shell. A branched support is not: struts arrive at a joint from different
directions, and the joint is where three or more tubes of different radii meet.

Meshing that as a single manifold shell is the hard part of every tree support
implementation. The joint needs either a mitred cross-section, which self-intersects when
the bend is sharp relative to the radius, or an implicit surface that is polygonised, which
is a marching-cubes pass and a whole new failure mode for a mesh this project then has to
slice.

What the mesh is actually for is narrow. It is appended to the model and handed to the
slicer, and `docs/decisions/0008` fixes the fill rule there: contours are filled by
**non-zero winding**. Two closed, outward-wound solids that overlap therefore rasterise as
their union, with the overlap exposed once.

## Decision

A support is meshed as **one closed tube per strut**, plus one for the trunk under the
root. Tubes overlap; they are never welded.

Each tube is a straight run: an axis, a list of rings along it, a cap on top and a flat
disc underneath. A tube whose top is a contact gets the tip cone of
`docs/decisions/0028`, pointing straight up into the model whatever way the strut leans,
and the widening top segment of the profile. The trunk carries the pillar, the foot
shoulder and the foot.

A strut does not stop at the joint it ends at. It reaches `parent.radius_mm` further along
its own direction, clamped so it cannot poke out of the bottom of what the support stands
on. A strut leans at most `branching.max_angle_deg` from vertical, so that reach moves it
sideways by less than the trunk's own radius and it stays inside the trunk: the two solids
genuinely overlap rather than meeting at a single circle.

The ring polygon is drawn in a frame derived from the tube's own direction, chosen so that
a tube pointing straight down uses the plate's own X and Y. A vertical column therefore
produces the identical mesh to step 7a, vertex for vertex.

## Consequences

The joint needs no geometry of its own. There is no mitre to invert, no sphere to
polygonise, and no case analysis on how many struts meet at a node.

Every tube is closed and outward-wound on its own, so `diagnose` reports zero boundary
edges, zero non-manifold edges and zero degenerate faces for a branched support, the same
as for a column. Disjoint and overlapping components share no edges, so the manifold
counts stay clean however many tubes cross.

`crates/core-supports/tests/branching_prints.rs` pins the fill down at the level that
matters: a layer through a joint has several contours, the area the rasteriser exposes
there is strictly less than the areas of those contours added up, and it is never less
than the trunk contained in them. Under the joint, the exposed area is one trunk circle
within 5%.

The support mesh self-intersects by construction, and its signed volume over-counts the
overlaps. Anything that measures resin from `signed_volume` of the support mesh reads a
little high — the bench in `core-supports` says 57% saved on a lifted ball, and the true
figure is slightly better than that. Nothing in the pipeline needs a watertight support
mesh, but the moment something does — a hollowing pass in step 8, or an STL export of the
supported plate — this has to be revisited, and the answer then is a mesh boolean, not a
mitre.

Face count is higher than a welded shell: a branched support pays one extra cap per strut.
On the lifted ball, 37 tips on 5 trunks mesh in 25 µs, against a model that is a million
faces.

## Alternatives considered

### Mitre the rings at each joint and sweep one continuous shell

The textbook answer, and it gives a watertight mesh with the fewest faces. Rejected
because the mitred ring has to be scaled by `1 / cos` of the half-angle, which grows
without bound as the bend sharpens, and because a node where three or more struts meet has
no single bisector to mitre against. It solves a problem — watertightness — that
`docs/decisions/0008` says we do not have.

### Polygonise an implicit surface through the struts

What an organic support generator does, and it gives the smooth fillets at the joints
that make branches pretty and strong. Rejected as a whole subsystem — a distance field, a
marching-cubes pass, and a mesh whose quality then has to be checked — for a step whose
subject is where branches go, not what they look like.

### Overlapping tubes, which is what we do

The honest costs are the two above: the support mesh is not watertight and its volume
over-reads, and the joints are visibly faceted where the tubes cross rather than filleted.
The viewport shows those crossings, because it shades the mesh as it is. A user reading
the joints as a defect is the signal to revisit the look, which is the implicit surface
above, and a separate decision from this one.
