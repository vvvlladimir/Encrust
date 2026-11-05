# 0400. Find a walk over a surface as an A* corridor pulled taut

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Measure gives the straight distance between two points of a model. Someone sizing a strap,
a seam or a label on a curved model wants the distance over the surface instead, drawn on
the model, in the window and in `encrust measure`. Models run to a million faces and the
answer has to come back in well under a second, off the drawing thread. Rule 7 of
`architecture.md` rules out a pass over every face for each query.

## Decision

`core_geometry::surface_path` finds a corridor of faces by A* between three points on each
shared edge, unfolds the corridor into a plane and pulls the line taut through its edges
with the simple stupid funnel algorithm. The result is a polyline on the surface with its
length; the window runs it on a `Worker`, the command line runs it directly, and a
watcher it tells how far it has got can stop it. Method and
tolerances: `docs/design/surface-path.md`.

## Consequences

No new dependency. Between two ends one surface joins, the cost grows with the faces near
the straight line between them, not with the model; ends on two shells are refused at once
by shell labels kept with the `Adjacency`. On a ball of 200k faces the `surface_path` bench
reads 23 µs for a few millimetres, 9 ms for a quarter round and 0.48 s for half round; the
window runs it with its progress and Cancel at the stage's foot. The length is exact on a
strip that unfolds flat and can be a little long elsewhere: the proptest holds a coarse
ball within 5 % and 0.1 mm of the great-circle arc.
A measurement worth more than that, or a report of a visibly bent walk, reopens this.

## Alternatives considered

### Dijkstra over the mesh's edges

Simplest to write, but the walk zigzags along edges and reads 10 to 40 % long, which is no
measurement.

### An exact geodesic (MMP or ICH)

Exact, but hundreds of lines of window propagation with delicate edge cases, and slower on
a dense model, for a reading shown to a hundredth of a millimetre on a mesh that is itself
an approximation.

### The heat method

Needs a sparse linear solver, a new dependency, and a factorisation per model.

### The corridor pulled taut, and what it costs

The corridor can miss the true shortest walk, and nothing says by how much on a given
model; three crossing points per edge triple A*'s nodes to keep that small.
