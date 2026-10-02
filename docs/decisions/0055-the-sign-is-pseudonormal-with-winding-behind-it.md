# 0055. The sign is the pseudonormal, with the winding number behind it

- **Status:** Accepted
- **Date:** 2026-09-21

## Context

ADR 0054 gave `core-geometry` the generalised winding number and said why the sign of a
distance field cannot be ray parity: an imported STL has holes, and parity is wrong over a
whole region behind one.

It also left a number to answer to. A band voxel needs one distance and one sign; the
distance costs 0.40 µs through the `Bvh`, and the winding number costs 7.3 µs. A 100 mm
model at 0.1 mm has of the order of 10⁷ band voxels, so the sign alone is 73 seconds of
single-core work against a step target of a second for the whole field. The sign is
eighteen times the distance, and it is expensive exactly where a field lives: near the
surface, where the nearby faces are the ones that cannot be summarised by a dipole.

The alternative was already named there. The angle-weighted pseudonormal of Bærentzen and
Aanæs signs a point by the dot product between the offset and the normal at the nearest
point, which `Bvh::closest` has already found. It is exact on a closed, consistently wound
mesh and says nothing useful on a mesh with a hole.

The pipeline welds and orients every mesh on import, and `diagnose` already reports whether
what came out is closed.

## Decision

`core-volume` decides the sign by `SignMode`, which is `Auto` by default:

- `Auto` runs `diagnose` once and picks `Pseudonormal` when the mesh is closed and
  `Winding` when it is not.
- `Pseudonormal` builds one normal per face, one per vertex weighted by the interior angle
  there, and one per edge summed from its two faces. The nearest point's barycentric
  coordinates say which of the three to sign against, so a point nearest a corner is signed
  by the corner rather than by whichever of the three faces meeting there happened to win.
- `Winding` builds a `Winding` hierarchy and asks it per voxel.

A field is therefore fast on the meshes that deserve it and correct on the ones that do
not, without the caller choosing.

## Consequences

Building the pseudonormals of a 200k-triangle mesh, plus the `diagnose` that chooses them,
costs 40 ms once, against 37 ms for the winding hierarchy. The per-voxel cost is a dot
product rather than a tree walk.

On the 200k-triangle sphere, at 0.4 mm, the whole build is 245 ms by pseudonormal and
375 ms by winding number — 53% more, not the eighteen times the query costs suggest,
because the nearest-point search is what both are paying for and the winding walk is
amortised over a tile's worth of cache-warm traversal. On a dirty mesh the winding number
is therefore an acceptable price rather than a refusal.

`SignMode::Auto` runs `diagnose` on every build, which is 18 ms on 200k triangles and
grows with the mesh, not with the field. A caller that already knows what its mesh is —
the window, which diagnoses on import — should pass the mode rather than `Auto`.

The signal to reopen this is a closed mesh whose field comes out wrong. Self-intersection
is the case the pseudonormal cannot see and `diagnose` does not report: two shells passing
through each other are each closed, and the pseudonormal signs the overlap by whichever
surface is nearer. If that shows up in practice, `Auto` has to consider more than
closedness.

## Alternatives considered

### The winding number everywhere

One code path, correct on everything, and the mode enum disappears. It costs 53% of the
build time on a clean mesh, which is most of them, for an answer that is identical there —
and the margin grows with resolution, because the winding number is per voxel while
`diagnose` is per mesh.

### The pseudonormal everywhere, with a warning

Simplest of all, and the fastest. It hands back a shell with its sign inverted inside every
hole in the model, which is the case this project exists to handle: other slicers refuse
those meshes, and refusing them is the thing we are trying not to do.

### Amortising the winding number over a tile

Walk the hierarchy once per 8³ tile, evaluating the far field at the tile's corners and
interpolating it across, and sum only the near faces per voxel. It keeps one sign path and
makes it much cheaper. The near faces are the bulk of the work for a tile that straddles
the surface, which is every tile a band stores, so the saving is on the cheap half. It also
introduces an interpolation error inside the tile that would need its own bound.

### The option that won, and what it costs

Two sign paths that have to agree, and a test that pins them together on a closed mesh. The
pseudonormal path carries a barycentric classification with a tolerance in it, which is one
more epsilon in the pipeline, and the vertex and edge normals are a second structure over
the mesh beside the `Bvh`. We are also trusting `diagnose`: a mesh it calls closed but that
is inside out in one shell gets the fast path and the wrong answer, where the winding
number would have said the same wrong thing more slowly.
