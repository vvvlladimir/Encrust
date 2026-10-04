# 0006. Classify vertices by strict sign, stitch contours by mesh edge

- **Status:** Superseded by 0186 (an edge left twice only)
- **Date:** 2026-09-17

## Context

Intersecting a triangle mesh with a plane is trivial until the mesh touches the plane
exactly. A vertex on the plane, an edge in the plane, a whole face in the plane: each one
makes the textbook "one end above, one end below" test produce zero-length segments,
duplicated points or a segment that one face reports and its neighbour does not. The
contour then fails to close.

This is not a rare case. Models are built on a 0.05 mm grid and printed at 0.05 mm layers,
so vertices land on layer boundaries constantly.

A second problem sits behind it. Even away from degeneracies, the interpolation
`v0 + t·(v1 - v0)` is not symmetric: swapping the endpoints changes the last bits of the
result. Two faces sharing an edge then disagree about where the contour point is, and
stitching by coordinate needs a distance tolerance to paper over the difference.

Step 1 already welds vertices and orients faces outwards, so the slicer can rely on real
indexed topology.

## Decision

A vertex is above the plane when `vertex.z > z`, strictly. A vertex exactly on the plane
counts as below it. An edge is crossed when its two ends classify differently.

A contour point is identified by the undirected mesh edge it sits on, written as both
vertex indices with the lower one first, and is always interpolated from the lower index
to the higher one.

Contours are stitched by following those edge keys, not by comparing coordinates. The
segment runs from the edge the face walks down through the plane to the edge it walks up
through it, which puts material on its left.

A broken mesh never fails a slice. Chains that cannot close are closed with a straight
jump and counted in `Sliced`, alongside contours with no area and edges that carried more
than one entry. `SliceEngine::slice` therefore returns `Sliced` rather than `Vec<Layer>`,
and the `SliceError::OpenContour` variant is gone. The CLI decides what to do with the
counts; `--strict` turns any of them into a non-zero exit code.

The mechanics are written up in `docs/design/slicing.md`.

## Consequences

Every degenerate configuration resolves without an epsilon, a perturbation or a tolerance,
and the result is bit-for-bit reproducible across platforms. Around a triangle the
classification flips zero or two times and never once, so a crossing face always yields
exactly one entry and one exit edge and that branch cannot be reached.

Stitching is `O(n)` through a hash map and needs no spatial index or distance cutoff.
Adjacent segments share exact coordinates, so rasterisation in step 3 will not meet
hairline gaps.

Slicing is half-open in Z, `[z_min, z_max)`. A plane at exactly `z_max` returns nothing,
which is why `plane_heights` never places one there.

The cost is a hard dependency on welded, consistently wound input. On an unwelded STL no
edge is shared and nothing stitches at all. That is acceptable because the CLI welds
before slicing, but any future caller of `SliceEngine` has to do the same, and
`--no-validate` skipping the orientation fix will produce inside-out windings.

Reopen this if a mesh format arrives that cannot be welded into indexed topology, or if
profiling shows the hash map dominating a large slice.

## Alternatives considered

### Shift the plane by an epsilon so no vertex lies on it

A common fudge. It works, but a fudge is what it is: it biases every layer by the epsilon, and
it fails exactly when it is needed most, because a model built on the same grid as the
layer height can put vertices on the shifted plane too. Choosing the epsilon is a guess
that depends on model scale.

### Stitch by hashing the coordinates of the endpoints

What a slicer built for broken meshes does, with a 0.5 mm nearest-point search. It
tolerates meshes with no usable topology, but it is `O(n²)` per layer and the cutoff is a
magic number that silently joins contours that should have stayed apart.

### The option that won, and what it costs

Classification by strict sign plus topological stitching is exact, but it moves the burden
onto the import stage: the slicer is only correct on a welded, oriented mesh, and it has
no way to notice that its input was neither. A mesh that is merely *nearly* welded — two
vertices 0.02 mm apart where the weld tolerance was 1e-5 mm — produces open contours that
get closed over silently, and the counter in `Sliced` is the only sign of it.

The half-open convention also means a face lying exactly at `z_max` contributes nothing at
all, which is invisible until someone asks why a model with a flat top prints its last
layer from the walls rather than the lid.
