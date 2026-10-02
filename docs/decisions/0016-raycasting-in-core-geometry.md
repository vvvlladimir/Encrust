# 0016. Ray casting lives in core-geometry

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Step 5b selects an object by clicking it, which means turning a cursor position into a ray
and asking which mesh that ray hits first. The intersection itself is two well-known
routines: the slab test against an axis-aligned box, and Möller-Trumbore against a
triangle.

Where they belong is the question. Only `encrust-app` needs them today, and
`AGENTS.md` says not to abstract ahead of time. Against that,
`core-geometry` already owns `Mesh`, `Triangle` and `Aabb`, and the three later features
that want the same routine — overhang detection and support placement in step 7, hollowing
and drain holes in step 8 — are all core work with no window involved.

## Decision

`core-geometry` gains a `ray` module: `Ray`, `RayHit`, `ray_aabb`, `ray_triangle` and
`raycast`. They are free functions, not methods on `Mesh`, because `Mesh` carries data and
not behaviour.

`Ray::new` normalises its direction, so every `t` these functions return is a length in
the ray's own space rather than a parameter of an arbitrary vector. `ray_triangle` counts
a hit on either face: picking has to work on a model whose winding is still wrong, which
is exactly the model the user wants to select and look at.

`encrust-app` keeps only what needs a camera and a scene. `pick.rs` unprojects the cursor,
moves the ray into each object's local space, and compares the hits in plate millimetres.

## Consequences

- Picking is tested against a unit cube with analytically known distances, with no window
  and no GPU. Step 7's overhang work gets the same routine already covered by tests.
- `raycast` tests every face of the mesh. A click is one ray against a few million
  triangles, which is single-digit milliseconds and invisible next to the frame it happens
  in. When it stops being invisible the fix is a BVH behind the same signature, justified
  by a benchmark as `AGENTS.md` requires.
- `core-geometry` grows a fifth public concern. It stays a leaf crate and gains no
  dependency, but the argument that it is only primitives is now weaker.
- The slab test handles the axes the ray does not travel along with an explicit branch
  rather than relying on `0 * inf` producing a NaN that `min` discards. The branchless form
  is correct for scalar `f32` and wrong on at least one of `glam`'s SIMD backends, which is
  how the case of a ray running along a box face first showed up.

## Alternatives considered

### Keep it in encrust-app

Honest to the rule about not abstracting early, and it would keep `core-geometry` to the
types the pipeline moves around. Rejected because it puts geometry that step 7 provably
needs behind a binary crate, where no core crate may reach it, and moving it later means
moving its tests too.

### Use parry3d, which is already a dependency

`parry3d` has `RayCast` implemented for `TriMesh`, with a BVH for free. Rejected because it
would mean converting our `Mesh` into a `parry3d::TriMesh` on every import — a second copy
of every vertex in memory, kept in step with the first — to reuse two functions that come
to sixty lines. `parry3d` is here for `Aabb` and stays there until something needs its
query pipeline rather than its arithmetic.

### A free function on core-geometry, and what it costs

`raycast(&mesh, &ray)` reads worse than `mesh.raycast(&ray)`, and putting behaviour in free
functions means a caller has to know the function exists rather than finding it on the
type. We keep it anyway, because the rule that `Mesh` is data is what has stopped slicing,
rasterising and repair from accumulating on it. The linear scan is also a decision to
revisit: it is fine for a click and would be wrong for anything that casts many rays, which
step 7 might well do.
