# 0258. Cut a model once however many copies of it stand on the plate

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

`bake` merged every model into one mesh, so twenty copies of one model were twenty sets
of triangles to transform, index and cross on every plane. On twenty 80k-triangle spheres
cutting the stack took 151 ms against 8 ms for one, and the plate held twenty copies of
the geometry in memory, though a duplicated or arrayed model already shares one mesh.

A copy that stands the same way at the same height differs from its original only by
where it stands on the plate, and so does every contour of it. The slicer stitches each
mesh by its own topology, so cutting two meshes apart and listing both sets of contours
is what cutting them merged gives; the positive winding rule takes the union.

## Decision

- **`core-slicer` cuts a `Solid`**: one mesh of everything that stands once, and
  `Repeated` bodies each cut once and laid down again at its offsets, contours moved by
  addition. `Windows::cut` and `stream` take a `Source`, which a `Mesh` and a `Solid`
  both are; `Writing::source` and `measure` take one too.
- **`bake` groups the models** that share one mesh and one set of cuts, by `Arc`, and
  one rotation, scale, stretch and height. A group of one goes into the mesh as before.
  A group's supports go into the mesh per model, since each model's were built for it.
- **The shrinkage is kept per copy**: each is still scaled about its own footprint, which
  is an offset the same for every point of it, so a copy's offset carries it.
- **The plan reads each body once** (`Solid::distinct`): a copy moved along the plate adds
  no height and no slope.

## Consequences

On the twenty spheres cutting falls from 151 ms to 9 ms and measuring the plate from
1.31 s to 0.46 s. Rasterising is now most of what is left, and still follows the copies.

A copy's contours are its original's plus an offset, where cutting it gave its own
vertices moved by the placement: they agree to about a micrometre, well inside a pixel,
not to the bit. A copy whose placement differs in any other way — a turn, a scale, a
height, its own cuts — is a body of its own and costs what it did.

## Alternatives considered

### Rasterise once and copy the runs

Removes the rasterising too. Rejected for now: runs only move by whole pixels, so a copy
would move by up to half a pixel, and a support meeting its model would be joined by
taking the brighter pixel rather than by the coverage of the union, which greys the seam.

### Group by comparing the meshes' contents

Would find an equal model imported twice. Rejected: comparing millions of vertices costs
more than it finds, and a copy made in the window already shares its `Arc`.

### The option that won, and what it costs

A second way into the slicer, `Source`, and a bake that is no longer one mesh: whatever
read the baked mesh reads `Solid::mesh` and finds the copies elsewhere.
