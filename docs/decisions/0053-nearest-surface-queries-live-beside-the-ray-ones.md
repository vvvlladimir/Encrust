# 0053. Nearest-surface queries live beside the ray ones, in `core-geometry`

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

Step 8c builds a narrow-band signed distance field. Every voxel of the band asks the same
two questions: how far is the nearest surface, and which side of it am I on. Neither
question exists in the workspace yet — `core-geometry` answers where a *ray* meets a mesh,
and nothing answers where a *point* does.

The distance has to be found through an acceleration structure. A 100 mm model at 0.1 mm
has of the order of 10⁷ voxels in a three-voxel band, and testing every face costs 2.6 ms
a point on a 200k-triangle mesh, which is four hours of work for one field.

The structure it has to go through already exists: `Bvh`, in `core-geometry`, built beside
every mesh the scene holds (ADR 0036). A nearest-point traversal is the same tree with a
different cull, so the choice was where the traversal lives, not which tree it walks.

`core-volume` will be a new crate above `core-geometry`. Rust's coherence rules mean it
cannot add an inherent method to `Bvh`; it would need the node array and the face order
made public, which is the whole of the type's internals, exposed for one consumer.

## Decision

`core-geometry` owns nearest-surface queries, in `closest.rs` beside `ray.rs`:

- `point_triangle`, the seven-region Voronoi test, mirroring `ray_triangle`;
- `point_aabb_squared`, mirroring `ray_aabb`, squared so a traversal compares without a
  square root per node;
- `closest_point`, which tests every face, mirroring `raycast`, and is what the hierarchy
  is measured and tested against;
- `Bvh::closest`, which walks the nearer child first and culls a box further than the best
  hit so far, mirroring `Bvh::raycast`.

`ClosestPoint` carries the point, the unsigned distance in millimetres and the face. The
sign is a separate question with a separate answer; see ADR 0054.

The stack carries each node's box distance alongside its index, so a node the search has
outgrown between being pushed and being popped is dropped without measuring its box a
second time. That is worth 36% of the query on a 200k-triangle sphere.

## Consequences

A band voxel costs 0.40 µs against 2.6 ms testing every face, a factor of 6500, measured
on a 200k-triangle sphere in `benches/field_queries.rs`. `core-volume` takes a `&Bvh`
exactly as `core_supports::columns` already does, and builds nothing of its own.

`core-geometry` grows about 150 lines it has no consumer for until step 8c-ii lands. It
stays a leaf with no new dependency, which is the property that matters.

`Bvh` now serves two query shapes. A third — the conservative all-faces-within-a-radius
query a per-tile field build would want — would be the signal to stop adding methods and
give the traversal its own module.

## Alternatives considered

### `core-volume` builds its own tree

The nearest-point traversal would sit next to the only code that uses it, and `Bvh` would
keep one job. But it means a second hierarchy over the same faces, built and held beside
the first, for a scene that already carries one per mesh — twice the build time and twice
the memory, to avoid a method.

### `Bvh` exposes its nodes

A public node array would let any crate write its own traversal, which is how a general
library would do it. It also freezes the node layout as public API: the `at`/`count`
packing, the left-child-is-next convention and the face grouping could then never change
without breaking a dependent. That is a high price for one caller inside one workspace.

### The option that won, and what it costs

`core-geometry` is no longer only the types and repairs every crate needs: it carries a
query whose only caller is a crate that does not exist yet, which is exactly the
speculative generality `AGENTS.md` rule 2 warns about. The honest
defence is that the trait rule is about extension points, and this is not one — it is a
second question asked of a structure that is already there, whose home is decided by the
language rather than by taste.
