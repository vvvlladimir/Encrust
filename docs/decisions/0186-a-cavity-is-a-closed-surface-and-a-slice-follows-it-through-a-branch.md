# 0186. A cavity is a closed surface, and a slice follows it through a branch

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

A sculpted model of a million faces hollowed into a cavity with 674 open and 678 branching
edges, and its slices carried two thousand contours closed over a gap: a straight line
across the layer, and a sliver of resin standing in the cavity. Five things made it, and
each of them alone is enough to break a slice.

- A shell lying inside the model, a stray part every sculpt keeps, was built into the field
  like any other, so the field read its outside as air and cut a wall around it.
- A block of the field far from the surface took its side from the face the carrier handed
  it, which near a thin wall is the far side of that wall (ADR 0082 trusts it for the
  distance, which it is good for, and the side came with it).
- The pseudonormal told an edge from a face by barycentrics, which a sliver triangle loses
  to cancellation.
- Extraction dropped a second copy of a folded triangle wound like the first, and ignored
  a third after two had cancelled (ADR 0084), and lost every quad naming a cell of a tile
  the field did not store.
- The stitcher keyed crossings by their mesh edge, so a second sheet through the same edge
  overwrote the first.

## Decision

The cavity is kept a closed surface by construction, and the slicer follows one through a
branch:

- A shell that the rest of the mesh encloses is left out of the fields and kept in the
  model, so it still prints solid where it is.
- A block's side is asked of the true nearest point. A triangle whose barycentrics cannot
  be trusted is told apart by geometry instead.
- Copies of a triangle are summed the way the surface sums them: two wound against each
  other go, two wound alike both stay. A tile below a stored one that the surface reaches
  into is meshed too.
- The stitcher keeps every crossing leaving an edge and, at an edge left twice, takes the
  sharpest left turn. That traces two loops touching at a point as two loops.

## Consequences

A cavity has no open edge and every edge is crossed back as often as it is crossed, so no
slice of it is closed over a gap. On that model: 2001 gaps to none inside, 2086 to none
through the bottom, 1859 to none as a mould.

The cavity still branches where two sheets of it touch, and `Sliced::unlinked_segments`
still counts each branch, so a hollowed model does not pass `--strict`. The branch is no
longer a defect in the slice, only in the mesh.

Hollowing costs 5 to 7 % more, extraction 4 % and a field build 3 %
(`cargo bench -p core-volume`): a nearest-point query per far block, the tiles below, and a
shell count on every run. A model that is not one piece pays a split and a winding number
on top.

Reopen this if the field grows a mode where a shell inside another means something, such as
a part meant to print hollow inside a solid one.

## Alternatives considered

### Close the gap in the slice

The stitcher already closed a broken chain with a straight jump. That jump is the line
across the layer. Any rule that guesses where a missing piece ran is a guess.

### Weld or remesh the cavity until it is manifold

It would stop the branches being counted, but it is the mesh boolean ADR 0059 refused,
and nothing the rasteriser does needs it.

### The option that won, and what it costs

A branch is followed by a turn rule rather than by the mesh, so two loops touching at a
point come out as two only while the rule holds. Where it does not, they come out as one
loop through the point, which encloses the same area under the non-zero rule but is not
what an offset would want. A shell left out of the field is also never hollowed: a second
body inside the first stays solid.
