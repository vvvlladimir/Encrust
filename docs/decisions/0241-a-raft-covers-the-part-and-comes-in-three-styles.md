# 0241. Draw a raft around the part by a margin in millimetres, in one of three styles

- **Status:** Accepted; the radius a foot is drawn at superseded by 0266
- **Date:** 2026-10-13

## Context

The raft was the hull or bounding box of the foot centres spread about its middle by
√`area_ratio` (ADR 0043). A part overhanging its outermost feet stood past the edge of its
own raft, so the raft that ADR 0238 lifts the model by was smaller than what it lifted. A
ratio also says nothing a user can check against a ruler, and an axis-aligned box around a
part turned on the plate is up to twice the area it needs. And a raft was only ever one
solid slab, which costs resin and sucks hardest on the plate of anything printed.

## Decision

`raft.area_ratio` is replaced by `raft.margin_mm`. A skates or grid raft is drawn to the
convex hull of the model's footprint and every foot's circle, pushed out by `margin_mm`:
with rounded corners for `shape = "hull"`, or as the smallest-area rectangle around that
hull, then pushed out, for `shape = "rectangle"`. The footprint is hulled in parallel over
chunks of the model's vertices.

`raft.style` picks what the raft is made of:

- `skates` — the default: a slab whose sloped wall carries on up past its top at the same
  angle into a lip as tall again as the slab, its inner face leaning at that angle too so
  the lip is 1 mm wide all the way up; one closed solid like a saucer.
- `grid` — a rim, bars both ways across the inside along the outline's longest side with
  square holes between, and a pad under every foot.
- `connected` — a pad under each foot, `margin_mm` past its edge, and a strip a foot wide
  along each edge of the feet's Euclidean minimum spanning tree, so every foot is joined
  to every other by the least strip there is.

A plain slab is not offered: the saucer covers the same ground and peels from its lip.
Every piece is its own closed solid overlapping the others under ADR 0041. Both fields
default for a profile written before them: `style` to `skates`, `margin_mm` to 2 mm.

## Consequences

A raft is never smaller than the part above it, and its size reads off the panel in
millimetres. A rounded corner is held to at least twice the depth the bevel, lip or grid
rim cuts in, so a margin under that adds a little at the corners only. Grid bar and hole
sizes and the lip's width are constants in `core-supports/src/raft.rs`; give them fields if
a user asks. The spanning tree is Prim's on the complete graph, quadratic in the feet;
reopen it with a Delaunay triangulation if a plate's feet run into the thousands. A saved
profile's `area_ratio` is ignored on reading.

## Alternatives considered

### Keep the ratio and add the model's footprint to it

It fixes the size, but a ratio about the middle grows a long part's ends far more than its
sides, and it is still not a length a user can measure.

### Union the feet's circles into the outline, as a wavy slab

It follows the supports more closely, but needs a polygon union this crate does not have,
and leaves the part's own overhang uncovered again.

### The option that won, and what it costs

A raft under a part with few feet is now as big as the part, which costs more resin than
the old foot-only hull; `connected` is the answer for that.
