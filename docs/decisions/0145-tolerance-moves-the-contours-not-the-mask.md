# 0145. Tolerance moves the contours, not the mask

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

ADR 0144 put the tolerance offsets in the resin's profile without applying them. Applying
them means moving each layer's walls by a different amount depending on which side of the
material the wall is on: `hole_offset_mm` inwards on a hole, `outer_offset_mm` outwards on
an outer contour.

A single morphological grow or shrink of the rasterised mask cannot do that. Dilating the
solid moves both walls by the same distance, and the whole point of two numbers is that a
bolt and the hole it goes into miss by different amounts. It is also bounded by the pixel:
a mask knows nothing finer.

Offsetting the contours can, and the sign of a contour's area already says which kind it
is. Doing it correctly is not a per-vertex displacement, though — walls run into each
other, holes close, thin features vanish, and the result has to be re-resolved into
non-self-intersecting rings. That is a boolean engine.

## Decision

`core_slicer::offset_contours` takes one plane's contours and the two offsets, and returns
the plane with its walls moved. It is built on `i_overlay`, a pure-Rust polygon overlay
engine whose `OutlineStyle` carries exactly the two numbers we have: `outer_offset` on
outer rings and `inner_offset` on holes, with the same sign convention. `core-slicer`
gains it as its first dependency beyond `core-geometry`.

The whole plane goes in as one shape rather than contour by contour, because which contour
is a hole in which is what decides where a wall ends up. The join is a bevel: a mitre puts
a spike on a sharp corner, which a panel cannot draw and a part does not want.

`core-pipeline` applies it. `Tolerance` pairs the resin's `Compensation` with its bottom
layer count, and `fold_group` offsets each layer before rasterising it — which is also
before it is measured, so what Preview says the print cures is what the file carries. The
index comes from the fold's own count, because a group is the next layers in print order.

## Consequences

Two behaviours fall out of the geometry rather than out of a choice, and both match what
the printer would have done anyway: a hole narrower than twice its offset closes, and a
wall thinner than twice its outer offset disappears.

Every front end gets it at once — the window, the CLI's sliced files and the CLI's PNG
stack all go through `fold_group` — at the cost of one more argument on it and on
`measure`. `Preview::measure` re-runs when the offsets change, because `Fold` carries them.

An offset costs one boolean pass per layer, inside the window the rasteriser is already
parallel over, so it follows the contours and not the panel. A stack with every offset at
zero pays nothing: `offset_contours` hands the contours straight back.

The signal to reopen: an offset that has to vary across one layer — a tolerance painted on
a feature rather than set for a resin — which this shape cannot express at all.

## Alternatives considered

### Morphological offsetting of the rasterised mask

No new dependency, and a distance transform over runs would be fast. It lost because it
cannot tell a hole's wall from an outer wall, so it collapses `a` and `b` into one number,
and because it cannot move a wall by less than a pixel, which is what the parity offset
exists to do.

### Offset each contour on its own by displacing its vertices

No dependency at all, and correct for a convex ring and a small offset. It lost on the
cases that matter: two walls closing on each other, a hole eaten away, a sharp corner. All
three need the rings re-resolved against each other, which is the boolean engine again,
written worse.

### The option that won, and what it costs

`core-slicer` now has a third-party geometry dependency, and the offsetting quality —
joins, how a collapsing feature is dropped, numerical robustness near a spike — is
`i_overlay`'s judgement and not ours. The engine works in fixed point internally, so a
result is quantised at its own scale rather than at ours, which we neither choose nor see.
