# 0306. Split a model by shared edges, with its scraps, and part it at narrow joins

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Split into its parts used `core_geometry::split`, which puts two faces in the same piece
whenever they share a vertex index. On a scan, or on the halves of a cut through one, that
goes wrong three ways. Each sliver the cut leaves becomes a part of its own on the plate.
Two bodies touching at a corner or along an edge stay one part. A body joined to the rest
by a thin strand stays one part, though a user sees it as separate. A hollowed model is a
fourth case: its cavity and lattice are shells of their own, and must stay with the body
around them.

## Decision

`core_volume::parts` splits a model in four steps. `PartSettings` is written into
`CutState`, and a file without it reads the defaults.

1. **Shells by edges.** The model is welded, then its faces are joined only across edges
   that exactly two faces share. Bodies meeting at a corner or along a non-manifold edge
   come apart.
2. **Nesting.** Each shell goes with the smallest shell round it, tested by bounding box
   and then by the winding number at its sampled vertices. Cavities, lattices and sunk
   bodies stay with their body.
3. **Narrow joins** (off by default). A body is eroded by half the neck width, as an
   inward-offset field, and extracted. Each core holding at least the smallest part's
   volume stands for a part. Each smaller core is grown back past where it was and cut out
   of the body by exact intersection, and the largest core keeps the rest. The pieces
   always add up to the body. A scrap the cut leaves goes back to the nearest piece. A body
   the exact cut cannot read is left whole and counted.
4. **Scraps.** A body under the smallest size (2 mm³ by default) is joined to a part
   within 0.2 mm of it, or dropped where it touches none. Size is volume for a closed body,
   or for open surface the volume of the cube of the same area. A model that is all scraps
   keeps them all.

The window runs the split as a job. `docs/design/parts.md` has the details.

## Consequences

The plate gets parts a user would name, and never stray slivers. A model left one part
whose scraps were removed is replaced by the cleaned part under its own name, losing its
supports as any split does. Parting at narrow joins costs two field builds and two exact
booleans per extra part, seconds on a scan. The new face across a join has the lattice's
finish; every other surface is the model's own. Reopen this if parts should be split along
a surface the user draws, or if thin features of one part get parted off as their own.

## Alternatives considered

### Shape-diameter segmentation with a graph cut

This is the classic way to segment a mesh into parts by thickness. It labels faces, not
volumes, so the cut through a join still has to be built, and averaging over a cone of
rays smooths away the very bottleneck being looked for.

### A minimum cut on the face graph

It finds the fewest faces to cut, which on a scan is often a seam of slivers rather than
the narrow join, and it too leaves the new face to be built.

### Erosion, and what it costs

A join is found only down to the lattice it is sampled on. Features of one part thinner
than the neck can make a core of their own if they hold more than the smallest part. That
is why the search is off by default and its width is the user's.
