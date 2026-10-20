# 0316. Union many solids by groups, set apart ones side by side, and slim the engine

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

Exact booleans sit behind the open floor, the lattice, Combine, text and joints, and most of
those union many parts: the letters of a label, the shells of a model, the cutters of a
difference. `core-volume` unioned them down a tree with a real boolean at every node, so two
parts that never meet still paid for both of their faces: 200 balls of 2 k faces, apart,
took 0.54 s.

Inside the engine (ADR 0315) one boolean on a million faces spent 7% of its time finding
crossings. The rest was work over the whole model: the result rebuilt from its triangles as
if read anew, a coplanar flood on one thread for every operand, a fresh `Vec` for every
untouched triangle, simplification checks walked on one thread. One such boolean peaked at
1.4 GB, and at four million faces an 8 GB machine swapped. Reading a solid, `core-csg` split
seams that nearly every model does not have.

## Decision

- **`Solid::union_all`.** Solids whose boxes meet, touching included, form a group; each group
  is unioned down a tree on every core, a part with more faces than the rest together taken
  in last; groups, and halves of a tree whose boxes do not meet, are set side by side by the
  engine's `Manifold::compose` with no boolean. Every union of many in `core-volume` goes
  through it; intersection keeps its tree.
- **A solid with no seam skips the split.** When every edge is walked once each way and every
  vertex carries one fan, `core-csg` hands the engine the welded mesh as the split would have
  numbered it.
- **The fork moves on.** `boolmesh` is pinned to `48b09fd` of its `parallel` branch, which adds
  `compose`; takes a boolean's result from its paired halfedges instead of rebuilding them;
  groups coplanar faces when an operand first needs it, walking only faces with a neighbour
  in their plane; pairs triangles, finds pinched vertices and duplicate edges, and places
  untouched edges on every core; stores indices as `u32` and the collider's boxes as `f32`
  rounded outward.

Every engine change gives the same result bit for bit as ADR 0315's commit, on and off
`rayon`, as its `examples/golden.rs` checks; `compose` and `union_all` are new answers.

## Consequences

On an M1, reading a million-face ball falls from 0.35 s to 0.19 s and cutting a hundred
prisms out of it from 0.58 s to 0.38 s (`core-csg`'s `solid` bench). The peak of that
boolean falls from 1.4 GB to 0.9 GB. 200 balls apart unite in 36 ms instead of 539, and 40
crossing struts in 18 ms instead of 22.

A union of parts that do not meet keeps each part's triangles as they were, where a boolean
would have renumbered them. Parts whose boxes meet but whose surfaces do not still go
through the engine: right, only slower.

The fork is further from upstream, and holds more `unsafe`: one helper for slices written
from several threads, each slot by one.

Reopen if a model whose boxes chain into one group of thousands — a scaffold — is common
enough that the tree's order inside a group matters, or when upstream takes the changes.

## Alternatives considered

### A lazy tree of operations, as the C++ original keeps

Reorders and batches every boolean a caller asks for, not only unions, but changes what a
`Solid` is for every caller at once. The unions of many are where nearly all of the gain is.

### Keep every model's solid beside it in the scene

Saves reading a model again between tools, at about 0.3 GB a million faces held for as long
as the model is open; on 8 GB that costs more than the 0.2 s a read now takes.

### The option that won, and what it costs

A box test decides between a boolean and none, so a union that skips the engine must be
right for every pair whose boxes do not meet; it is, as such solids share no point. The
fork's changes are ours to carry until upstream wants them.
