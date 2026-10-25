# 0331. Measure a fill's rims and inside in from the patch's footprint, each to a profile of its own

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

A filled patch put its rim on the edges the painted faces end on and its grid right up to
them (ADR 0092). A tip centred on the edge is half off the surface it holds, a large flat
underside wants a second row inside the first, and a grid is not the only useful inside: rings
that follow the rim hold an edge-shaped area evenly, and scattered points break the straight
rows a grid leaves to peel. Everything was built to the group in hand, though the rim of a
flat underside wants lighter tips than its middle, or heavier.

## Decision

- **Measured on the footprint.** A rim offset, a second rim and every pattern but a grid with
  no offset are read off the patch's footprint on the plate: the lowest surface over cells of
  a quarter of the finest spacing (0.1 to 0.5 mm), and each cell's distance in to the outline
  by an 8SSEDT sweep. A rim or a ring is the band of cells at its depth, walked cell to
  nearest cell and kept every spacing. A hole in the patch is outline too.
- **What `ProjectSettings` carries.** `border_offset_mm`, `second_border_mm` (the gap inside
  the first rim), `infill_offset_mm` (in from the innermost rim, or the outline) and
  `pattern`: `grid`, `concentric` or `random`, the last ordered by a hash of each cell so a
  fill is repeatable. New fills start at 1 mm offsets; a file without them reads nought and
  a grid, which is what it was filled with. A rim at nought still walks the faces' edges.
- **Two halves, two groups.** `project` answers `Projected { rim, inside }`, and
  `SupportState::fill_profiles` names a saved profile for each by id, or none for the group
  in hand. A fill puts each half in the group built to its profile, made for it when there
  is none, never retuning the group in hand.

## Consequences

A rim no longer leans off the edge, and a flat underside is held by rows and rings the hand
does not have to place. Depth is measured across the plate, so on a face leaning far from
flat an offset is shorter along the surface than asked, and a wall seen edge-on has no
footprint and gets only a rim at nought. Reopen with a distance over the faces if a steep
patch is reported.

## Alternatives considered

### Offset the rim along the faces

Each rim point moved inward across its face and snapped back to the patch: right on a steep
patch, but a ring of the inside then needs every point's distance to the rim over the
surface, which is a geodesic field over the faces for a button press.

### Profiles as group numbers

Simpler to store, but a group's number shifts when one below it is dropped, so a saved
project would fill into the wrong group.

### The option that won, and what it costs

A second way of reading a patch beside the edge walk, a footprint as fine as a quarter of
the spacing, whose memory follows the patch's area, and a fill that may add up to two
groups to the plate.
