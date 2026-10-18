# 0306. Split a model by shared edges, with its scraps, and cut a marked part off at its narrowest join

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Split into its parts used `core_geometry::split`, which puts two faces in the same piece
whenever they share a vertex index. On a scan, or on the halves of a cut through one, that
goes wrong three ways. Each sliver the cut leaves becomes a part of its own on the plate.
Two bodies touching at a corner or along an edge stay one part. A body joined to the rest
by a thin strand stays one part, though a user sees it as separate. A hollowed model is a
fourth case: its cavity and lattice are shells of their own, and must stay with the body
around them. Scans are thin-walled, so every wall has the same thickness and only how wide
a join is tells it from the bodies either side.

## Decision

`core_volume::parts` splits a model in three steps; `PartSettings`, written into
`CutState`, holds the smallest part.

1. **Shells by edges.** The model is welded, then its faces are joined only across edges
   that exactly two faces share.
2. **Nesting.** Each shell goes with the smallest shell round it, tested by bounding box
   and then by the winding number at its sampled vertices.
3. **Scraps.** A body under the smallest size (2 mm³ by default) is joined to a part
   within 0.2 mm of it, or dropped where it touches none.

`core_volume::part_off` cuts off the part a user marks with a click. A breadth-first wave
runs from the mark through the model's inside on a dense lattice, read off the sign of its
field. The join is the level whose front is narrowest, at least twice over, against the
widest fronts both before and after it. The plane is fitted through the piece of that
front the wave went on through. Only inside a box round the join is the model cut, by
exact booleans. Each piece goes to the mark's side or the rest by what the wave reaches
with the plane closed, and each side is joined back by exact union. A wave held in a
small piece is run again on a finer lattice round it. If it is still held in, nothing the
lattice sees joins the piece, and it is cut out along the gap round it. With no join at
all the answer is `NoNeck`, and nothing is cut. The window marks with a one-shot button
in the Cut tool and runs both as a job. `docs/design/parts.md` has the details.

## Consequences

The plate gets parts a user would name, never stray slivers, and a marked part comes off
with one flat face to glue, every other surface the model's own. Cutting one off costs a
field, two waves and four exact booleans, seconds on a scan. The front is counted over the
whole wave, so a part with arms of its own can hide its join from it. Reopen this if
marked parts are cut at the wrong join on real scans, or if joins need cutting along a
curved surface.

## Alternatives considered

### Parting every join narrower than a width, by erosion

Eroding by half a width splits the cores either side of each join. On a thin-walled scan
every wall erodes at once, so it parts everything or nothing. It was the first version of
this record and is gone.

### Shape-diameter segmentation with a graph cut, or a minimum cut on the face graph

Both label faces, not volumes, so the cut through a join still has to be built. Averaging
over a cone of rays smooths away the bottleneck, and the fewest faces to cut on a scan is
often a seam of slivers.

### The wave, and what it costs

It needs a mark, so it finds one join at a time. A join is found only down to the lattice,
and walls thinner than two of its voxels may stop the wave. The flat cut fits only a join
that is roughly straight across.
