# 0069. Walk out to the isosurface instead of taking every tile within reach of it

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

A hollowing field is built around the isosurface `d = -t`, which stands a whole wall
thickness away from the mesh (ADR 0058). `build` found the tiles to fill by taking the
tiles the mesh itself lands in and dilating that set by `rings = |iso| / tile`, a solid
ball of tiles around every one of them.

A ball is the cube of its radius. A 6 mm wall on a 0.14 mm lattice is nine rings, which is
6 859 candidate tiles offered for every tile the mesh touches — and every candidate then
paid for a nearest-point query to be thrown away. Measured with `hollow-lab` on a 60 mm
ball: the field for a 6 mm wall took **378 s** where the same lattice with a 0.5 mm wall
took two. On a 406 000-face model a thick wall puts tens of millions of tiles in the
candidate set before anything is filled, which is hundreds of megabytes of hash table and,
past a point, the crash the window kept dying of.

It also poisoned the budget of ADR 0063, which prices the candidates: a thick wall looked
five times more expensive than it is and was coarsened for no reason.

## Decision

The candidates are found by walking outward from the tiles the mesh lands in, one ring of
neighbours at a time, keeping a tile only while it is still nearer the mesh than the
isosurface's own band. The walk needs one nearest-point query per tile — the query the
fill was going to make anyway — and visits the shell between the mesh and the isosurface
rather than the ball around it.

A tile is filled when it is within the band of the isosurface, which is what the walk
tests as it goes, so `build` now prices exactly the tiles it will fill.

`TILE_COST_BYTES` is re-measured against that: 7.5 kB a tile, from `hollow-lab` on two
walls of a 60 mm ball at 0.05 mm.

## Consequences

The field for a 6 mm wall on the 60 mm ball went from 378 s to 5.2 s. Nothing else about
a run changed: the same tiles are filled and the same surface comes out.

Because the price is now honest, precision is honoured where it used to be refused: the
same ball at the finest setting builds at 0.05 mm where the old estimate coarsened it to
0.088 mm, and does it faster than the coarsened run used to be.

Wall thickness is no longer a cost multiplier, so the Hollow tool's ranges mean what they
say. What is left growing with the settings is the cavity's triangle count, which is
geometry and not waste.

The walk is a wavefront: each ring is measured on every core, but the rings themselves are
in order, so a field whose shell is many tiles deep has that many synchronisation points.
On the numbers above it does not show; a wall of centimetres on a fine lattice would be
where to look.

## Alternatives considered

### Grow every face's bounds by `|iso| + band`

Exact, and what the dilation was avoiding. Rejected on the same grounds as before: on a
large mesh it is tens of millions of hash inserts, one per face per tile it could reach.

### Dilate on a coarse grid, then refine

Dilate blocks of tiles rather than tiles, then test each block's tiles. Cheaper than the
ball by the block volume, but it still offers what it does not need, and it needs a second
grid to reason about. The walk needs neither.

### The decision above, and what it costs

The walk is sequential in its rings where the dilation was one parallel sweep, and it
holds a visited set of every tile it stepped through — the shell, not the surface, so
several times the tiles that end up filled. On a thick wall at a fine lattice that set is
the largest thing in the build, and nothing prices it.
