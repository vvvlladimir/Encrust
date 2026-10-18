# Exact booleans

`core-csg` unions, subtracts and intersects closed meshes exactly, over `boolmesh`, a pure
Rust port of the Manifold algorithm (ADR 0302). The field in `core-volume` does the same
by sampling; this is the answer when the edges must stay sharp and the faces few.

## A solid

`Solid::new` reads a mesh once: it welds vertices standing on the same exact point, checks
that every edge has two faces wound opposite ways, and builds the index the booleans query.
A mesh may hold several shells, but none may cross another or itself. What fails is
`CsgError::NotSolid`, carrying the engine's own reason; an open scan or a model with
self-intersections is exactly that, and a caller that must answer anyway falls back on
the field, which reads anything with a winding number.

Every operation answers a new `Solid` and leaves both sides as they were, so one cavity is
cut by several groups in turn without being read again. Nothing at all is the empty
solid, not an error: union and difference pass the other side through, intersection
answers empty. The engine reports an empty result as an error, `"empty pos matrix"`, and
`Solid` turns that one into the empty solid.

The engine works in `f64`; a mesh goes in and comes out as the pipeline's `f32`.

## What it costs

On an M-series laptop, a sphere minus a hundred square prisms:

| Sphere faces | Read | Boolean |
|---|---|---|
| 20 k | 11 ms | 30 ms |
| 200 k | 130 ms | 200 ms |
| 1 M | 640 ms | 1.0 s |

Cost follows the faces of both sides and the crossings between them. A union of thousands
of shapes crossing each other — the struts of a scaffold at every node — is many times
dearer than one cut of the same faces; see the lattice in `hollowing.md` for how that is
avoided.

## What it does not promise

The engine walks hash maps, so the same boolean may split a flat polygon of its result
along other diagonals from one run to the next. The surface is the same; the triangle list
is not. Tests compare face counts, volumes and vertices, never the list.

## Primitives

`core-geometry` builds the closed meshes a boolean is usually handed: `cuboid`,
`oriented_box`, a convex `prism` stood upright, and a `cylinder` whose flats
`segments_for` counts from a tolerance in millimetres.
