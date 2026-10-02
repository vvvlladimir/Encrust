# 0054. Inside is decided by the generalised winding number

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

A signed distance field needs a sign. `Bvh::closest` gives the distance to the nearest
surface (ADR 0053); which side of that surface a point is on is a separate question, and
on an imported mesh it is the harder one.

The meshes this slicer is given are not clean. `diagnose` exists because STL files arrive
with holes, duplicated faces, non-manifold edges and self-intersections; `orient_outward`
exists because their winding cannot be trusted either. `weld` and `orient_outward` repair
what they can, and `MeshDiagnostics` reports what they cannot. Hollowing is the first
operation whose result is wrong rather than merely ugly when the sign is wrong: a shell
inverted inside a pocket is a solid with a void in the wrong place, and it prints.

Ray parity — counting crossings of a ray to infinity — is the classical answer and is
exactly the one that fails here. One hole in the surface flips the parity of every point
behind it, so the error is a whole region, not a voxel.

## Decision

`core-geometry` owns the generalised winding number, in `winding.rs`:

- `solid_angle`, the signed solid angle a triangle subtends at a point, by Van Oosterom
  and Strackee (1983);
- `winding_number`, which sums every face and is what the hierarchy is measured against;
- `Winding`, a hierarchy after Barill et al., *Fast Winding Numbers for Soups and Clouds*
  (2018): the same median split as `Bvh`, with each node summarised by an area-weighted
  centre, a radius covering its faces and the sum of its area-weighted normals. A node
  further away than `FAR_ENOUGH` times its radius is summarised by its dipole; anything
  nearer is walked and summed exactly.
- `Winding::is_inside`, which reads the sign at the half-winding.

`FAR_ENOUGH` is 2, Barill's own recommendation.

## Consequences

The sign degrades where the mesh does, and only there: a cube missing a face still reads
as solid from well inside it, and a point near the hole reads as a fraction rather than
flipping a whole half-space. That is the behaviour that lets us hollow a model other
slicers refuse.

A query costs 7.3 µs on a 200k-triangle sphere for a point in the band, against 2.4 ms
summing every face. The hierarchy costs 37 ms to build, beside the `Bvh`'s own build.

The dipole truncation error is 3.6% of a winding at two radii, measured on that sphere.
That is far from the half-winding the sign is read against, and far too coarse for anyone
wanting the winding number itself — the fix there is the quadrupole term, not a wider
beta, since the cost roughly doubles per unit of beta while the error falls with its
square.

7.3 µs a voxel is the number step 8c-ii has to answer to: 10⁷ band voxels is 73 seconds of
sign on one core, against a target of a second for the whole field. The sign is the
expensive half of a band voxel, not the distance, and the winding number near a surface is
expensive precisely because the nearby faces cannot be summarised. Step 8c-ii therefore
has to amortise it — one traversal for a tile of 8³ voxels rather than one per voxel, or
the closest face's angle-weighted pseudonormal where `diagnose` says the mesh is closed,
with the winding number as the arbiter where it is not. That decision needs its own ADR
and the numbers to go with it.

## Alternatives considered

### Ray parity

Free, given a `Bvh` that already casts rays, and exact on a closed mesh. One hole makes it
wrong over a region rather than at a point, and a hole is what an imported STL has. It also
needs a tie-break policy for rays that graze an edge, which is its own epsilon problem.

### The closest face's angle-weighted pseudonormal

Nearly free, since the closest face is already known: take the sign of the dot product
between the offset and the angle-weighted normal at the point it lands on. Exact for a
closed, consistently oriented mesh, by Bærentzen and Aanæs. It says nothing useful about a
mesh with a hole, and a self-intersection makes it flip inside the overlap — the two cases
this project has to handle. It stays on the table as a fast path for meshes `diagnose`
calls closed, which is why the two live behind `Bvh::closest` and `Winding` separately
rather than behind one signed-distance call.

### The option that won, and what it costs

The winding number is the slow answer: two structures over one mesh, 37 ms of build, and
7.3 µs a query where the pseudonormal is a dot product. Its accuracy is also approximate
by construction — every answer is a few percent off, which is harmless for a sign and
unusable for anything else. We are paying for correctness on meshes we do not control, and
if step 8c-ii cannot make the field fast enough with it, the fallback is a fast path for
clean meshes rather than a different sign.
