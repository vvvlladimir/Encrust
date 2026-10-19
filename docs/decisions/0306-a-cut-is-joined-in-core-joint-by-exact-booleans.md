# 0306. Join a cut in `core-joint`, by pins, dowels, magnets or a key

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

ADR 0223 pinned a cut with a peg appended as a second shell and a socket drilled as a
drain hole, for want of a mesh boolean, and said to reopen it when one landed. `core-csg`
has one now (ADR 0303). The 0223 pins also fall short on a resin printer. A hollow model's
cut face is a wall too thin for any peg. A cut that crosses two legs could get both pins in
one leg. Nothing checked that a socket fits in the half behind it. Two equal pins let the
halves go together turned round. And a peg standing on a cut face keeps that face off the
supports, where the glue would hide their marks.

## Decision

A new crate, `core-joint`, on `core-geometry`, `core-csg`, `rayon` and `serde`, joins the
two halves of a cut. `join` takes the halves, the plane and the uncut model, and `Joints`
picks one of four kinds:

- **Dowels** (the default): sockets in both halves, and vented, chamfered dowels printed
  standing beside them. Both cut faces stay flat.
- **Pins**: vented, chamfered pins welded onto the lower half, sockets in the upper.
- **Magnets**: a socket for every disc magnet in each half.
- **Key**: a tongue following the edge of the lower half's cut face, into a groove in the
  upper, narrowed to the middle of a wall too thin for its own width.

Fixings are placed per separate piece of the cut face. The face is drawn on a grid of at
most 1024 cells across, with an exact distance transform. Fixings are spread by farthest
point, and each one is sounded for depth in both halves by rays through the `Bvh` before
it is taken. Pegs can be sized from each piece's widest inscribed circle. A keyed set has
one narrower peg. In a hollow model, fixings are placed on the model's outside, and a pillar
clipped to it carries a socket where the shell is too thin. All of it is built by
`core-csg`, so each half stays one closed solid. `Joints` is written into `CutState`, and
from there into the project, the preferences and undo. A file without it reads as a plain
cut. The window runs the cut as a job.

`docs/design/joints.md` has the placement and sizes.

## Consequences

A joined half is a single solid that slices, hollows and combines like any other. Sockets
are no longer drain holes: the Drain tool does not show them and cannot clear them. A cut
is a boolean of every joined half, a second or more on a large model, which is why it runs
off the drawing thread. `core-engine` takes `core-joint` for the type it writes. Reopen
this if a fixing has to follow a curved cut, or if users ask for a fixing beyond these four.

## Alternatives considered

### Keep the pegs in `core-geometry`

That is where 0223 put them. But `core-geometry` sits under `core-csg` and cannot use it,
and a peg that is not welded in is the defect being fixed.

### Put the joints in `core-volume`

It already takes `core-csg`, but it owns the distance field. A joint never samples that
field, and placing a joint would then mean building the whole crate.

### A separate crate, and what it costs

One more crate, one more edge to `core-engine`, and a second raster of the cut face beside
the slicer's. The raster is a million cells at most, about 20 MB, and is dropped as soon as
the joint is built.
