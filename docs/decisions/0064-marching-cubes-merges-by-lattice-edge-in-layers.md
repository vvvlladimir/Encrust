# 0064. Name a marching cubes vertex by its lattice edge, and merge a layer at a time

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

Extraction ran marching cubes over every tile of the field in parallel, concatenated the
tile meshes and welded the result by position, because two tiles meeting on a plane each
wrote their own copy of every vertex on it.

That held three copies of the whole surface at once — the parts, the concatenation and the
weld's output — at the moment the weld ran, on a mesh that is millions of triangles on a
real model. The weld itself was also the single slowest step of a hollowing run: 1.16 s of
the 12.5 s a 20 mm ball at 0.1 mm took, against 0.37 s for all the marching cubes.

A vertex cut by marching cubes does not need to be found by position. It lies on one edge
of the lattice, and the lattice is global: voxel zero is the origin of the space, so the
edge from a voxel along an axis names the vertex exactly, in every tile that writes it.

## Decision

Each tile's part carries, beside its vertices and faces, the lattice edge every vertex
sits on, packed into a `u64`: twenty bits an axis and two for the axis the edge runs
along. An edge lying inside the tile is marked `NOT_SHARED` and skips the table entirely,
which is about three vertices in five.

Tiles are extracted and merged a layer of tile Z at a time, ascending. A layer's tiles are
marched in parallel, then merged into the output one after another through a map from edge
to vertex index. After a layer, the map is cut back to the plane it shares with the layer
above, since no other edge of it can be asked for again.

There is no weld. The surface comes out shared by construction.

## Consequences

Extraction of the same 5 million faces went from 1.16 s to 0.37 s, and on a 406 000-face
model's cavity from 5.70 s to 0.37 s. Peak memory over extraction is the output mesh plus
one layer of parts plus a table over one plane, rather than three copies of everything.

The result is more correct than the weld's, not less: two vertices on one edge are the
same vertex whatever floating point did to their positions, so a seam cannot be left open
by a tolerance that was slightly too tight. Triangles whose corners land twice on one edge
have no area and are dropped in the merge, which is what the weld used to do.

The merge is sequential. It is a hash lookup per vertex against a parallel march, and at
0.37 s for 8 million faces it is not what the run is waiting for, but it is the part that
does not scale with cores.

This is what exposed the hash: a `u64` key multiplied by one odd constant leaves its low
bits barely mixed, and a hash map takes its bucket from the low bits. Keys inside one
layer differ only in their low bits, so the first cut of this change piled millions of
entries into a handful of buckets and made extraction fifteen times slower rather than
three times faster. `FastHasher::finish` now folds the high bits down before returning,
which is also why `weld` — the same hasher — got faster everywhere else.

## Alternatives considered

### Keep the weld, but stream it a slab at a time

Less code to change. Rejected because welding slab by slab leaves the seams between slabs
unwelded unless a second pass stitches them, which is the edge-key idea with extra steps
and a position tolerance still in the middle of it.

### One global edge table rather than one per layer

Simpler: merge every tile against one map. Rejected on memory — a map over every shared
vertex of a large cavity is hundreds of megabytes, which is the thing this change is for.

### The decision above, and what it costs

Vertices are now named by a packed integer, so the lattice has a hard extent: twenty bits
an axis is half a million voxels either way, 26 m at the finest spacing hollowing asks
for. That is far past anything a resin printer can hold, but it is a limit the old
position weld did not have, and nothing in the code checks it. The merge is also
order-dependent in a way the weld was not: tiles must be merged in layer order for the
table to be safe to cut back, so a future parallel merge cannot simply be dropped in.
