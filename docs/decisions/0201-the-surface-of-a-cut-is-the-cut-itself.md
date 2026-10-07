# 0201. The surface of a cut is the cut's own body, kept where it stands in material

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

Nothing in this workspace cuts a mesh. A drain hole is a closed body appended wound inward
and the fill rule subtracts it when the layer is rasterised (ADR 0059, 0071), and the
viewport discards every fragment standing inside that body per pixel (ADR 0073). Both are
exact. What was never exact is the answer to the question those two leave open: what is
drawn in the opening.

That was `bores` — the wall of each tube and the floor of each hole, built by casting a line
per sector along the tube and keeping the stretches that run through material. It
approximates the curve where the tube meets the model, and an approximation of that curve
fails where the curve leaves one face for another: a strip whose far line had left the
material was dropped, and a slot a sector wide was left through the wall the hole had just
cut. The inside of the model showed through a wall that prints solid. Halving the sector to
find the edge narrows the slot; it does not change what kind of mistake it is, and the next
model — a lattice cell, a curved face, a corner — brings it back.

Every CSG renderer that does not build geometry solves this the same way. Goldfeather's
observation is that only the front faces of intersected primitives and the **back faces of
subtracted** ones are ever visible: the surface of a hole on screen is the far wall of the
cylinder that cut it. That surface is not approximated, because the cylinder is already a
mesh. The only question left is where it stands in material, and that is a parity count —
the one this workspace already does in the stencil to cap a section (ADR 0074).

## Decision

The viewport draws the cut's own body and keeps it where material stands in front of it.
`bores` and everything that fed it are deleted.

Per object carrying cuts, before the models are drawn, into the viewport's own planes:

1. the cut bodies, one copy, into depth alone — the candidate surface;
2. the object's shell with the counting stencil, depth-tested against that candidate and
   writing neither colour nor depth — how much material stands in front of it;
3. the cut bodies again, shaded, where the count ran negative, which is what
   [`inside_only_stencil`] already means;
4. the cut bodies a third time where it did not, writing `frag_depth` of 1, so the
   candidate's depth leaves no ghost over the plate behind it.

The stencil is returned to zero between objects. A cut body is wound inward (ADR 0071), so
the faces that survive culling are the far wall of the tube — the surface being looked
into — with its own normals, shaded like any other surface of the model.

The fragment test of ADR 0073 stays as it is: it is exact for the convex primitives a cut is
made of, and it is what takes the model's own surface off the opening.

## Consequences

An opening is closed by construction. Its edge is the silhouette of the material, resolved
per pixel, so no crack can be left at a corner, a lattice cell or a curved face, and nothing
depends on how finely a tube is facetted.

`core-volume` loses `bores` and the casting behind it, and `ModelHollow` loses the tree over
the shell it built for nothing else. No tool meshes "what the cut looks like" any more: a
tool hands over a body, and the viewport draws it. That is what makes this hold for the cuts
that come later rather than for drains alone.

A count only describes an inside on a closed surface, so a model that is not sound gets a
surface wherever its own parity says material — which may be wrong. Unlike the section cap,
which is turned off there (ADR 0195), this is kept: what it can paint is bounded by the
tube's own screen area, and a hole that draws nothing at all reads as a worse defect than
one whose wall is patchy on a mesh that is already reported as broken.

The cost is one more pass of the object's shell per frame for an object that carries cuts,
and three small passes of the bodies, scissored to the screen box of the cut. The depth
plane is written and wiped in those passes, so the order of the frame now matters: cuts are
resolved before the models.

The signal to reopen is a cut whose body is not convex, or one the fragment test cannot
express. The surface would still be drawn, but the model's own material would stand in front
of it; that is the point at which a real boolean (Manifold) or a subtraction in the field
with a re-extraction has to be taken on. See `local-docs/PLAN-viewport-cuts.md` for what
other slicers do there.

## Alternatives considered

### Keep casting, cast better

More sides on the tube, or halving the sector at the boundary. Both shrink the crack by a
constant and neither removes it, and both pay in rays per hole on every edit.

### Cut the mesh for real

A boolean (CGAL, Manifold) or a subtraction in the SDF with marching cubes, which is what
PrusaSlicer falls back to. It gives geometry everything can use, not just a picture, and it
is the honest answer for a cut that is not a primitive. Rejected here because it remeshes
the model on every change to a hole, which ADR 0059 and 0073 already turned down, and
because it answers a question nobody asked yet: today's cuts are primitives.

### The option that won, and what it costs

The viewport now lies in one more place: it draws a surface the mesh does not have, and the
slicer is what makes that true. A bug that stopped a cut reaching the fill rule would still
look right on screen — the same trade ADR 0073 took, now covering the surface as well as the
hole. The layer preview is what catches it.
