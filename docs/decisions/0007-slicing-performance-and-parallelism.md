# 0007. Bucket faces by Z and parallelise over layers

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Testing every face against every plane is `O(faces × planes)`. A miniature of 80k faces at
0.05 mm layers is 64 million intersection tests, and the number grows with model height
even though the work per layer does not. A production model of a million faces at a
0.01 mm layer height would be hopeless.

`AGENTS.md` requires a `criterion` benchmark before any optimisation, and a
before-and-after number to justify it.

Layers are independent of each other: nothing a layer computes is needed by the one above
or below. That makes the layer loop the obvious place to parallelise, and it keeps the
result deterministic because `rayon` collects in order.

## Decision

Faces are bucketed once per slicing run by the Z range they span, in `ZBins`. A plane
looks up its bucket and tests only those faces. The bucket count is the number of layers,
capped at 1024.

Faces that lie flat in Z are left out of the buckets entirely: under the classification
rule of ADR 0006 they never cross a plane.

Buckets are a filter, not an answer. A face on a bucket boundary is listed in both
neighbours, and `plane::crossing` decides. Over-reporting is allowed; leaving out a face
that does cross is not.

Layers are sliced in parallel with `rayon::par_iter` over the plane heights.

`crates/core-slicer/benches/slice.rs` benchmarks an 80k-face sphere at 0.1 mm and 0.05 mm
and stays in the repository.

## Consequences

Slicing cost becomes roughly proportional to the number of faces the planes actually
touch, not to model height. Measured on the benchmark: 27.9 ms to 3.6 ms at 0.1 mm layers,
58.7 ms to 6.4 ms at 0.05 mm, a factor of about eight on both.

The `ZBins` build is a sequential pass over the faces before the parallel section. It has
not shown up in the benchmark and is not worth parallelising yet.

Memory grows with how many buckets each face spans. The 1024 cap bounds the worst case: a
face spanning the whole model is listed 1024 times rather than once per layer. A mesh of
very tall thin faces — a lattice, a support tree — is the shape that would make this hurt,
and the benchmark does not cover it.

Reopen this if a real model shows the bucket build or bucket memory dominating, or if a
layer's own work grows enough that parallelising within a layer starts to pay.

## Alternatives considered

### A sweep line with an active list

Sort faces by their lowest Z and sweep the planes upwards, adding and removing faces from
an active set. This is the asymptotically optimal approach,
`O(n log n + k + m)`, with memory linear in the number of faces rather than in how many
buckets they span. It lost because the sweep is inherently sequential in Z, which would
give up the straightforward parallelism over layers for a constant-factor gain the
benchmark does not yet ask for.

### A BVH over the faces

What raycasting keeps anyway. A full 3D acceleration structure is more than a plane
query needs: the query is one-dimensional, so a one-dimensional index is the right size of
tool, and it builds far faster.

### The option that won, and what it costs

Bucketing wastes memory on faces that span many buckets, and the 1024 cap is a number
chosen by judgement rather than measurement — no benchmark in the repository justifies
1024 over 256 or 4096. The buckets are also rebuilt on every call to `slice`, so slicing
the same mesh twice at two layer heights pays for the build twice. Neither is visible at
the sizes measured so far, and both wait for a model that makes them visible.
