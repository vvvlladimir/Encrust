# 0185. A wall is measured on the plate, not in the model's own space

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

ADR 0059 keeps a hollowed mesh in the model's own space and measures the wall there. Every
number the Hollow tool takes — the wall, the infill's cell, a blocker's radius — is typed in
millimetres of the plate, and the model is scaled onto the plate only after the cavity is
cut. A model stretched 1.6 times along X therefore printed a wall 1.6 times thicker along X
and a hive of stretched hexagons, and the resin it reported saving was the model's
unscaled cavity. A relief had the same problem and ADR 0122 answered it the same way.

## Decision

`core_volume::hollow_at_scale` takes the scale the model stands at, hollows a copy scaled
to it — wall, lattice, infill and blockers all in plate millimetres — and divides the
result back into the model's own space. A mirror is an isometry and only the size of the
scale is used. The shell is still kept in the model's own space (ADR 0059), so a model
rescaled afterwards still makes it stale. Both callers that hold a transform — the window's
hollow job and `core-engine` reopening a project — go through it; the command line hollows
a mesh already placed and keeps calling `hollow`.

## Consequences

The wall, the cell and the resin saved are what was typed, whatever the scale. A scaled
model costs one copy of its vertices and a hierarchy built over them for the run; an
unscaled one costs nothing extra. A model scaled to nothing on an axis is refused with
`VolumeError::BadScale` rather than divided by zero.

A blocker is a ball in the model's space and an ellipsoid on the plate; it is laid on the
plate as a ball of its radius through the smallest axis, the factor it was stored under.
A channel's sleeve is sized the same way, so under an uneven scale its wall follows the
smallest axis rather than the wall typed.

## Alternatives considered

### Hollow in the model's space with an anisotropic field

The distance could be measured through the scale inside `build`. It would reach into every
field, the lattice and marching cubes, for what one copy of the vertices does outside them.

### Bake the scale into the model at import of the tool

Keeping the scaled mesh would drop the copy, but the window keeps every model in its own
space so that a transform stays an edit (ADR 0015, 0028); a hollow would be the one thing
that does not.

### The option that won, and what it costs

A copy of the vertices, the faces and a hierarchy for every scaled model hollowed, and a
round trip through a multiply and a divide that may move a vertex of the outer surface by
an ulp.
