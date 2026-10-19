# Exact booleans

`core-csg` unions, subtracts and intersects closed meshes exactly, over `boolmesh`, a pure
Rust port of the Manifold algorithm (ADR 0303). The field in `core-volume` does the same
by sampling; this is the answer when the edges must stay sharp and the faces few.

## A solid

`Solid::new` reads a mesh once: it welds vertices standing on the same exact point, checks
that every edge is walked as often one way as the other, and builds the index the booleans
query. A mesh may hold several shells, but none may cross another or itself. What fails is
`CsgError::NotSolid`; an open scan is exactly that, and a caller that must answer anyway
falls back on the field, which reads anything with a winding number.

The engine wants every edge used by exactly two faces and indexes past its own tables,
panicking, on more. Shells that touch along an edge or at a vertex — surface nets leaves
such seams wherever two sheets of a cavity meet — are split first. The faces round an edge
are sorted by angle about it and paired across each wedge of solid; on a surface folded
through itself the wedges nest rather than alternate, and are matched as brackets are. The
corners each pair joins are the fan one copy of a vertex carries, and every copy of a split
vertex is moved `1e-4` mm into its own fan, since the engine welds on exact positions. Where
a fold pairs the faces so that a vertex still carries both fans, that edge is paired the
next way round and the fans counted again, up to four times. A panic the engine raises
anyway becomes an error.

Every operation answers a new `Solid` and leaves both sides as they were, so one cavity is
cut by several groups in turn without being read again. Nothing at all is the empty
solid, not an error: union and difference pass the other side through, intersection
answers empty. The engine reports an empty result as an error, `"empty pos matrix"`, and
`Solid` turns that one into the empty solid.

The engine works in `f64`; a mesh goes in and comes out as the pipeline's `f32`.

## What it costs

On an M1, a sphere minus a hundred square prisms:

| Sphere faces | Read | Boolean |
|---|---|---|
| 200 k | 120 ms | 200 ms |
| 1 M | 660 ms | 0.94 s |
| 4 M | 4.3 s | 5.0 s |

The weld and the seams run on every core (ADR 0314); the rest of the read and the boolean
are the engine's, on one thread.

Cost follows the faces of both sides and the crossings between them. A union of thousands
of shapes crossing each other — the struts of a scaffold at every node — is many times
dearer than one cut of the same faces; see the lattice in `hollowing.md` for how that is
avoided.

## What it does not promise

The engine walks hash maps, so the same boolean may split a flat polygon of its result
along other diagonals from one run to the next. The surface is the same; the triangle list
is not. Tests compare face counts, volumes and vertices, never the list.

## Combining models

`core_volume::combine` is what the window's Combine models runs (ADR 0304). Each model is
split into its shells and they are unioned first: the engine reads shells that cross — a
part pushed into a base — without a complaint, and then counts what they share twice. A
model with a shell wound inward is a part with a cavity, and is read whole. The models are
read as solids in parallel and folded down a tree, so `n` of them cost `log n` rounds rather
than `n` cuts into one growing whole. The model of the most faces stays out of the tree and
is cut once at the end, as a difference cuts its first once with everything after it unioned:
every pass through the engine costs the faces of both sides, so letters folded into a
million-face model a level at a time cost three passes over it, where one is enough —
`benches/boolean.rs` measures it. If any of them is not a solid the engine reads, all of them are sampled instead,
each a field on one lattice — precision 0.5 at their combined area, coarsened to the budget
as hollowing is — combined by `min`, `max` and `max(a, -b)` and extracted once.
`combine_exact` is the same without the field, refusing with `NotSolid`.

`combine_exact_toward` is what labels take (ADR 0311). The first model is cut by a plane
square to the side the others stand on, 0.5 mm under the lowest of them, and only the side
above goes through the engine with them. The cut makes every point on it once, keyed by the
edge it came from, so both halves carry it bit for bit. On the side below the cap is its last
faces, as many as `Cut::cap_faces` says, and their corners are the seam. In the answer the
cap is the faces with every corner within 0.1 µm of the plane: the engine leaves the cap as
it was, 0.5 mm clear of anything it adds, and the side above holds no face of the model's own
in the plane but one looking down out of it, which a plane at a height the letters set all
but never meets. Both caps go, the seam's points are matched by position — negative zero
taken as zero, which the engine answers for some — and every edge along it must be walked by
one side and back by the other, or the whole model is combined after all. A model that is
torn or of several shells is combined whole from the start: cut, the side below would keep
what the whole cut refuses, or count twice what crossing shells share.

## Primitives

`core-geometry` builds the closed meshes a boolean is usually handed: `cuboid`,
`oriented_box`, a convex `prism` stood upright, and a `cylinder` whose flats
`segments_for` counts from a tolerance in millimetres.
