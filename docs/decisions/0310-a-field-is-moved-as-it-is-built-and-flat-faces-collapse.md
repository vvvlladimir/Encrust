# 0310. Move a relief's field as it is built, and collapse faces with no area

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A relief pressed into the Detail test came back marked broken, and Repair left it so. The
field is quantised, a lattice point can read exactly zero, and every cell around such a
point puts its vertex on it: two vertices on one point, or an `f32` rounding apart, and a
triangle between them with no area. Repair dropped doubled faces and closed holes but had
nothing for a face with no area. The same press took 14 s at 0.05 mm, half of it a second
nearest-point query per lattice point after the build had already made one, and a third of
what was left `Bvh::closest` searching from an unbounded start in `refine`.

## Decision

`core_geometry::collapse_flat_faces` welds the corners of each face with no area that stand
within `1e-4` mm of each other and drops what that leaves with a repeated corner. `extract`
runs it on every mesh it returns, and Repair runs it first. `core_geometry::Bvh` gains
`closest_within`, and `refine` bounds its exact search by the carried face's distance.
`core_volume` gains a crate-private `build_moved` that takes each value through a closure
given the voxel's nearest point as it is built; `press` uses it, with the band centred on
the middle of the range the surface can move over. `Sdf` keys its tiles by `FastMap` and
hands `extract` a block with eight tile lookups.

## Consequences

No relief of the twelve built-in textures over the Detail test, across depth, sign,
symmetry, tile and mask, leaves an open, degenerate or doubled face; Repair mends one that
does. `press a 0.4 mm relief into a 40 mm ball` fell from 2.83 s to 1.84 s, `extract a 0.1 mm
field` from 142 ms to 106 ms, `hollow a 40 mm ball` from 449 ms to 425 ms; the Detail test
press from 14.4 s to 6.8 s. A face whose three corners lie on one line further apart is not
collapsed and still reads as degenerate; none has been seen. Surface nets still branches
an edge where a feature is thinner than a voxel, which the window notes but does not call
broken.

## Alternatives considered

### Keeping a lattice point off zero

Nudging a zero to the smallest step either side moves every vertex near it by a step, and
two cells can still round to one point in `f32`.

### Dropping the faces with no area

Leaves the surface open along the edge they spanned, which is broken in a different way.

### The option that won, and what it costs

A weld pass over every extraction, a closure threaded through the sweep, and a field build
that is no longer the only way values are written.
