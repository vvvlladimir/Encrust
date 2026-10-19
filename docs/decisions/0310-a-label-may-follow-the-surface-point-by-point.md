# 0310. Let a label follow the surface, its letters filled fine and laid point by point

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A label is flat (ADR 0304): on a face that curves away by more than the depth, raised
letters stand off the surface at their ends and sunk ones cut deeper there. Text on a
scanned figure, a bottle or a sphere needs every letter to keep the depth off the surface
under it. The flat label keeps its own reason to exist — it never reaches past twice the
depth, so a pillar beside it is never cut.

## Decision

`LabelSettings::follow_surface` picks between the two. Following, each filled shape is
triangulated as before and its edges split at their middles, round after round, until none
is longer than 1/160 of the label's longest side and never under 0.25 mm; a split edge is
split for both triangles on it and put back into the ring the walls run along, so the letter
stays closed. Under every point a ray goes down the plane's normal, or up it where the point
lies inside the model, and the point's bottom and top stand off that height: the overlap
and the depth for a raised letter, the depth and the overlap for a sunk one. Heights beyond
half the label's longest side, or twice its depth, are not taken; a point with none stands
at the mean of the rest. The rays run on `rayon`. The Relief tool's Text mode offers Flat
and On the surface, Flat by default.

## Consequences

Letters keep their depth on any curve, measured along the click's normal: where the face
turns steeply away from it the letter is thinner across the face, by the cosine. A
following label carries hundreds to thousands of triangles a letter, which the preview
rebuilds each frame and the exact join has to cut. A point off the edge of the model is
filled, not refused. Reopen if a letter has to keep its depth square to a steep face, or the
preview stutters on a long label.

## Alternatives considered

### Steiner points inside the fill

A grid of points handed to the triangulator would fill as evenly, but earcut takes none,
and placing them clear of the outline is its own problem.

### Offsetting along each point's own normal

Square to the face everywhere, but a scan's normals are noise, and neighbouring offsets
cross in any hollow tighter than the depth.

### The option that won, and what it costs

Splitting edges leaves the slivers earcut makes as many smaller slivers, so the fill has
more triangles than an even one would. A ray down from inside a model meets the far side
first, which is why the inside is told by the face it meets and the surface looked for above.
