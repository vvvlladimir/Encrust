# 0089. Cut on the mesh, not through a field, and cap with `earcutr`

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

Cutting a model in two is the last tool of step 11, and there were two places it could
live. `core-volume` already intersects fields, so a cut could be a half-space subtracted
from a model's field and meshed back. `core-geometry` has no boolean at all, so a cut
there has to be written: classify, split the straddling triangles, and close each half
over the section.

The two differ in what they give back. A field is a sampling: at the 0.2 mm precision a
bust is hollowed at, a cut face comes back rounded at the corners and costs seconds. A
cut is usually made so the two halves can be printed separately and glued, so the flat
face and the sharp edge are the point of it.

Closing the section is the hard half. The section of a bust is concave, and the section of
a hollowed model is concave with a hole in it. `parry3d`, which is already a dependency,
has ear clipping but only in its 2D build and not publicly.

## Decision

`core-geometry::cut` cuts a mesh with a plane and hands back both halves, each closed.
Triangles are classified with a 0.1 µm band about the plane; straddlers are clipped into
polygons of at most four corners and fanned. A face lying in the plane goes to the lower
half, which is the half it already closes.

The points a cut makes are keyed by the edge they came from, lower vertex index first, so
the two triangles sharing that edge produce one point rather than two a hair apart and the
section's loops close on exact indices instead of on a tolerance. Loops that never close
are counted in `Cut::open_loops` and left uncapped, so a hole in the model shows up as an
uncapped hole rather than as a guessed cap.

The section is triangulated by `earcutr`, the Rust port of `mapbox/earcut`: MIT, no
dependencies of its own, holes handled. Rings are projected into the plane's own basis,
the largest ring says which way round the outside winds, and anything winding the other
way inside it is a hole. Each cap triangle is then wound by checking its normal against
the plane, so neither half comes out inside-out.

`core-geometry::split` breaks a mesh into the pieces that share no vertex, by union-find.

## Consequences

A cut face is exactly flat and the edges stay sharp, at any size of model, and the cut
costs one pass over the triangles rather than a field build. A hollow model cuts into two
hollow halves with the wall capped and the cavity left open, which is what makes the
halves printable.

`core-geometry` now has a dependency that is not `glam` or `parry3d`. It is small and it
does one thing, and the alternative was ear clipping with hole bridging written here,
which is the part of this a wrong answer hides in.

The cut is a plane, not a curve or a sketch, and the window offers the three axes rather
than an arbitrary plane — `cut` itself takes any plane, and a test cuts a cube across its
diagonal. What a cut does not survive is the model's supports, cavity and drain holes:
the halves are new geometry and are placed as new models.

## Alternatives considered

### Subtract a half-space in `core-volume`

No new code for the section: the field already handles holes and concavity, and the cavity
comes out right by construction. Rejected on what it returns — a rounded, sampled cut face
and seconds of field build for something a user expects to be instant and flat.

### Ear clipping with hole bridging, written here

No dependency, full control. Rejected on risk and size: bridging holes correctly needs
visibility tests between rings, and the degenerate cases — a ring touching itself, two
holes sharing a vertex — are exactly what a cut through a real scan produces.

### The decision above, and what it costs

An external crate decides whether a cap is correct, and its failure mode is to return an
error, which this code turns into a silently missing cap for that ring. Nothing in the
tests would catch a section that `earcutr` refuses; only `Cut::open_loops` reports the
other failure, the one this code can see.
