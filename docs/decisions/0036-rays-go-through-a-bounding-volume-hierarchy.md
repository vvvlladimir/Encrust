# 0036. Cast every ray through a bounding volume hierarchy

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

`raycast` tested every face of the mesh. ADR 0016 said that was enough because a ray was
cast once per click, and `ray.rs` carried the note that an acceleration structure would
arrive when a benchmark said a click was slow.

Automatic support placement made that false. `ObjectSupports::refresh` drops one ray per
support point, and a run over a real part places hundreds to thousands of them. On a
880 604-triangle model one ray costs 6.99 ms, so a thousand supports cost seven seconds,
single-threaded, on the thread that draws the window. The placement job itself finishes in
82 ms; what the user sees is a progress bar frozen near the end while the window is busy
landing the columns the job already worked out.

A criterion benchmark on a 200k-triangle sphere put the same ray at 1.0537 ms, and 64
support drops at 68.8 ms.

`core-geometry` is a leaf crate that depends on nothing in this workspace, and the
raycast has three callers of very different shapes: picking an object with the mouse,
picking a support column, and landing a column under a contact.

## Decision

`core-geometry` owns a `Bvh`: a bounding volume hierarchy over a mesh's faces, in that
mesh's own space. It is built by median split on the widest axis of the face centroids,
with up to four faces per leaf, and traversed iteratively with the far child culled once
something nearer has been hit.

`raycast_placed` takes the hierarchy of the mesh it is given. Callers hold the two
together: `SceneObject` builds the `Bvh` in `Scene::insert`, beside the `Arc<Mesh>`, so
the two cannot drift apart, and `core_supports::columns` and `landing` take it as an
argument. Points are landed on every core through `rayon`.

`raycast`, which tests every face, stays as the reference the hierarchy is measured and
tested against.

## Consequences

On the 200k-triangle benchmark one ray goes from 1.0537 ms to 497.6 ns, and 64 support
drops from 68.8 ms to 44.5 µs. On the 880 604-triangle model, landing 1600 columns goes
from about 11 s to 1.10 ms, and building the hierarchy costs 167 ms once at import.

Import now pays for a hierarchy nobody may ever cast a ray at. A mesh that is only sliced
and never clicked carries that 167 ms for nothing, which is the price of the guarantee
that the mesh and its hierarchy are never out of step.

The hierarchy holds only for the space it was built in, which is why the ray is still
moved into the model's own space rather than the mesh into the ray's. Anything that edits
a mesh in place has to rebuild it; nothing does today, because placement lives in
`Transform` and supports are meshed separately.

Reopen this if a mesh becomes editable in place, or if the 167 ms of build time starts to
show against the rest of import once import is measured again.

## Alternatives considered

### Keep testing every face, and just move the work off the UI thread

It would stop the window freezing without any new structure. It does not make the work
finish: the user still waits ten seconds for supports, and the same rays are cast again
every time the model is moved or the profile is edited. Picking an object with the mouse
would still cost a millisecond a click.

### A uniform grid instead of a hierarchy

Cheaper to build and simpler to write. It is also much worse on the meshes this slicer
sees: a print-sized model is a thin shell in a large empty box, so the cells are nearly
all empty and the occupied ones hold far more than their share. The hierarchy adapts to
where the faces actually are.

### `parry3d`'s own `Qbvh`

Already a dependency, already tested, and would have been no code at all. It wants the
mesh as a `parry3d` shape, built from its own `Point`/`TriMesh` types, which means either
keeping a second copy of every mesh in memory or converting on every build. A 880k-face
model is 32 MB of vertex data; a second copy of it to answer a ray is not a trade this
project should make while a hundred lines do the job.

### The option that won, and what it costs

A hand-written hierarchy is a hundred and eighty lines that have to be maintained and
tested, in a crate whose point is that it is small. The split is a plain median, not a
surface area heuristic, so the tree is measurably worse than a good one would be on a
mesh with very uneven face sizes — but it builds in a third of the time a binned SAH takes,
and 497 ns a ray is already far past what the callers need. Every caller of
`raycast_placed` now has to carry a `Bvh` next to its mesh, which is a wider API than one
function that only wants a mesh.
