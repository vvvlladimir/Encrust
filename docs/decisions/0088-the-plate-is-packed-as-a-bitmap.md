# 0088. Pack the plate as a bitmap, into the corner, and centre the block

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

Arranging several models on a plate is a 2D irregular bin-packing problem. The established
answer is `libnest2d`: no-fit polygons and a first-fit selection.
That needs polygon offsetting and polygon booleans, and this workspace has neither, by
decision — nothing here has ever needed a polygon boolean, and taking one on for arranging
would make it a dependency of the plate.

The shapes being packed are concave: a bust's footprint has bays another model nests into,
and a bounding box throws that away. Whatever replaces the no-fit polygon has to keep it.

## Decision

Footprints are bitmaps. Each model's projection onto the plate is rasterised into cells of
`cell_mm` — 1 mm by default — as a `Mask` of one bit a cell packed into 64-bit words, so
testing a part against the plate is an `and` of a few words a row. Triangles are filled by
their cell centres and their edges are walked as well, so a sliver thinner than a cell
still marks what it crosses: the mask covers the model rather than sampling it.

A clearance is a dilation. `Mask::grown` returns a mask larger on every side, and it is
the grown mask that is painted onto the plate, so the gap lands outside the part rather
than eating into it.

Packing is first-fit by decreasing footprint area, and each part goes as near the near
left corner as it will go. The block that results is then moved once, to sit in the middle
of the plate.

## Consequences

Concavity is kept to the cell: a star nests into another star's bays, which a bounding-box
packer cannot do and a no-fit polygon would have needed a boolean for. Eight concave stars
pack onto a Mars 4 plate in 0.95 ms (`cargo bench -p core-plate --bench arrange`).

Packing is exact only to `cell_mm`. At 1 mm the arrangement can waste up to a millimetre a
side against a polygon packer, which on these plates is worth less than the dependency.
The search cost follows the cell size squared, so halving it quadruples the positions
tried; 0.95 ms says there is room for that if anyone wants it.

Models are packed where they stand, in the rotation they carry: nothing is turned about
the vertical to make it fit. A tall thin model lying diagonally therefore packs as its
diagonal bounding shape, and a plate that would fit six of them upright may take four.
That is the first thing to fix if arranging turns out to waste room.

Supports are not part of a footprint. A tree leaning outside its model can end up inside a
neighbour's clearance; the 3 mm default covers the usual lean and nothing guarantees it.
The signal to reopen: a plate that prints with two support trees fused.

## Alternatives considered

### `libnest2d`, or a no-fit polygon written here

The dense answer, and what the FDM slicers ship. Rejected on the dependency: it needs
polygon offsetting and booleans, which nothing else in this workspace wants, and writing a
correct no-fit polygon for concave shapes with holes is a project of its own.

### Bounding boxes, packed as rectangles

Trivial, and fast. Rejected because it cannot nest, which is most of what arranging a
plate of busts is for.

### Packing from the middle outwards, with no centring pass

One pass instead of two, and every part as central as possible. Rejected on a measurement:
the first part lands dead centre and splits the plate into two halves, so three 60 mm
models on a 150 mm plate pack as one instead of two.

### The decision above, and what it costs

A grid, and therefore a resolution: the packer is exact to a millimetre and nothing tells
the user that. First-fit-decreasing is also not optimal — it takes the first hole a part
fits in, not the best one — so a plate can end up with a gap a later, smaller part would
have filled if it had been placed first.
