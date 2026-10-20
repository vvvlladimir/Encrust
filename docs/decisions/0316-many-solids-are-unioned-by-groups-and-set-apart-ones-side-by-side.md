# 0316. Union many solids by groups, and set apart ones side by side

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

Exact booleans sit behind the open floor, the lattice, Combine, text and joints, and most of
those union many parts: the letters of a label, the shells of a model, the cutters of a
difference. `core-volume` unioned them down a tree with a real boolean at every node, so two
parts that never meet still paid for both of their faces: 200 balls of 2 k faces, apart,
took 0.54 s.

Reading a solid, `core-csg` also split seams that nearly every model does not have.

## Decision

- **`Solid::union_all`.** Solids whose boxes meet, touching included, form a group; each group
  is unioned down a tree on every core, a part with more faces than the rest together taken
  in last; groups, and halves of a tree whose boxes do not meet, are set side by side by the
  engine's `compose` (ADR 0315) with no boolean. Every union of many in `core-volume` goes
  through it; intersection keeps its tree.
- **A solid with no seam skips the split.** When every edge is walked once each way and every
  vertex carries one fan, `core-csg` hands the engine the welded mesh as the split would have
  numbered it.

`compose` and `union_all` are new answers; every other result is the same bit for bit.

## Consequences

On an M1, 200 balls apart unite in 36 ms instead of 539, and 40 crossing struts in 18 ms
instead of 22. With the slimmer engine of ADR 0315, reading a million-face ball falls from
0.35 s to 0.19 s and cutting a hundred prisms out of it from 0.58 s to 0.38 s (`core-csg`'s
`solid` bench).

A union of parts that do not meet keeps each part's triangles as they were, where a boolean
would have renumbered them. Parts whose boxes meet but whose surfaces do not still go
through the engine: right, only slower.

Reopen if a model whose boxes chain into one group of thousands — a scaffold — is common
enough that the tree's order inside a group matters.

## Alternatives considered

### A lazy tree of operations, as the C++ original keeps

Reorders and batches every boolean a caller asks for, not only unions, but changes what a
`Solid` is for every caller at once. The unions of many are where nearly all of the gain is.

### Keep every model's solid beside it in the scene

Saves reading a model again between tools, at about 0.3 GB a million faces held for as long
as the model is open; on 8 GB that costs more than the 0.2 s a read now takes.

### The option that won, and what it costs

A box test decides between a boolean and none, so a union that skips the engine must be
right for every pair whose boxes do not meet; it is, as such solids share no point.
