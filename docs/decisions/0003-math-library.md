# 0003. Use glam with parry3d, and f32 scalars

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Every crate in the pipeline needs vectors, and three consumers pull in different
directions. Geometry needs cross products, plane intersections and transforms. Collision
queries — raycasting for object picking in step 5, overhang detection and support
placement in step 7 — need an acceleration structure, and writing a BVH ourselves is work
we would rather borrow. The viewport needs vertex buffers uploaded to the GPU, which means
types with a guaranteed layout that `bytemuck` can cast without copying.

The plan was nalgebra plus parry3d, on the assumption that parry is nalgebra's collision
library. That assumption is out of date: parry3d 0.29 and 0.30 depend on `glamx`, a glam
extension crate, and do not use nalgebra at all. `parry3d::bounding_volume::Aabb` is built
from `glam::Vec3`. Mixing nalgebra points with parry means a conversion at every call.

## Decision

`glam` is the vector library for the whole workspace, and `core-geometry` re-exports
`Vec2`, `Vec3`, `Mat4` and `Quat` so no other crate depends on glam directly. `Aabb` is
re-exported from `parry3d` rather than redefined. `Scalar` is `f32`.

One vector type runs from the STL parser to the GPU buffer. glam's types are already
`bytemuck`-compatible, so a `Vec<Vec3>` of mesh vertices uploads to wgpu as bytes with no
conversion pass. parry3d stays available for BVH construction and raycasting when steps 5
and 7 need them, on the same types.

f32 is enough for the precision that matters. Seven significant digits over a 200 mm build
plate is about 1e-5 mm, two orders of magnitude finer than a 0.01 mm layer and far finer
than any resin printer resolves. Halving the size of a vertex matters more: meshes run to
millions of triangles, and the whole vertex buffer is walked once per layer.

## Consequences

- Mesh vertices reach the GPU with no conversion pass, and the viewport in step 5 has one
  less thing to get wrong.
- parry3d's BVH and raycasting are available on our own types, so picking and overhang
  detection do not need a hand-rolled acceleration structure.
- `Scalar` is a type alias, so a later change to f64 is mechanical in signatures — but not
  in behaviour, and not in parry, which would have to come along.
- Contour stitching in step 2 will need explicit epsilon handling. Near-degenerate
  triangles and vertices that land exactly on a slicing plane are where f32 bites, and the
  tolerance has to be chosen deliberately and documented in `docs/design/slicing.md`
  rather than discovered by a print failing.
- Reopen this if hollowing or mesh booleans in step 8 produce artefacts that trace to
  precision rather than to algorithm choice.

## Alternatives considered

### nalgebra without parry3d

Full linear algebra — decompositions, eigenvectors, generic dimensions — and the strongest
type safety of the options, with points and vectors as distinct types. Rejected because
dropping parry3d means writing our own BVH and raycasting for steps 5 and 7, and because
nalgebra's types still need converting to plain f32 arrays before every GPU upload. Paying
both costs to gain linear algebra we do not currently use is a bad trade.

### nalgebra with an old parry3d

Pinning parry3d to its last nalgebra-based release, around 0.22, would keep the original
plan intact. Rejected: it is a frozen branch that will receive no fixes, and it drags an
old simba and nalgebra into the dependency graph where they will eventually collide with
everything else.

### glam, the option that won, and what it costs

glam is a game-and-graphics maths library, not a linear algebra library. There is no SVD,
no eigendecomposition, no generic dimensions. If auto-orientation by principal component
analysis ever gets built, the symmetric eigensolver has to come from somewhere else — from
`glamx`, which parry already pulls in and which does expose `SymmetricEigen3`, or written
by hand for the 3×3 case. glam also does not distinguish points from vectors: `Vec3` is
both, so a translation applied to a direction is a type-correct bug that the compiler will
not catch. Careful naming is the only defence, and it is weaker than nalgebra's types.
